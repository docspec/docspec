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

use axum::{body::Body, Router};
use docspec_http::{
    metrics::{build_recorder, METRIC_CONVERSIONS_TOTAL, METRIC_HTTP_REQUESTS_TOTAL},
    router::router_with_metrics,
};
use metrics_exporter_prometheus::PrometheusHandle;
use tower::ServiceExt as _;

const EMPTY_HEADERS: &[(&str, &str)] = &[];
const JSON_HEADERS: &[(&str, &str)] = &[("content-type", "application/json")];
const MARKDOWN_HEADERS: &[(&str, &str)] = &[("content-type", "text/markdown")];
const TEXT_ACCEPT_HEADERS: &[(&str, &str)] =
    &[("content-type", "text/markdown"), ("accept", "text/plain")];

#[derive(Clone, Copy)]
enum RequestBody {
    Empty,
    Text(&'static str),
}

impl RequestBody {
    fn into_body(self) -> Body {
        match self {
            Self::Empty => Body::empty(),
            Self::Text(body) => Body::from(body),
        }
    }
}

#[derive(Clone, Copy)]
struct RequestCase {
    body: RequestBody,
    headers: &'static [(&'static str, &'static str)],
    method: &'static str,
    uri: &'static str,
}

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

fn router_with_local_recorder() -> (
    metrics_exporter_prometheus::PrometheusRecorder,
    PrometheusHandle,
    Router,
) {
    let (recorder, handle) = build_recorder().expect("recorder builds");
    let router = router_with_metrics(handle.clone());
    (recorder, handle, router)
}

async fn send_case(router: Router, case: RequestCase) {
    let request = common::request(case.method, case.uri, case.headers, case.body.into_body());
    router.oneshot(request).await.unwrap();
}

async fn send_cases(router: &Router, cases: &[RequestCase]) {
    for case in cases {
        send_case(router.clone(), *case).await;
    }
}

fn allowed_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|s| (*s).to_owned()).collect()
}

#[tokio::test(flavor = "current_thread")]
async fn path_label_cardinality_is_bounded_to_known_routes() {
    let (recorder, handle, router) = router_with_local_recorder();
    let _guard = metrics::set_default_local_recorder(&recorder);

    send_cases(
        &router,
        &[
            RequestCase {
                body: RequestBody::Empty,
                headers: EMPTY_HEADERS,
                method: "GET",
                uri: "/health",
            },
            RequestCase {
                body: RequestBody::Empty,
                headers: EMPTY_HEADERS,
                method: "HEAD",
                uri: "/health",
            },
            RequestCase {
                body: RequestBody::Text("# Hello"),
                headers: MARKDOWN_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
            RequestCase {
                body: RequestBody::Empty,
                headers: EMPTY_HEADERS,
                method: "GET",
                uri: "/does-not-exist",
            },
            // Extra segments beyond the registered template do not extend it — still "unknown".
            RequestCase {
                body: RequestBody::Empty,
                headers: EMPTY_HEADERS,
                method: "GET",
                uri: "/conversion/with/extra/segments",
            },
        ],
    )
    .await;

    let rendered = handle.render();
    let observed = extract_label_values(&rendered, METRIC_HTTP_REQUESTS_TOTAL, "path");
    let expected = allowed_set(&["/conversion", "/health", "unknown"]);

    assert_eq!(
        observed, expected,
        "path label set must be exactly {{/conversion, /health, unknown}}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn query_string_not_in_path_label() {
    let (recorder, handle, router) = router_with_local_recorder();
    let _guard = metrics::set_default_local_recorder(&recorder);

    send_case(
        router,
        RequestCase {
            body: RequestBody::Empty,
            headers: EMPTY_HEADERS,
            method: "GET",
            uri: "/health?secret=token&another=value",
        },
    )
    .await;

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
    let (recorder, handle, router) = router_with_local_recorder();
    let _guard = metrics::set_default_local_recorder(&recorder);

    send_cases(
        &router,
        &[
            RequestCase {
                body: RequestBody::Text("# Hello"),
                headers: MARKDOWN_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
            RequestCase {
                body: RequestBody::Empty,
                headers: MARKDOWN_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
            RequestCase {
                body: RequestBody::Text("{}"),
                headers: JSON_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
            RequestCase {
                body: RequestBody::Text("# Hello"),
                headers: TEXT_ACCEPT_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
        ],
    )
    .await;

    let rendered = handle.render();
    let observed = extract_label_values(&rendered, METRIC_CONVERSIONS_TOTAL, "error_class");
    let allowed = allowed_set(&[
        "empty_body",
        "body_not_utf8",
        "internal",
        "method_not_allowed",
        "not_acceptable",
        "not_found",
        "unprocessable",
        "unsupported_media_type",
        "none",
    ]);

    let unexpected: BTreeSet<_> = observed.difference(&allowed).collect();
    assert!(
        unexpected.is_empty(),
        "unexpected error_class values observed: {unexpected:?}\nallowed set: {allowed:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn result_label_cardinality_bounded() {
    let (recorder, handle, router) = router_with_local_recorder();
    let _guard = metrics::set_default_local_recorder(&recorder);

    send_cases(
        &router,
        &[
            RequestCase {
                body: RequestBody::Text("# Hello"),
                headers: MARKDOWN_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
            RequestCase {
                body: RequestBody::Empty,
                headers: MARKDOWN_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
            RequestCase {
                body: RequestBody::Text("{}"),
                headers: JSON_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
        ],
    )
    .await;

    let rendered = handle.render();
    let observed = extract_label_values(&rendered, METRIC_CONVERSIONS_TOTAL, "result");
    let allowed = allowed_set(&["success", "client_error", "server_error"]);

    let unexpected: BTreeSet<_> = observed.difference(&allowed).collect();
    assert!(
        unexpected.is_empty(),
        "unexpected result values observed: {unexpected:?}\nallowed set: {allowed:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn status_label_only_observed_codes() {
    let (recorder, handle, router) = router_with_local_recorder();
    let _guard = metrics::set_default_local_recorder(&recorder);

    send_cases(
        &router,
        &[
            RequestCase {
                body: RequestBody::Empty,
                headers: EMPTY_HEADERS,
                method: "GET",
                uri: "/health",
            },
            // HEAD /health is an explicitly registered handler returning 204, not the default 200.
            RequestCase {
                body: RequestBody::Empty,
                headers: EMPTY_HEADERS,
                method: "HEAD",
                uri: "/health",
            },
            RequestCase {
                body: RequestBody::Text("# Hello"),
                headers: MARKDOWN_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
            RequestCase {
                body: RequestBody::Empty,
                headers: MARKDOWN_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
            RequestCase {
                body: RequestBody::Empty,
                headers: EMPTY_HEADERS,
                method: "GET",
                uri: "/does-not-exist",
            },
            RequestCase {
                body: RequestBody::Empty,
                headers: EMPTY_HEADERS,
                method: "GET",
                uri: "/conversion",
            },
            RequestCase {
                body: RequestBody::Text("# Hello"),
                headers: TEXT_ACCEPT_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
            RequestCase {
                body: RequestBody::Text("{}"),
                headers: JSON_HEADERS,
                method: "POST",
                uri: "/conversion",
            },
        ],
    )
    .await;

    let rendered = handle.render();
    let observed = extract_label_values(&rendered, METRIC_HTTP_REQUESTS_TOTAL, "status");
    let allowed = allowed_set(&[
        "200", "204", "400", "404", "405", "406", "415", "422", "500",
    ]);

    let unexpected: BTreeSet<_> = observed.difference(&allowed).collect();
    assert!(
        unexpected.is_empty(),
        "unexpected status values observed: {unexpected:?}\nallowed set: {allowed:?}"
    );
}
