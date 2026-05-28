//! Document conversion request handlers.

use axum::{
    body::{Body, Bytes},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
    response::IntoResponse,
};
use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::{EventSink as _, EventSource as _, StackTrackingSink};
use docspec_markdown_reader::MarkdownReader;

use crate::{error::HttpError, mime_parser};

/// Handle `OPTIONS /conversion` — returns allowed methods.
#[allow(clippy::unused_async)]
// Reason: Axum handlers are async for route consistency even when no await is needed.
#[inline]
pub async fn options_conversion() -> impl IntoResponse {
    (
        StatusCode::NO_CONTENT,
        [(header::ALLOW, HeaderValue::from_static("POST, OPTIONS"))],
    )
}

/// Handle `POST /conversion` — convert markdown to `BlockNote` JSON.
///
/// The request body is buffered, then converted to completion inside
/// `spawn_blocking`, then returned in a single response. Conversion errors
/// surface as 422 (parse / sink errors) or 500 (finalize errors) **before**
/// any response body is sent — no truncated `200 OK` on failure.
///
/// # Errors
///
/// Returns [`HttpError`] when request headers or body are invalid, the
/// conversion fails, or the response cannot be constructed.
#[inline]
pub async fn post_conversion(headers: HeaderMap, body: Bytes) -> Result<Response<Body>, HttpError> {
    mime_parser::validate_content_type(headers.get(header::CONTENT_TYPE))?;
    mime_parser::negotiate_accept(headers.get(header::ACCEPT))?;

    if body.is_empty() {
        return Err(HttpError::EmptyBody);
    }

    let markdown = String::from_utf8(body.into()).map_err(|error| {
        tracing::debug!(error = %error, "request body is not valid UTF-8");
        HttpError::BodyNotUtf8
    })?;

    let output = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, HttpError> {
        let mut output_buffer = Vec::new();
        let mut reader = MarkdownReader::new(&markdown);
        let mut sink = StackTrackingSink::new(BlockNoteWriter::new(&mut output_buffer));

        loop {
            match reader.next_event() {
                Ok(Some(event)) => sink.handle_event(event).map_err(|error| {
                    tracing::debug!(error = %error, "conversion sink failed");
                    HttpError::Unprocessable {
                        detail: error.to_string(),
                    }
                })?,
                Ok(None) => break,
                Err(error) => {
                    tracing::debug!(error = %error, "markdown reader failed");
                    return Err(HttpError::Unprocessable {
                        detail: error.to_string(),
                    });
                }
            }
        }

        sink.finish().map_err(|error| {
            tracing::debug!(error = %error, "conversion sink finish failed");
            HttpError::Internal
        })?;

        Ok(output_buffer)
    })
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "spawn_blocking join failed");
        HttpError::Internal
    })??;

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/vnd.docspec.blocknote+json; charset=utf-8"),
        )
        .body(Body::from(output))
        .map_err(|error| {
            tracing::error!(error = %error, "failed to build conversion response");
            HttpError::Internal
        })
}
