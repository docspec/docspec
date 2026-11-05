//! `OpenAPI` document handler.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;

use crate::{
    error::{HttpError, ProblemJson},
    openapi::{self, MIME_OPENAPI},
};

/// Handle `GET /openapi.json` — the `OpenAPI` 3.1 description of this server.
///
/// # Errors
///
/// Returns [`HttpError::Internal`] if the document could not be built.
#[utoipa::path(
    get,
    path = "/openapi.json",
    tag = "discovery",
    summary = "OpenAPI description",
    description = "This document.",
    responses(
        (status = 200, description = "The OpenAPI 3.1 document.", body = serde_json::Value, content_type = MIME_OPENAPI),
        (status = 500, description = "The document could not be built.", body = ProblemJson, content_type = "application/problem+json"),
    ),
)]
#[inline]
// Reason: Axum handlers must be async to satisfy the Handler trait, even if they don't await.
#[allow(clippy::unused_async)]
pub async fn get_openapi() -> Result<impl IntoResponse, HttpError> {
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static(MIME_OPENAPI))],
        openapi::json()?,
    ))
}
