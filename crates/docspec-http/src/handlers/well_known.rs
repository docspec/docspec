//! Discovery document handler.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;

use crate::{
    discovery::{ApiCatalog, WellKnown},
    error::HttpError,
    format,
};

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
    let body = WellKnown::current()
        .to_json()
        .map_err(HttpError::internal)?;
    Ok((
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        body,
    ))
}

/// Handle `GET /.well-known/api-catalog` — the RFC 9727 API catalog.
///
/// Returns 200 with an RFC 9264 linkset pointing at `/conversion` and its
/// `OpenAPI` description, plus the `Link: rel="api-catalog"` header that
/// RFC 9727 §2 requires on `HEAD` (axum answers `HEAD` from this handler).
///
/// # Errors
///
/// Returns [`HttpError::Internal`] if the body cannot be serialized.
#[inline]
// Reason: Axum handlers must be async to satisfy the Handler trait, even if they don't await.
#[allow(clippy::unused_async)]
pub async fn get_api_catalog() -> Result<impl IntoResponse, HttpError> {
    let body = ApiCatalog::current()
        .to_json()
        .map_err(HttpError::internal)?;
    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(format::API_CATALOG_CONTENT_TYPE),
            ),
            (
                header::LINK,
                HeaderValue::from_static(format::API_CATALOG_LINK),
            ),
        ],
        body,
    ))
}
