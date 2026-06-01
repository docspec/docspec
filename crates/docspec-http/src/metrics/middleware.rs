//! Tower middleware for recording HTTP-level Prometheus metrics.
//!
//! This middleware records [`crate::metrics::METRIC_HTTP_REQUESTS_TOTAL`] and
//! [`crate::metrics::METRIC_HTTP_REQUEST_DURATION_SECONDS`] for every request
//! EXCEPT requests to `/metrics` itself (which would create a feedback loop).

use std::time::Instant;

use axum::{body::Body, extract::MatchedPath, http::Request, middleware::Next, response::Response};

use crate::metrics::{
    LABEL_METHOD, LABEL_PATH, LABEL_STATUS, METRIC_HTTP_REQUESTS_TOTAL,
    METRIC_HTTP_REQUEST_DURATION_SECONDS, PATH_UNKNOWN,
};

/// The route path for the Prometheus metrics endpoint.
/// Requests to this path are excluded from HTTP metrics recording
/// to prevent a feedback loop on every scrape.
const METRICS_ROUTE_PATH: &str = "/metrics";

/// Tower middleware that records HTTP request metrics.
///
/// Records [`METRIC_HTTP_REQUESTS_TOTAL`] and [`METRIC_HTTP_REQUEST_DURATION_SECONDS`]
/// for every request except those to `/metrics` (to avoid a scrape feedback loop).
///
/// The `path` label uses [`axum::extract::MatchedPath`] (route template, never raw URI).
/// Fallback routes (404, 405) that have no matched path use the literal `"unknown"`.
#[inline]
pub async fn record_http_metrics(request: Request<Body>, next: Next) -> Response {
    // Extract matched path BEFORE consuming the request
    let matched_path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|matched: &MatchedPath| matched.as_str().to_owned());

    // Skip recording for /metrics itself to prevent feedback loop
    if matched_path.as_deref() == Some(METRICS_ROUTE_PATH) {
        return next.run(request).await;
    }

    let method = request.method().to_string();
    let path_label = matched_path.unwrap_or_else(|| PATH_UNKNOWN.to_owned());

    let started_at = Instant::now();
    let response = next.run(request).await;
    let duration_seconds = started_at.elapsed().as_secs_f64();

    let status_label = response.status().as_u16().to_string();

    metrics::counter!(
        METRIC_HTTP_REQUESTS_TOTAL,
        LABEL_METHOD => method.clone(),
        LABEL_PATH => path_label.clone(),
        LABEL_STATUS => status_label.clone()
    )
    .increment(1);

    metrics::histogram!(
        METRIC_HTTP_REQUEST_DURATION_SECONDS,
        LABEL_METHOD => method,
        LABEL_PATH => path_label,
        LABEL_STATUS => status_label
    )
    .record(duration_seconds);

    response
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::tests_outside_test_module,
        clippy::unwrap_used,
        clippy::expect_used
    )]

    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt as _;

    #[test]
    fn metrics_route_path_is_metrics() {
        assert_eq!(METRICS_ROUTE_PATH, "/metrics");
    }

    /// Verify that the `/metrics` early-return guard (line 38) fires when the
    /// middleware is mounted ON the `/metrics` route.
    ///
    /// In production, `router_with_metrics` adds `/metrics` OUTSIDE the
    /// middleware layer stack, so the guard is never reached — it is
    /// defense-in-depth. This test exercises the guard directly by placing
    /// the route inside the stack, which is the only way to reach that branch.
    #[test]
    fn metrics_path_early_return_skips_recording() {
        let (recorder, handle) = crate::metrics::build_recorder().expect("test recorder builds");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime builds");

        // Mount /metrics INSIDE the middleware so the guard is exercised.
        let router = Router::new()
            .route("/metrics", get(|| async { "ok" }))
            .layer(middleware::from_fn(record_http_metrics));

        let request = Request::builder()
            .method("GET")
            .uri("/metrics")
            .body(Body::empty())
            .expect("request builds");

        let response = metrics::with_local_recorder(&recorder, || {
            runtime
                .block_on(router.oneshot(request))
                .expect("oneshot succeeds")
        });

        assert_eq!(response.status(), StatusCode::OK);

        let rendered = handle.render();
        assert!(
            !rendered.contains(crate::metrics::METRIC_HTTP_REQUESTS_TOTAL),
            "Expected no request counter for /metrics path; got:\n{rendered}"
        );
    }
}
