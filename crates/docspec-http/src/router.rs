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
    use axum::body::Body;
    use axum::http::header::HeaderName;
    use axum::middleware::{self, Next};
    use axum::routing::{get, post};
    use tower::util::option_layer;
    use tower::ServiceBuilder;
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
                .layer(option_layer(telemetry::tower_new_layer()))
                .layer(option_layer(telemetry::tower_http_layer()))
                .layer(middleware::from_fn(attach_sentry_tags))
                .layer(TraceLayer::new_for_http())
                .layer(PropagateRequestIdLayer::new(x_request_id))
                .layer(PropagateRequestIdLayer::new(x_trace_id))
                .layer(cache_control_layer()),
        )
}
