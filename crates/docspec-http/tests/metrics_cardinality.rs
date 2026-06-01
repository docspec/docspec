//! Cardinality regression tests for Prometheus metric label values.
//!
//! Pins the exact or bounded set of values each label can take. Any future
//! code change that introduces a new label value fails here, requiring explicit
//! acknowledgment and a deliberate plan update.

#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use std::collections::BTreeSet;

use axum::{body::Body, http::Request};
use docspec_http::{
    metrics::{build_recorder, METRIC_CONVERSIONS_TOTAL, METRIC_HTTP_REQUESTS_TOTAL},
    router::router_with_metrics,
};
use tower::ServiceExt as _;

fn extract_label_values(rendered: &str, metric_name: &str, label_key: &str) -> BTreeSet<String> {
    rendered
        .lines()
        .filter(|l| l.starts_with(metric_name) && !l.starts_with('#'))
        .filter_map(|l| {
            let key_eq = format!("{label_key}=\"");
            let start = l.find(&key_eq)?.checked_add(key_eq.len())?;
            let rest = l.get(start..)?;
            let end = rest.find('"')?.checked_add(start)?;
            Some(l.get(start..end)?.to_owned())
        })
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn path_label_cardinality_is_bounded_to_known_routes() {
    let (recorder, handle) = build_recorder().expect("recorder builds");
    let router = router_with_metrics(handle.clone());
    let _guard = metrics::set_default_local_recorder(&recorder);

    {
        let req = Request::builder()
            .method("GET")
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("HEAD")
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "text/markdown")
            .body(Body::from("# Hello"))
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("GET")
            .uri("/does-not-exist")
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    // Extra segments beyond the registered template do not extend it — still "unknown"
    {
        let req = Request::builder()
            .method("GET")
            .uri("/conversion/with/extra/segments")
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };

    let rendered = handle.render();
    let observed = extract_label_values(&rendered, METRIC_HTTP_REQUESTS_TOTAL, "path");

    let expected: BTreeSet<String> = ["/conversion", "/health", "unknown"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    assert_eq!(
        observed, expected,
        "path label set must be exactly {{/conversion, /health, unknown}}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn query_string_not_in_path_label() {
    let (recorder, handle) = build_recorder().expect("recorder builds");
    let router = router_with_metrics(handle.clone());
    let _guard = metrics::set_default_local_recorder(&recorder);

    let req = Request::builder()
        .method("GET")
        .uri("/health?secret=token&another=value")
        .body(Body::empty())
        .unwrap();
    router.oneshot(req).await.unwrap();

    let rendered = handle.render();

    assert!(
        !rendered.contains("secret=token"),
        "query param 'secret=token' must not appear in rendered metrics:\n{rendered}"
    );
    assert!(
        !rendered.contains("another=value"),
        "query param 'another=value' must not appear in rendered metrics:\n{rendered}"
    );
    assert!(
        !rendered.contains('?'),
        "query string delimiter '?' must not appear in rendered metrics:\n{rendered}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn error_class_cardinality_bounded() {
    let (recorder, handle) = build_recorder().expect("recorder builds");
    let router = router_with_metrics(handle.clone());
    let _guard = metrics::set_default_local_recorder(&recorder);

    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "text/markdown")
            .body(Body::from("# Hello"))
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "text/markdown")
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "text/markdown")
            .header("accept", "text/plain")
            .body(Body::from("# Hello"))
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };

    let rendered = handle.render();
    let observed = extract_label_values(&rendered, METRIC_CONVERSIONS_TOTAL, "error_class");

    let allowed: BTreeSet<String> = [
        "empty_body",
        "body_not_utf8",
        "internal",
        "method_not_allowed",
        "not_acceptable",
        "not_found",
        "unprocessable",
        "unsupported_media_type",
        "none",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();

    let unexpected: BTreeSet<_> = observed.difference(&allowed).collect();
    assert!(
        unexpected.is_empty(),
        "unexpected error_class values observed: {unexpected:?}\nallowed set: {allowed:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn result_label_cardinality_bounded() {
    let (recorder, handle) = build_recorder().expect("recorder builds");
    let router = router_with_metrics(handle.clone());
    let _guard = metrics::set_default_local_recorder(&recorder);

    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "text/markdown")
            .body(Body::from("# Hello"))
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "text/markdown")
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };

    let rendered = handle.render();
    let observed = extract_label_values(&rendered, METRIC_CONVERSIONS_TOTAL, "result");

    let allowed: BTreeSet<String> = ["success", "client_error", "server_error"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    let unexpected: BTreeSet<_> = observed.difference(&allowed).collect();
    assert!(
        unexpected.is_empty(),
        "unexpected result values observed: {unexpected:?}\nallowed set: {allowed:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn status_label_only_observed_codes() {
    let (recorder, handle) = build_recorder().expect("recorder builds");
    let router = router_with_metrics(handle.clone());
    let _guard = metrics::set_default_local_recorder(&recorder);

    {
        let req = Request::builder()
            .method("GET")
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    // HEAD /health is an explicitly registered handler returning 204, not the default 200
    {
        let req = Request::builder()
            .method("HEAD")
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "text/markdown")
            .body(Body::from("# Hello"))
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "text/markdown")
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("GET")
            .uri("/does-not-exist")
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("GET")
            .uri("/conversion")
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "text/markdown")
            .header("accept", "text/plain")
            .body(Body::from("# Hello"))
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };
    {
        let req = Request::builder()
            .method("POST")
            .uri("/conversion")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        router.clone().oneshot(req).await.unwrap()
    };

    let rendered = handle.render();
    let observed = extract_label_values(&rendered, METRIC_HTTP_REQUESTS_TOTAL, "status");

    let allowed: BTreeSet<String> = [
        "200", "204", "400", "404", "405", "406", "415", "422", "500",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();

    let unexpected: BTreeSet<_> = observed.difference(&allowed).collect();
    assert!(
        unexpected.is_empty(),
        "unexpected status values observed: {unexpected:?}\nallowed set: {allowed:?}"
    );
}
