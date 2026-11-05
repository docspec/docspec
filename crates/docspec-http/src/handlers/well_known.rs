//! Discovery document handler.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;

use crate::{
    discovery::{ApiCatalog, WellKnown},
    error::{HttpError, Problem},
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
#[utoipa::path(
    get,
    path = "/.well-known/api-catalog",
    tag = "discovery",
    summary = "API catalog (RFC 9727)",
    description = "A linkset (RFC 9264) from the conversion API to its OpenAPI description. \
                   Links are relative to this document's URL. `HEAD` returns the \
                   `Link: rel=\"api-catalog\"` header without a body.",
    responses(
        (
            status = 200,
            description = "The API catalog.",
            body = ApiCatalog,
            content_type = format::API_CATALOG_CONTENT_TYPE,
            headers(("Link" = String, description = "`</.well-known/api-catalog>; rel=\"api-catalog\"`")),
        ),
        (status = 500, description = "The response could not be built.", body = Problem, content_type = "application/problem+json"),
    ),
)]
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
