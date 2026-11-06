//! `OpenAPI` document handler.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;

use crate::{
    error::HttpError,
    openapi::{self, MIME_OPENAPI},
};

/// Handle `GET /openapi.json` — the `OpenAPI` 3.1 description of this server.
///
/// # Errors
///
/// Returns [`HttpError::Internal`] if the document could not be built.
#[inline]
pub async fn get_openapi() -> Result<impl IntoResponse, HttpError> {
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static(MIME_OPENAPI))],
        openapi::json()?,
    ))
}
