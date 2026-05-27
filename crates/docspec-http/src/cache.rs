//! Cache-Control middleware for all HTTP responses.

use axum::http::{header::CACHE_CONTROL, HeaderValue};
use tower_http::set_header::SetResponseHeaderLayer;

/// Returns a middleware layer that adds `Cache-Control: max-age=0, private, must-revalidate`
/// to every response, overriding any existing Cache-Control header.
///
/// This prevents intermediate proxies and clients from caching conversion results
/// or health check responses.
#[inline]
pub fn cache_control_layer() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        CACHE_CONTROL,
        HeaderValue::from_static("max-age=0, private, must-revalidate"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use tower::ServiceExt as _;

    #[allow(clippy::single_call_fn)]
    // Reason: handler is defined once per test for clarity
    async fn ok_handler() -> &'static str {
        "ok"
    }

    #[allow(clippy::single_call_fn)]
    // Reason: handler is defined once per test for clarity
    async fn err_handler() -> (axum::http::StatusCode, &'static str) {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "error")
    }

    #[tokio::test]
    #[allow(clippy::expect_used)]
    // Reason: test setup requires valid request construction; failure indicates test bug
    async fn adds_header_to_success() {
        let app = Router::new()
            .route("/", get(ok_handler))
            .layer(cache_control_layer());
        let response = app
            .oneshot(
                Request::get("/")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .expect("cache-control header present"),
            "max-age=0, private, must-revalidate"
        );
    }

    #[tokio::test]
    #[allow(clippy::expect_used)]
    // Reason: test setup requires valid request construction; failure indicates test bug
    async fn adds_header_to_error_response() {
        let app = Router::new()
            .route("/", get(err_handler))
            .layer(cache_control_layer());
        let response = app
            .oneshot(
                Request::get("/")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("request succeeds");
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .expect("cache-control header present"),
            "max-age=0, private, must-revalidate"
        );
    }
}
