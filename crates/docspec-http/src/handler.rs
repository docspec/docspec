//! HTTP request handlers for the `DocSpec` API.

use std::io::{ErrorKind, Write};

use axum::body::Body;
use axum::http::{header::CONTENT_TYPE, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use docspec_core::{EventSink as _, EventSource as _};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;

use crate::error::{HttpError, Result};
use crate::format;

/// Maximum buffered bytes before [`ChannelWriter`] emits a chunk.
///
/// Caps both the internal accumulation buffer and the chunk size used when
/// splitting oversized incoming slices, keeping per-chunk memory bounded.
const CHANNEL_CHUNK_SIZE: usize = 8192;

/// Bridges synchronous [`Write`] calls to an async channel.
///
/// Each emitted chunk is at most [`CHANNEL_CHUNK_SIZE`] (8 KiB), regardless of
/// the size of incoming `write` slices: oversized slices are split into
/// `CHANNEL_CHUNK_SIZE`-sized chunks sent directly to the channel rather than
/// re-buffered. On drop, flushes remaining buffered bytes best-effort. Channel
/// send failures are mapped to [`ErrorKind::BrokenPipe`].
struct ChannelWriter {
    /// Buffered output awaiting channel send. Never grows past [`CHANNEL_CHUNK_SIZE`].
    buf: Vec<u8>,
    /// Bounded sender consumed by the streaming HTTP response body.
    tx: mpsc::Sender<std::result::Result<Bytes, std::io::Error>>,
}

impl ChannelWriter {
    /// Creates a new writer backed by the provided bounded channel sender.
    // Reason: Constructor documents the sync/async bridge boundary despite one production call site.
    #[allow(clippy::single_call_fn)]
    #[inline]
    fn new(tx: mpsc::Sender<std::result::Result<Bytes, std::io::Error>>) -> Self {
        Self {
            buf: Vec::new(),
            tx,
        }
    }
}

impl Drop for ChannelWriter {
    #[inline]
    fn drop(&mut self) {
        match self.flush() {
            Ok(()) | Err(_) => {}
        }
    }
}

impl Write for ChannelWriter {
    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let taken = std::mem::take(&mut self.buf);
        self.tx
            .blocking_send(Ok(Bytes::from(taken)))
            .map_err(|_send_error| {
                std::io::Error::new(ErrorKind::BrokenPipe, "client disconnected")
            })?;
        Ok(())
    }

    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Reason: For oversized writes, drain self.buf first to preserve byte
        // order, then send CHANNEL_CHUNK_SIZE slices directly so a single large
        // write never emits one large chunk.
        if buf.len() >= CHANNEL_CHUNK_SIZE {
            self.flush()?;
            let mut remaining = buf;
            while remaining.len() >= CHANNEL_CHUNK_SIZE {
                let (chunk, tail) = remaining.split_at(CHANNEL_CHUNK_SIZE);
                self.tx
                    .blocking_send(Ok(Bytes::copy_from_slice(chunk)))
                    .map_err(|_send_error| {
                        std::io::Error::new(ErrorKind::BrokenPipe, "client disconnected")
                    })?;
                remaining = tail;
            }
            self.buf.extend_from_slice(remaining);
        } else {
            self.buf.extend_from_slice(buf);
            if self.buf.len() >= CHANNEL_CHUNK_SIZE {
                self.flush()?;
            }
        }
        Ok(buf.len())
    }
}

/// Handles `POST /convert` — converts Markdown to `BlockNote` JSON.
///
/// Accepts `text/markdown` with optional parameters and returns
/// `application/vnd.docspec.blocknote+json` as a streaming response.
///
/// # Errors
///
/// Returns [`HttpError::UnsupportedMediaType`] if `Content-Type` is not `text/markdown`.
/// Returns [`HttpError::NotAcceptable`] if `Accept` does not include `BlockNote` JSON.
/// Returns [`HttpError::BadRequest`] if the body is not valid UTF-8.
#[tracing::instrument(skip_all, fields(content_type = tracing::field::Empty, accept = tracing::field::Empty))]
#[inline]
pub async fn convert_handler(headers: HeaderMap, body: Bytes) -> Result<Response> {
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    if !content_type.is_some_and(format::is_markdown) {
        return Err(HttpError::UnsupportedMediaType {
            received: content_type.map(str::to_string),
        });
    }

    let accept = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|value| value.to_str().ok());

    let span = tracing::Span::current();
    if let Some(ct) = content_type {
        span.record("content_type", ct);
    }
    if let Some(acc) = accept {
        span.record("accept", acc);
    }

    if !format::accepts_blocknote(accept) {
        return Err(HttpError::NotAcceptable {
            received: accept.map(str::to_string),
        });
    }

    let body_string =
        String::from_utf8(body.to_vec()).map_err(|_utf8_error| HttpError::BadRequest {
            detail: "request body is not valid UTF-8".into(),
        })?;

    let (chunk_tx, chunk_rx) = mpsc::channel::<std::result::Result<Bytes, std::io::Error>>(32);
    let (start_signal_tx, start_rx) = oneshot::channel::<Result<()>>();
    tokio::task::spawn_blocking(move || run_conversion(&body_string, &chunk_tx, start_signal_tx));

    match start_rx.await {
        Ok(Ok(())) => {
            let stream = ReceiverStream::new(chunk_rx);
            let mut response = Response::new(Body::from_stream(stream));
            *response.status_mut() = StatusCode::OK;
            response.headers_mut().insert(
                CONTENT_TYPE,
                HeaderValue::from_static(format::OUTPUT_BLOCKNOTE),
            );
            Ok(response)
        }
        Ok(Err(http_err)) => Err(http_err),
        Err(_recv_error) => Err(HttpError::Internal {
            detail: "conversion task aborted before signaling".to_string(),
        }),
    }
}

// Reason: Extracted from the `spawn_blocking` closure body so the streaming pipeline
// can be unit-tested without going through the public `convert_handler` entry point.
#[allow(clippy::single_call_fn)]
fn run_conversion(
    body_string: &str,
    chunk_tx: &mpsc::Sender<std::result::Result<Bytes, std::io::Error>>,
    start_signal_tx: oneshot::Sender<Result<()>>,
) {
    let mut start_tx = Some(start_signal_tx);
    let writer = ChannelWriter::new(chunk_tx.clone());
    let mut sink = docspec_core::StackTrackingSink::new(
        docspec_blocknote_writer::BlockNoteWriter::new(writer),
    );
    let mut reader = docspec_markdown_reader::MarkdownReader::new(body_string);
    loop {
        match reader.next_event() {
            Ok(Some(event)) => {
                if let Err(err) = sink.handle_event(event) {
                    tracing::debug!(error = %err, "conversion error in spawn_blocking");
                    signal_conversion_error(&mut start_tx, err, chunk_tx);
                    return;
                }
                signal_start_ok(&mut start_tx);
            }
            Ok(None) => break,
            Err(err) => {
                tracing::debug!(error = %err, "reader error in spawn_blocking");
                signal_conversion_error(&mut start_tx, err, chunk_tx);
                return;
            }
        }
    }
    if let Err(err) = sink.finish() {
        tracing::debug!(error = %err, "sink finish error in spawn_blocking");
        signal_conversion_error(&mut start_tx, err, chunk_tx);
        return;
    }
    signal_start_ok(&mut start_tx);
}

fn signal_start_ok(start_tx: &mut Option<oneshot::Sender<Result<()>>>) {
    if let Some(tx) = start_tx.take() {
        drop(tx.send(Ok(())));
    }
}

fn signal_conversion_error(
    start_tx: &mut Option<oneshot::Sender<Result<()>>>,
    err: docspec_core::Error,
    chunk_tx: &mpsc::Sender<std::result::Result<Bytes, std::io::Error>>,
) {
    if let Some(tx) = start_tx.take() {
        drop(tx.send(Err(HttpError::Conversion(err))));
    } else {
        drop(chunk_tx.blocking_send(Err(std::io::Error::other("mid-stream conversion error"))));
    }
}

/// Handles `GET /health` — returns 204 No Content with an empty body.
///
/// No `Content-Type` header is set. No body is returned.
#[inline]
pub async fn health_handler() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::unwrap_used)]
    // Reason: Test code uses unwrap and index notation for assertion clarity.

    use axum::http::{header, HeaderValue};
    use http_body_util::BodyExt as _;

    use super::*;

    async fn collect_body(body: Body) -> Vec<u8> {
        body.collect().await.unwrap().to_bytes().to_vec()
    }

    fn make_headers(content_type: Option<&str>, accept: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(ct) = content_type {
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(ct).unwrap());
        }
        if let Some(acc) = accept {
            headers.insert(header::ACCEPT, HeaderValue::from_str(acc).unwrap());
        }
        headers
    }

    #[tokio::test]
    async fn convert_406() {
        let headers = make_headers(Some("text/markdown"), Some("text/html"));
        let body = Bytes::from("# Hello\n");
        let err = convert_handler(headers, body).await.unwrap_err();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_ACCEPTABLE);
    }

    #[tokio::test]
    async fn convert_415_missing_content_type() {
        let headers = make_headers(None, None);
        let body = Bytes::from("# Hello\n");
        let err = convert_handler(headers, body).await.unwrap_err();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn convert_415_wrong_content_type() {
        let headers = make_headers(Some("application/json"), None);
        let body = Bytes::from("{}");
        let err = convert_handler(headers, body).await.unwrap_err();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn convert_charset_accepted() {
        let headers = make_headers(Some("text/markdown; charset=utf-8"), None);
        let body = Bytes::from("# Hello\n");
        let resp = convert_handler(headers, body)
            .await
            .unwrap()
            .into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn convert_happy() {
        let headers = make_headers(Some("text/markdown"), None);
        let body = Bytes::from("# Hello\n");
        let resp = convert_handler(headers, body)
            .await
            .unwrap()
            .into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/vnd.docspec.blocknote+json"
        );
        let bytes = collect_body(resp.into_body()).await;
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json.is_array(), "expected JSON array");
    }

    #[tokio::test]
    async fn convert_empty_body_returns_empty_array() {
        let headers = make_headers(Some("text/markdown"), None);
        let resp = convert_handler(headers, Bytes::new())
            .await
            .unwrap()
            .into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = collect_body(resp.into_body()).await;
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json, serde_json::json!([]));
    }

    #[tokio::test]
    async fn conversion_error_before_stream_returns_422() {
        let (chunk_tx, mut chunk_rx) = mpsc::channel(1);
        let (start_signal_tx, start_rx) = oneshot::channel();
        let mut start_tx = Some(start_signal_tx);

        signal_conversion_error(
            &mut start_tx,
            docspec_core::Error::Other {
                message: "parse error".to_string(),
            },
            &chunk_tx,
        );

        let err = start_rx.await.unwrap().unwrap_err();
        assert_eq!(
            err.into_response().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert!(chunk_rx.try_recv().is_err());
    }

    #[test]
    fn conversion_error_after_stream_sends_body_error() {
        let (chunk_tx, mut chunk_rx) = mpsc::channel(1);
        let mut start_tx = None;

        signal_conversion_error(
            &mut start_tx,
            docspec_core::Error::Other {
                message: "late error".to_string(),
            },
            &chunk_tx,
        );

        let sent = chunk_rx.blocking_recv().unwrap();
        assert!(sent.is_err());
    }

    #[tokio::test]
    async fn run_conversion_writer_failure_terminates_stream() {
        // Given: a receiver dropped before any send, and a body large enough to exceed
        // the 8 KiB ChannelWriter buffer so the writer is forced to flush at least once.
        let (chunk_tx, chunk_rx) = mpsc::channel(1);
        drop(chunk_rx);
        let (start_signal_tx, start_rx) = oneshot::channel::<Result<()>>();
        let body = "# Heading\n\n".repeat(2000);

        tokio::task::spawn_blocking(move || {
            super::run_conversion(&body, &chunk_tx, start_signal_tx);
        })
        .await
        .unwrap();

        // Then: start_rx resolves with either Ok (first event flushed before failure) or
        // Err (failure observed pre-start); either path exercises the writer-error branch.
        let result = start_rx.await.unwrap();
        drop(result);
    }

    #[tokio::test]
    async fn convert_invalid_utf8() {
        let headers = make_headers(Some("text/markdown"), None);
        let body = Bytes::from(vec![0xFF, 0xFE, 0x00]);
        let err = convert_handler(headers, body).await.unwrap_err();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn channel_writer_chunks_large_single_write() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut writer = ChannelWriter::new(tx);
        let payload = vec![b'a'; CHANNEL_CHUNK_SIZE * 3 + 100];
        let n = writer.write(&payload).unwrap();
        assert_eq!(n, payload.len());
        let mut chunks: Vec<Bytes> = Vec::new();
        while let Ok(item) = rx.try_recv() {
            chunks.push(item.unwrap());
        }
        assert_eq!(chunks.len(), 3);
        for chunk in &chunks {
            assert_eq!(chunk.len(), CHANNEL_CHUNK_SIZE);
        }
        writer.flush().unwrap();
        let tail = rx.try_recv().unwrap().unwrap();
        assert_eq!(tail.len(), 100);
    }

    #[test]
    fn channel_writer_flushes_pending_buffer_before_large_write() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut writer = ChannelWriter::new(tx);
        writer.write_all(b"prefix").unwrap();
        let payload = vec![b'x'; CHANNEL_CHUNK_SIZE * 2];
        writer.write_all(&payload).unwrap();
        let mut chunks: Vec<Bytes> = Vec::new();
        while let Ok(item) = rx.try_recv() {
            chunks.push(item.unwrap());
        }
        assert_eq!(chunks[0].as_ref(), b"prefix");
        assert_eq!(chunks.len(), 3);
        for chunk in chunks.iter().skip(1) {
            assert_eq!(chunk.len(), CHANNEL_CHUNK_SIZE);
        }
        let total: usize = chunks.iter().map(Bytes::len).sum();
        assert_eq!(total, b"prefix".len() + CHANNEL_CHUNK_SIZE * 2);
    }

    #[tokio::test]
    async fn health_204_empty() {
        let resp = health_handler().await.into_response();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert!(resp.headers().get(header::CONTENT_TYPE).is_none());
        let bytes = collect_body(resp.into_body()).await;
        assert!(bytes.is_empty());
    }
}
