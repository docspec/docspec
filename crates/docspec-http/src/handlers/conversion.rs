//! Document conversion request handlers.

use core::mem;
use std::io::{self, Write as _};

use axum::{
    body::{Body, Bytes},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
    response::IntoResponse,
};
use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::{EventSink as _, EventSource as _, StackTrackingSink};
use docspec_markdown_reader::MarkdownReader;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::{error::HttpError, format};

const CHANNEL_WRITER_BUF_SIZE: usize = 8 * 1024;

/// A [`std::io::Write`] adapter that sends buffered chunks over a tokio mpsc sender.
///
/// Internal 8 KiB buffer. Flushes to the channel on each `flush()` call or
/// when dropped. Used inside a `spawn_blocking` task to stream the conversion
/// output to the HTTP response body.
///
/// # SAFETY (fire-and-forget `JoinHandle`)
///
/// The `spawn_blocking` `JoinHandle` that drives this writer is intentionally
/// dropped without awaiting. Awaiting it would deadlock: the response body
/// consumer drives the mpsc receiver, but the blocking task may be suspended
/// waiting for channel capacity until that receiver is polled. Drop of `tx`
/// signals EOF to the response stream.
struct ChannelWriter {
    buf: Vec<u8>,
    tx: mpsc::Sender<Result<Bytes, io::Error>>,
}

impl ChannelWriter {
    #[allow(clippy::single_call_fn)]
    // Reason: Constructor documents the ChannelWriter initialization contract.
    #[inline]
    fn new(tx: mpsc::Sender<Result<Bytes, io::Error>>) -> Self {
        Self {
            buf: Vec::with_capacity(CHANNEL_WRITER_BUF_SIZE),
            tx,
        }
    }
}

impl Drop for ChannelWriter {
    #[inline]
    fn drop(&mut self) {
        if let Err(err) = self.flush() {
            tracing::debug!(error = %err, "ChannelWriter flush on drop failed");
        }
    }
}

impl io::Write for ChannelWriter {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }

        let chunk = Bytes::from(mem::take(&mut self.buf));
        self.tx
            .blocking_send(Ok(chunk))
            .map_err(|err| io::Error::new(io::ErrorKind::BrokenPipe, err.to_string()))
    }

    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        if self.buf.len() >= CHANNEL_WRITER_BUF_SIZE {
            self.flush()?;
        }
        Ok(buf.len())
    }
}

/// Handle `OPTIONS /conversion` — returns allowed methods.
#[allow(clippy::unused_async)]
// Reason: Axum handlers are async for route consistency even when no await is needed.
#[inline]
pub async fn options_conversion() -> impl IntoResponse {
    (
        StatusCode::NO_CONTENT,
        [(header::ALLOW, HeaderValue::from_static("POST"))],
    )
}

/// Handle `POST /conversion` — convert markdown to `BlockNote` JSON, streamed.
///
/// # Errors
///
/// Returns [`HttpError`] when request headers or body are invalid, or when the
/// response cannot be constructed.
#[allow(clippy::unused_async)]
#[allow(clippy::unnecessary_safety_comment)]
// Reason: Axum handlers are async for route consistency even when no await is needed.
// Reason: The task requires a SAFETY comment for this fire-and-forget JoinHandle.
#[inline]
pub async fn post_conversion(headers: HeaderMap, body: Bytes) -> Result<Response<Body>, HttpError> {
    format::validate_content_type(headers.get(header::CONTENT_TYPE))?;
    format::negotiate_accept(headers.get(header::ACCEPT))?;

    if body.is_empty() {
        return Err(HttpError::EmptyBody);
    }

    let markdown = String::from_utf8(body.into()).map_err(|err| {
        tracing::debug!(error = %err, "request body is not valid UTF-8");
        HttpError::BodyNotUtf8
    })?;

    let (tx, rx) = mpsc::channel::<Result<Bytes, io::Error>>(32);
    let writer = ChannelWriter::new(tx);

    // SAFETY: Fire-and-forget by design. Awaiting this JoinHandle would prevent
    // the response body receiver from driving channel progress and can deadlock.
    // Dropping the handle lets the blocking conversion continue; dropping `tx`
    // at task completion or error signals EOF to the streaming body.
    drop(tokio::task::spawn_blocking(move || {
        let mut reader = MarkdownReader::new(&markdown);
        let mut sink = StackTrackingSink::new(BlockNoteWriter::new(writer));

        let mut next = reader.next_event();
        while let Ok(Some(event)) = next {
            if let Err(err) = sink.handle_event(event) {
                tracing::debug!(error = %err, "conversion sink failed");
                return;
            }
            next = reader.next_event();
        }

        if let Err(err) = next {
            tracing::debug!(error = %err, "markdown reader failed");
            return;
        }

        if let Err(err) = sink.finish() {
            tracing::debug!(error = %err, "conversion sink finish failed");
        }
    }));

    let stream = ReceiverStream::new(rx);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, format::OUTPUT_MIME_FULL)
        .body(Body::from_stream(stream))
        .map_err(|err| {
            tracing::error!(error = %err, "failed to build conversion response");
            HttpError::Internal
        })
}

#[cfg(test)]
mod tests {
    // Reason: test code uses explicit panics for invalid setup and assertions.
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

    use axum::{
        body::Body,
        http::{Method, Request},
        routing::post,
        Router,
    };
    use serde_json::Value;
    use tower::ServiceExt as _;

    use super::*;

    #[inline]
    async fn body_text(response: Response<Body>) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body collection succeeds");
        String::from_utf8(bytes.to_vec()).expect("response body is UTF-8")
    }

    #[inline]
    fn markdown_request(markdown: &'static str) -> Request<Body> {
        Request::post("/conversion")
            .header(header::CONTENT_TYPE, format::INPUT_MIME_MARKDOWN)
            .header(header::ACCEPT, format::OUTPUT_MIME_PRIMARY)
            .body(Body::from(markdown))
            .expect("valid markdown request")
    }

    #[inline]
    fn test_router() -> Router {
        Router::new().route(
            "/conversion",
            post(post_conversion).options(options_conversion),
        )
    }

    #[tokio::test]
    async fn empty_body_returns_400() {
        let response = test_router()
            .oneshot(markdown_request(""))
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = body_text(response).await;
        assert!(body.contains("Request body is empty"));
    }

    #[tokio::test]
    async fn happy_path_returns_blocknote_json() {
        let response = test_router()
            .oneshot(markdown_request("# Hello"))
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .expect("content-type present"),
            format::OUTPUT_MIME_FULL
        );

        let body = body_text(response).await;
        let json: Value = serde_json::from_str(&body).expect("valid JSON");
        assert!(json.is_array());
    }

    #[tokio::test]
    async fn invalid_utf8_returns_400() {
        let request = Request::post("/conversion")
            .header(header::CONTENT_TYPE, format::INPUT_MIME_MARKDOWN)
            .header(header::ACCEPT, format::OUTPUT_MIME_PRIMARY)
            .body(Body::from(Bytes::from_static(&[0xFF, 0xFE])))
            .expect("valid request");

        let response = test_router()
            .oneshot(request)
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = body_text(response).await;
        assert!(body.contains("not valid UTF-8"));
    }

    #[tokio::test]
    async fn missing_accept_returns_200() {
        let request = Request::post("/conversion")
            .header(header::CONTENT_TYPE, format::INPUT_MIME_MARKDOWN)
            .body(Body::from("# Hello"))
            .expect("valid request");

        let response = test_router()
            .oneshot(request)
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn missing_content_type_returns_415() {
        let request = Request::post("/conversion")
            .header(header::ACCEPT, format::OUTPUT_MIME_PRIMARY)
            .body(Body::from("# Hello"))
            .expect("valid request");

        let response = test_router()
            .oneshot(request)
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn options_returns_204() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/conversion")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response
                .headers()
                .get(header::ALLOW)
                .expect("allow header present"),
            "POST"
        );
    }

    #[tokio::test]
    async fn wrong_accept_returns_406() {
        let request = Request::post("/conversion")
            .header(header::CONTENT_TYPE, format::INPUT_MIME_MARKDOWN)
            .header(header::ACCEPT, "application/json")
            .body(Body::from("# Hello"))
            .expect("valid request");

        let response = test_router()
            .oneshot(request)
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
    }

    #[tokio::test]
    async fn wrong_content_type_returns_415() {
        let request = Request::post("/conversion")
            .header(header::CONTENT_TYPE, "text/plain")
            .header(header::ACCEPT, format::OUTPUT_MIME_PRIMARY)
            .body(Body::from("# Hello"))
            .expect("valid request");

        let response = test_router()
            .oneshot(request)
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
}
