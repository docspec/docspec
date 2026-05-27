//! HTTP router and route definitions.

use axum::{http::Request, Router};
use tower_http::request_id::{MakeRequestId, RequestId};

/// A [`MakeRequestId`] implementation that never generates a request ID.
///
/// Used for `X-Trace-ID`: the header is echoed if present but never generated
/// if absent. This matches docspecio/api's behavior where `X-Trace-ID` is only
/// propagated from upstream, never self-assigned.
#[derive(Clone, Copy)]
struct EchoOnly;

impl MakeRequestId for EchoOnly {
    #[inline]
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        None
    }
}

/// Build the HTTP API router with all routes and middleware.
#[inline]
pub fn router() -> Router {
    use axum::http::header::HeaderName;
    use axum::routing::{get, post};
    use tower::ServiceBuilder;
    use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
    use tower_http::trace::TraceLayer;

    use crate::cache::cache_control_layer;
    use crate::handlers::{
        conversion::{options_conversion, post_conversion},
        fallback::{conversion_method_not_allowed, health_method_not_allowed, not_found},
        health::{get_health, head_health, options_health},
    };

    let x_request_id = HeaderName::from_static("x-request-id");
    let x_trace_id = HeaderName::from_static("x-trace-id");

    let conversion_route = post(post_conversion)
        .options(options_conversion)
        .fallback(conversion_method_not_allowed);

    let health_route = get(get_health)
        .head(head_health)
        .options(options_health)
        .fallback(health_method_not_allowed);

    Router::new()
        .route("/conversion", conversion_route)
        .route("/health", health_route)
        .fallback(not_found)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::new(
                    x_request_id.clone(),
                    MakeRequestUuid,
                ))
                .layer(SetRequestIdLayer::new(x_trace_id.clone(), EchoOnly))
                .layer(TraceLayer::new_for_http())
                .layer(PropagateRequestIdLayer::new(x_request_id))
                .layer(PropagateRequestIdLayer::new(x_trace_id))
                .layer(cache_control_layer()),
        )
}

#[cfg(test)]
mod tests {
    // Reason: router tests use expect to make invalid test setup fail loudly.
    #![allow(clippy::expect_used)]

    use axum::{
        body::Body,
        http::{header, Method, Request, StatusCode},
    };
    use tower::ServiceExt as _;

    use super::router;

    const CACHE_CONTROL_VALUE: &str = "max-age=0, private, must-revalidate";

    #[tokio::test]
    async fn cache_control_on_404() {
        let response = router()
            .oneshot(
                Request::get("/unknown")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .expect("cache-control header present"),
            CACHE_CONTROL_VALUE
        );
    }

    #[tokio::test]
    async fn does_not_generate_trace_id() {
        let response = router()
            .oneshot(
                Request::get("/health")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");

        assert!(response.headers().get("x-trace-id").is_none());
    }

    #[tokio::test]
    async fn echoes_request_id() {
        let response = router()
            .oneshot(
                Request::get("/health")
                    .header("x-request-id", "my-custom-id")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");

        assert_eq!(
            response
                .headers()
                .get("x-request-id")
                .expect("x-request-id header present"),
            "my-custom-id"
        );
    }

    #[tokio::test]
    async fn echoes_trace_id() {
        let response = router()
            .oneshot(
                Request::get("/health")
                    .header("x-trace-id", "trace-123")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");

        assert_eq!(
            response
                .headers()
                .get("x-trace-id")
                .expect("x-trace-id header present"),
            "trace-123"
        );
    }

    #[tokio::test]
    async fn generates_request_id_when_missing() {
        let response = router()
            .oneshot(
                Request::get("/health")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");

        assert!(response.headers().get("x-request-id").is_some());
    }

    #[tokio::test]
    async fn put_conversion_returns_405_with_allow() {
        let response = router()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/conversion")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            response
                .headers()
                .get(header::ALLOW)
                .expect("allow header present"),
            "POST"
        );
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .expect("cache-control header present"),
            CACHE_CONTROL_VALUE
        );
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .expect("content-type header present"),
            "application/problem+json; charset=utf-8"
        );
    }
}
