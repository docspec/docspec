//! Discovery document handler.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;

use crate::{
    discovery::WellKnown,
    error::{HttpError, ProblemJson},
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
#[utoipa::path(
    get,
    path = "/.well-known/docspec",
    tag = "discovery",
    summary = "Discovery document",
    description = "The version, links to the other resources (relative to this document's URL), \
                   and the media types `/conversion` supports.",
    responses(
        (status = 200, description = "The discovery document.", body = WellKnown, content_type = "application/json"),
        (status = 500, description = "The response could not be built.", body = ProblemJson, content_type = "application/problem+json"),
    ),
)]
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
