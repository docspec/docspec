//! HTTP router and route definitions.

use axum::{http::Request, Router};
use metrics_exporter_prometheus::PrometheusHandle;
use tower_http::request_id::{MakeRequestId, RequestId};

use crate::metrics::{metrics_handler, middleware::record_http_metrics};

/// Default request body size limit for the `/conversion` route: 10 MiB.
pub const DEFAULT_BODY_LIMIT_BYTES: usize = 10 * 1024 * 1024;

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

/// Build the HTTP API router using the default body size limit.
///
/// Equivalent to `router_with_body_limit(DEFAULT_BODY_LIMIT_BYTES)`.
#[inline]
pub fn router() -> Router {
    router_with_body_limit(DEFAULT_BODY_LIMIT_BYTES)
}

/// Build the HTTP API router with a custom `/conversion` body size limit.
///
/// `body_limit_bytes` is applied only to the `/conversion` route via
/// [`tower_http::limit::RequestBodyLimitLayer`]. Bodies exceeding the limit
/// are rejected with HTTP 413.
#[inline]
pub fn router_with_body_limit(body_limit_bytes: usize) -> Router {
    use axum::body::Body;
    use axum::http::header::HeaderName;
    use axum::middleware::{self, Next};
    use axum::routing::{get, post};
    use tower::util::option_layer;
    use tower::ServiceBuilder;
    use tower_http::limit::RequestBodyLimitLayer;
    use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
    use tower_http::trace::TraceLayer;

    use crate::cache::cache_control_layer;
    use crate::handlers::{
        conversion::{options_conversion, post_conversion},
        fallback::{conversion_method_not_allowed, health_method_not_allowed, not_found},
        health::{get_health, head_health, options_health},
    };
    use crate::telemetry;

    let attach_sentry_tags = |request: Request<Body>, next: Next| async move {
        if sentry::Hub::current().client().is_some() {
            let request_id = request
                .extensions()
                .get::<RequestId>()
                .and_then(|req_id| req_id.header_value().to_str().ok())
                .unwrap_or_default()
                .to_owned();
            let trace_id = request
                .headers()
                .get("x-trace-id")
                .and_then(|head| head.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            sentry::configure_scope(|scope| {
                if !request_id.is_empty() {
                    scope.set_tag("request_id", &request_id);
                }
                if !trace_id.is_empty() {
                    scope.set_tag("trace_id", &trace_id);
                }
            });
        }
        next.run(request).await
    };

    let x_request_id = HeaderName::from_static("x-request-id");
    let x_trace_id = HeaderName::from_static("x-trace-id");

    let conversion_route = post(post_conversion)
        .options(options_conversion)
        .fallback(conversion_method_not_allowed)
        .layer(RequestBodyLimitLayer::new(body_limit_bytes));

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
                .layer(option_layer(telemetry::tower_new_layer()))
                .layer(option_layer(telemetry::tower_http_layer()))
                .layer(middleware::from_fn(attach_sentry_tags))
                .layer(TraceLayer::new_for_http())
                .layer(middleware::from_fn(record_http_metrics))
                .layer(PropagateRequestIdLayer::new(x_request_id))
                .layer(PropagateRequestIdLayer::new(x_trace_id))
                .layer(cache_control_layer()),
        )
}

/// Builds the public router with the default body limit and additionally registers
/// `GET /metrics` using the provided Prometheus handle.
///
/// The `/metrics` route is intentionally registered **after** the
/// `ServiceBuilder` layer stack so it does NOT inherit:
///   - `cache_control_layer` (Prometheus scrapers ignore cache headers,
///     but a clean exposition response is preferred)
///   - request-id / trace-id propagation
///   - sentry hub binding
///   - the HTTP metrics middleware (double-skip protection — even if
///     someone removes the `MatchedPath == "/metrics"` guard in
///     `record_http_metrics`, the router topology still prevents the
///     scrape from incrementing counters).
#[inline]
pub fn router_with_metrics(handle: PrometheusHandle) -> Router {
    router_with_metrics_and_body_limit(handle, DEFAULT_BODY_LIMIT_BYTES)
}

/// Builds the public router with a custom body limit and additionally registers
/// `GET /metrics` using the provided Prometheus handle.
///
/// See [`router_with_metrics`] for details on the middleware exclusions that apply
/// to the `/metrics` route.
#[inline]
pub fn router_with_metrics_and_body_limit(
    handle: PrometheusHandle,
    body_limit_bytes: usize,
) -> Router {
    router_with_body_limit(body_limit_bytes)
        .route("/metrics", axum::routing::get(metrics_handler(handle)))
}
