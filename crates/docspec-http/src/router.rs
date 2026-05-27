//! Axum router assembly with middleware stack.

use axum::routing::{get, post};
use axum::Router;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

use crate::error::HttpError;
use crate::handler::{convert_handler, health_handler};

/// Builds the application router with all routes and middleware.
///
/// Routes:
/// - `POST /convert` — Markdown to `BlockNote` JSON conversion
/// - `GET /health` — Health check (204 No Content)
///
/// Middleware stack (outermost first — `tower`'s `.layer()` wraps the existing
/// service, so the last `.layer(...)` call in source order is the outermost
/// layer at runtime):
/// 1. [`SetRequestIdLayer`] — generates `x-request-id` UUID per request
/// 2. [`PropagateRequestIdLayer`] — copies `x-request-id` to response headers
/// 3. [`TraceLayer`] — logs method, path, status, latency
///
/// Unknown routes return 404 with RFC 7807 problem+json.
#[inline]
pub fn router() -> Router {
    Router::new()
        .route("/convert", post(convert_handler))
        .route("/health", get(health_handler))
        .fallback(fallback_handler)
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}

/// Handles unknown routes with a 404 RFC 7807 problem+json response.
// Reason: This function is called only via Axum's fallback mechanism, not from production Rust code.
#[allow(clippy::single_call_fn)]
async fn fallback_handler() -> HttpError {
    HttpError::NotFound
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::single_call_fn)]
    // Reason: Test code uses unwrap, index notation, and small helpers called once for assertion clarity.

    use axum::body::Body;
    use axum::http::{header, Method, Request, StatusCode};
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;

    use super::*;

    async fn collect_body(body: Body) -> Vec<u8> {
        body.collect().await.unwrap().to_bytes().to_vec()
    }

    #[tokio::test]
    async fn convert_route_wired() {
        let app = router();
        let req = Request::builder()
            .method(Method::POST)
            .uri("/convert")
            .header(header::CONTENT_TYPE, "text/markdown")
            .body(Body::from("# Hello\n"))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/vnd.docspec.blocknote+json"
        );
    }

    #[tokio::test]
    async fn request_id_header_present() {
        let app = router();
        let req = Request::builder()
            .method(Method::GET)
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert!(resp.headers().get("x-request-id").is_some());
    }

    #[tokio::test]
    async fn unknown_route_404() {
        let app = router();
        let req = Request::builder()
            .method(Method::GET)
            .uri("/unknown")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/problem+json"
        );
        let bytes = collect_body(resp.into_body()).await;
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["type"], "https://docspec.dev/errors/not-found");
    }
}
