//! HTTP router and route definitions.

use axum::{http::Request, Router};
use metrics_exporter_prometheus::PrometheusHandle;
use tower_http::request_id::{MakeRequestId, RequestId};

use crate::metrics::{metrics_handler, middleware::record_http_metrics};

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
    use axum::extract::DefaultBodyLimit;
    use axum::http::header::HeaderName;
    use axum::middleware;
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

    // Sentry-only imports — Body and Next are only used in the attach_sentry_tags
    // closure; option_layer and telemetry are only used in the sentry middleware stack.
    #[cfg(feature = "sentry")]
    use crate::telemetry;
    #[cfg(feature = "sentry")]
    use axum::body::Body;
    #[cfg(feature = "sentry")]
    use axum::middleware::Next;
    #[cfg(feature = "sentry")]
    use tower::util::option_layer;

    let x_request_id = HeaderName::from_static("x-request-id");
    let x_trace_id = HeaderName::from_static("x-trace-id");

    let conversion_route = post(post_conversion)
        .options(options_conversion)
        .fallback(conversion_method_not_allowed);

    let health_route = get(get_health)
        .head(head_health)
        .options(options_health)
        .fallback(health_method_not_allowed);

    #[cfg(feature = "sentry")]
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

    // Two cfg-exclusive service stacks — one that includes the sentry
    // hub-binding / HTTP layers and the tag middleware, one without.
    // Using two `let middleware_stack` bindings (mutually exclusive via cfg)
    // is the idiomatic Rust approach when each branch produces a different
    // ServiceBuilder type due to different layer counts.
    #[cfg(feature = "sentry")]
    let middleware_stack = ServiceBuilder::new()
        // Reason: axum's `Bytes`/`String`/`Json` extractors silently
        // cap request bodies at 2 MiB by default. The README pledges
        // `Body size: No limit ... DoS risk is accepted.`, so we
        // disable the cap globally. Without this layer the
        // `/conversion` handler would 413 on bodies >2 MiB.
        .layer(DefaultBodyLimit::disable())
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
        .layer(cache_control_layer());

    #[cfg(not(feature = "sentry"))]
    let middleware_stack = ServiceBuilder::new()
        // Reason: axum's `Bytes`/`String`/`Json` extractors silently
        // cap request bodies at 2 MiB by default. The README pledges
        // `Body size: No limit ... DoS risk is accepted.`, so we
        // disable the cap globally. Without this layer the
        // `/conversion` handler would 413 on bodies >2 MiB.
        .layer(DefaultBodyLimit::disable())
        .layer(SetRequestIdLayer::new(
            x_request_id.clone(),
            MakeRequestUuid,
        ))
        .layer(SetRequestIdLayer::new(x_trace_id.clone(), EchoOnly))
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(record_http_metrics))
        .layer(PropagateRequestIdLayer::new(x_request_id))
        .layer(PropagateRequestIdLayer::new(x_trace_id))
        .layer(cache_control_layer());

    Router::new()
        .route("/conversion", conversion_route)
        .route("/health", health_route)
        .fallback(not_found)
        .layer(middleware_stack)
}

/// Builds the public router and additionally registers `GET /metrics`
/// using the provided Prometheus handle.
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
    router().route("/metrics", axum::routing::get(metrics_handler(handle)))
}
