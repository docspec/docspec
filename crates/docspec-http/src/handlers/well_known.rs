//! Discovery document handler.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;

use crate::{discovery, error::HttpError, format};

/// Handle `GET /.well-known/docspec` — describe this server and link to its resources.
///
/// Returns 200 with `Content-Type: application/json` and a body holding the
/// server `version`, `links` to `/conversion` and `/health`, and the
/// `conversion` media types (same lists as `GET /conversion`).
///
/// # Errors
///
/// Returns [`HttpError::Internal`] if the body cannot be serialized.
#[inline]
// Reason: Axum handlers must be async to satisfy the Handler trait, even if they don't await.
#[allow(clippy::unused_async)]
pub async fn get_well_known() -> Result<impl IntoResponse, HttpError> {
    let body = discovery::well_known_json().map_err(HttpError::internal)?;
    Ok((
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        body,
    ))
}

/// Fallback handler for wrong HTTP methods on `/.well-known/docspec`.
///
/// Returns 405 Method Not Allowed with `Allow: GET, HEAD`.
#[inline]
#[must_use]
// Reason: Axum handlers must be async to satisfy the Handler trait, even if they don't await.
#[allow(clippy::unused_async)]
pub async fn well_known_method_not_allowed() -> HttpError {
    HttpError::MethodNotAllowed {
        allowed: format::ALLOW_WELL_KNOWN,
    }
}
