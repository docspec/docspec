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
            HeaderValue::from_static(format::HEALTH_CONTENT_TYPE),
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

#[cfg(test)]
mod tests {
    // Reason: test code legitimately uses unwrap for asserting expected-Ok results;
    // panicking here indicates a test bug, not a runtime error.
    #![allow(clippy::unwrap_used)]

    use super::*;
    use axum::routing::MethodRouter;
    use axum::Router;
    use http::Request;
    use tower::ServiceExt as _;

    fn test_router() -> Router {
        Router::new().route(
            "/health",
            MethodRouter::new()
                .get(get_health)
                .head(head_health)
                .options(options_health),
        )
    }

    #[tokio::test]
    async fn get_returns_200() {
        let app = test_router();
        let request = Request::builder()
            .method("GET")
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), http::StatusCode::OK);

        let content_type = response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(content_type, "text/plain; charset=utf-8");

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), b"Healthy.");
    }

    #[tokio::test]
    async fn head_returns_204() {
        let app = test_router();
        let request = Request::builder()
            .method("HEAD")
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), http::StatusCode::NO_CONTENT);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn options_returns_204() {
        let app = test_router();
        let request = Request::builder()
            .method("OPTIONS")
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), http::StatusCode::NO_CONTENT);

        let allow_header = response
            .headers()
            .get(http::header::ALLOW)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(allow_header, "GET, HEAD, OPTIONS");
    }
}
