//! Health check handler.

use axum::http;
use axum::http::HeaderValue;
use axum::response::IntoResponse;

use crate::format;

/// Handle `GET /health` — liveness check.
///
/// Returns 200 OK with body `"Healthy."` and `Content-Type: text/plain; charset=utf-8`.
#[inline]
#[must_use]
// Reason: Axum handlers must be async to satisfy the Handler trait, even if they don't await.
#[allow(clippy::unused_async)]
pub async fn get_health() -> impl IntoResponse {
    (
        http::StatusCode::OK,
        [(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        format::HEALTH_BODY,
    )
}

/// Handle `HEAD /health` — liveness check without body.
///
/// Returns 204 No Content with no body and no Content-Type.
/// This handler is registered explicitly because Axum's auto-HEAD returns 200
/// with an empty body, which is incorrect for a health check (should be 204).
#[inline]
#[must_use]
// Reason: Axum handlers must be async to satisfy the Handler trait, even if they don't await.
#[allow(clippy::unused_async)]
pub async fn head_health() -> impl IntoResponse {
    http::StatusCode::NO_CONTENT
}

/// Handle `OPTIONS /health` — advertise allowed methods.
///
/// Returns 204 No Content with `Allow: GET, HEAD, OPTIONS`.
#[inline]
#[must_use]
// Reason: Axum handlers must be async to satisfy the Handler trait, even if they don't await.
#[allow(clippy::unused_async)]
pub async fn options_health() -> impl IntoResponse {
    (
        http::StatusCode::NO_CONTENT,
        [(
            http::header::ALLOW,
            HeaderValue::from_static("GET, HEAD, OPTIONS"),
        )],
    )
}
