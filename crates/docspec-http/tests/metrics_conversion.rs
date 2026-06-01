//! Integration tests for conversion outcome metrics.
//!
//! Each test uses a fresh, isolated Prometheus recorder via
//! [`docspec_http::metrics::build_recorder`] and [`metrics::with_local_recorder`].
//! No global recorder is installed.
//!
//! Tests are synchronous: each test creates its own
//! `tokio::runtime::Runtime` and drives the HTTP request inside
//! [`metrics::with_local_recorder`], so the thread-local recorder is active
//! throughout the full request — including after any `spawn_blocking` join.

// Reason: integration tests use standard test patterns with expect/unwrap.
#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

use axum::{body::Body, http::Request};
use tower::ServiceExt as _;

// ─── Test helpers ─────────────────────────────────────────────────────────────

fn make_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime builds")
}

fn valid_markdown_request(body: impl Into<Body>) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/conversion")
        .header("content-type", "text/markdown")
        .header("accept", "application/vnd.docspec.blocknote+json")
        .body(body.into())
        .unwrap()
}

/// Asserts that the Prometheus exposition text `rendered` contains a line that
/// is an exact string match of `expected_line`.
fn assert_metric_line(rendered: &str, expected_line: &str) {
    let found = rendered.lines().any(|line| line == expected_line);
    assert!(
        found,
        "Expected metric line not found.\n  expected: {expected_line}\n  rendered:\n{rendered}",
    );
}

// ─── Test 1: success ─────────────────────────────────────────────────────────

/// A successful conversion increments `docspec_conversions_total` with
/// `result="success"` and `error_class="none"`.
#[test]
fn success_increments_with_none_error_class() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(valid_markdown_request("# Hello World")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="success",error_class="none",input_mime_type="text/markdown",output_mime_type="application/vnd.docspec.blocknote+json"} 1"#,
    );
}

// ─── Test 2: empty body ───────────────────────────────────────────────────────

/// An empty request body increments `docspec_conversions_total` with
/// `result="client_error"` and `error_class="empty_body"`.
#[test]
fn empty_body_records_client_error() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(valid_markdown_request("")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="client_error",error_class="empty_body",input_mime_type="text/markdown",output_mime_type="none"} 1"#,
    );
}

// ─── Test 3: invalid UTF-8 ────────────────────────────────────────────────────

/// A request body with invalid UTF-8 bytes increments `docspec_conversions_total`
/// with `result="client_error"` and `error_class="body_not_utf8"`.
#[test]
fn invalid_utf8_records_client_error() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = Request::builder()
        .method("POST")
        .uri("/conversion")
        .header("content-type", "text/markdown")
        .header("accept", "application/vnd.docspec.blocknote+json")
        .body(Body::from(b"\xFF\xFE\x00".to_vec()))
        .unwrap();

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="client_error",error_class="body_not_utf8",input_mime_type="text/markdown",output_mime_type="none"} 1"#,
    );
}

// ─── Test 4: unsupported media type ──────────────────────────────────────────

/// A request with an unsupported `Content-Type` increments
/// `docspec_conversions_total` with `result="client_error"` and
/// `error_class="unsupported_media_type"`.
#[test]
fn unsupported_media_type_records_client_error() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = Request::builder()
        .method("POST")
        .uri("/conversion")
        .header("content-type", "application/json")
        .header("accept", "application/vnd.docspec.blocknote+json")
        .body(Body::from(r#"{"hello": "world"}"#))
        .unwrap();

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="client_error",error_class="unsupported_media_type",input_mime_type="unsupported",output_mime_type="none"} 1"#,
    );
}

// ─── Test 5: not acceptable ───────────────────────────────────────────────────

/// A request with a non-matching `Accept` header increments
/// `docspec_conversions_total` with `result="client_error"` and
/// `error_class="not_acceptable"`.
#[test]
fn not_acceptable_records_client_error() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = Request::builder()
        .method("POST")
        .uri("/conversion")
        .header("content-type", "text/markdown")
        .header("accept", "application/json")
        .body(Body::from("# Hello"))
        .unwrap();

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="client_error",error_class="not_acceptable",input_mime_type="text/markdown",output_mime_type="none"} 1"#,
    );
}

// ─── Test 6: duration on success ─────────────────────────────────────────────

/// A successful conversion records one observation in
/// `docspec_conversion_duration_seconds` with `result="success"`.
#[test]
fn conversion_duration_recorded_on_success() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(valid_markdown_request("# Hello")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversion_duration_seconds_count{result="success",input_mime_type="text/markdown",output_mime_type="application/vnd.docspec.blocknote+json"} 1"#,
    );
}

// ─── Test 7: duration on error ───────────────────────────────────────────────

/// A failed conversion (empty body) records one observation in
/// `docspec_conversion_duration_seconds` with `result="client_error"`.
#[test]
fn conversion_duration_recorded_on_error() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(valid_markdown_request("")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversion_duration_seconds_count{result="client_error",input_mime_type="text/markdown",output_mime_type="none"} 1"#,
    );
}

// ─── Test 8: body size ───────────────────────────────────────────────────────

/// The body size histogram records the exact byte count of the request body.
/// `b"# Hello"` is 7 bytes (`#`, ` `, `H`, `e`, `l`, `l`, `o`), so the
/// `docspec_http_request_body_bytes_sum` line must equal `7`.
#[test]
fn body_size_histogram_records_correct_byte_count() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    // "# Hello" = '#' + ' ' + 'H' + 'e' + 'l' + 'l' + 'o' = 7 bytes
    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(valid_markdown_request("# Hello")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_http_request_body_bytes_sum{input_mime_type="text/markdown"} 7"#,
    );
}

mod tracing_test_helpers {
    use core::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use tracing::field::{Field, Visit};
    use tracing::span::{Attributes, Record};
    use tracing::{Event, Id, Metadata, Subscriber};

    /// A captured tracing event with its fields as key-value string pairs.
    #[derive(Debug, Default, Clone)]
    pub struct CapturedEvent {
        pub fields: std::collections::HashMap<String, String>,
    }

    struct FieldVisitor<'a>(&'a mut CapturedEvent);

    impl Visit for FieldVisitor<'_> {
        fn record_bool(&mut self, field: &Field, value: bool) {
            self.0
                .fields
                .insert(field.name().to_owned(), value.to_string());
        }

        fn record_debug(&mut self, field: &Field, value: &dyn core::fmt::Debug) {
            self.0
                .fields
                .insert(field.name().to_owned(), format!("{value:?}"));
        }

        fn record_i64(&mut self, field: &Field, value: i64) {
            self.0
                .fields
                .insert(field.name().to_owned(), value.to_string());
        }

        fn record_str(&mut self, field: &Field, value: &str) {
            self.0
                .fields
                .insert(field.name().to_owned(), value.to_owned());
        }

        fn record_u64(&mut self, field: &Field, value: u64) {
            self.0
                .fields
                .insert(field.name().to_owned(), value.to_string());
        }
    }

    /// A minimal tracing subscriber that captures INFO-level events.
    pub struct CapturingSubscriber {
        events: Arc<Mutex<Vec<CapturedEvent>>>,
        next_span_id: AtomicU64,
    }

    impl CapturingSubscriber {
        pub fn new(events: Arc<Mutex<Vec<CapturedEvent>>>) -> Self {
            Self {
                events,
                next_span_id: AtomicU64::new(1),
            }
        }
    }

    impl Subscriber for CapturingSubscriber {
        fn enabled(&self, metadata: &Metadata<'_>) -> bool {
            metadata.is_event() && *metadata.level() <= tracing::Level::INFO
        }

        fn enter(&self, _span: &Id) {}

        fn event(&self, event: &Event<'_>) {
            let mut captured = CapturedEvent::default();
            event.record(&mut FieldVisitor(&mut captured));
            self.events.lock().unwrap().push(captured);
        }

        fn exit(&self, _span: &Id) {}

        fn new_span(&self, _attrs: &Attributes<'_>) -> Id {
            Id::from_u64(self.next_span_id.fetch_add(1, Ordering::Relaxed))
        }

        fn record(&self, _span: &Id, _values: &Record<'_>) {}

        fn record_follows_from(&self, _span: &Id, _follows: &Id) {}
    }
}

/// Asserts that a Prometheus metric line matching `prefix` has a numeric value
/// strictly greater than `threshold`.
fn assert_metric_value_gt(rendered: &str, prefix: &str, threshold: f64) {
    let line = rendered
        .lines()
        .find(|line| line.starts_with(prefix))
        .unwrap_or_else(|| {
            panic!(
                "Metric line with prefix not found.\n  prefix: {prefix}\n  rendered:\n{rendered}"
            )
        });
    let value_str = line.rsplit_once(' ').map_or_else(
        || panic!("Metric line has no space-separated value.\n  line: {line}"),
        |(_, v)| v,
    );
    let value: f64 = value_str
        .parse()
        .unwrap_or_else(|_| panic!("Metric value is not a float.\n  value: {value_str}"));
    assert!(
        value > threshold,
        "Expected metric value > {threshold}, got {value}.\n  line: {line}"
    );
}

// ─── Test 9: input_mime_type="unsupported" for non-markdown content type ──────

/// A request with `Content-Type: application/pdf` records
/// `input_mime_type="unsupported"` on `docspec_conversions_total`.
#[test]
fn input_mime_unsupported_when_other_content_type() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = Request::builder()
        .method("POST")
        .uri("/conversion")
        .header("content-type", "application/pdf")
        .header("accept", "application/vnd.docspec.blocknote+json")
        .body(Body::from("foo"))
        .unwrap();

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="client_error",error_class="unsupported_media_type",input_mime_type="unsupported",output_mime_type="none"} 1"#,
    );
}

// ─── Test 10: input_mime_type="none" when no Content-Type header ──────────────

/// A request with no `Content-Type` header records
/// `input_mime_type="none"` on `docspec_conversions_total`.
#[test]
fn input_mime_none_when_no_content_type() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = Request::builder()
        .method("POST")
        .uri("/conversion")
        .header("accept", "application/vnd.docspec.blocknote+json")
        .body(Body::from("# x"))
        .unwrap();

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="client_error",error_class="unsupported_media_type",input_mime_type="none",output_mime_type="none"} 1"#,
    );
}

// ─── Test 11: output_bytes recorded on success ────────────────────────────────

/// A successful conversion records one observation in
/// `docspec_conversion_output_bytes` with a positive sum.
#[test]
fn output_bytes_recorded_on_success() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(valid_markdown_request("# Hello")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversion_output_bytes_count{input_mime_type="text/markdown",output_mime_type="application/vnd.docspec.blocknote+json"} 1"#,
    );
    assert_metric_value_gt(
        &rendered,
        r#"docspec_conversion_output_bytes_sum{input_mime_type="text/markdown",output_mime_type="application/vnd.docspec.blocknote+json"}"#,
        0.0,
    );
}

// ─── Test 12: output_bytes NOT recorded on empty body error ───────────────────

/// A failed conversion (empty body) does NOT record any observation in
/// `docspec_conversion_output_bytes`.
#[test]
fn output_bytes_not_recorded_on_error() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(valid_markdown_request("")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert!(
        !rendered
            .lines()
            .any(|l| l.starts_with("docspec_conversion_output_bytes")),
        "Expected no docspec_conversion_output_bytes lines on error, but found some.\n  rendered:\n{rendered}",
    );
}

// ─── Test 13: output_bytes NOT recorded on unsupported media type ─────────────

/// A failed conversion (unsupported media type) does NOT record any observation
/// in `docspec_conversion_output_bytes`.
#[test]
fn output_bytes_not_recorded_on_unsupported_media() {
    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = Request::builder()
        .method("POST")
        .uri("/conversion")
        .header("content-type", "application/pdf")
        .header("accept", "application/vnd.docspec.blocknote+json")
        .body(Body::from("foo"))
        .unwrap();

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert!(
        !rendered
            .lines()
            .any(|l| l.starts_with("docspec_conversion_output_bytes")),
        "Expected no docspec_conversion_output_bytes lines on error, but found some.\n  rendered:\n{rendered}",
    );
}

// ─── Test 14: tracing event emitted on success ────────────────────────────────

/// A successful conversion emits a `conversion_completed` tracing event with
/// all required fields.
#[test]
fn tracing_event_emitted_on_success() {
    use std::sync::{Arc, Mutex};
    use tracing_test_helpers::{CapturedEvent, CapturingSubscriber};

    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let events: Arc<Mutex<Vec<CapturedEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let subscriber = CapturingSubscriber::new(Arc::clone(&events));

    tracing::subscriber::with_default(subscriber, || {
        metrics::with_local_recorder(&recorder, || {
            rt.block_on(router.oneshot(valid_markdown_request("# Hello")))
        })
        .expect("oneshot succeeds");
    });

    let captured = events.lock().unwrap();
    let conversion_event = captured
        .iter()
        .find(|e| e.fields.get("event").map(String::as_str) == Some("conversion_completed"))
        .expect("conversion_completed event was emitted");

    assert_eq!(
        conversion_event.fields.get("result").map(String::as_str),
        Some("success"),
        "result field"
    );
    assert_eq!(
        conversion_event
            .fields
            .get("input_mime_type")
            .map(String::as_str),
        Some("text/markdown"),
        "input_mime_type field"
    );
    assert_eq!(
        conversion_event
            .fields
            .get("output_mime_type")
            .map(String::as_str),
        Some("application/vnd.docspec.blocknote+json"),
        "output_mime_type field"
    );
    assert!(
        conversion_event.fields.contains_key("output_bytes"),
        "output_bytes field present"
    );
    assert!(
        conversion_event.fields.contains_key("input_bytes"),
        "input_bytes field present"
    );
    assert!(
        conversion_event.fields.contains_key("duration_ms"),
        "duration_ms field present"
    );
    assert!(
        conversion_event.fields.contains_key("request_id"),
        "request_id field present"
    );
}

// ─── Test 15: tracing event emitted on error ──────────────────────────────────

/// A failed conversion (empty body) emits a `conversion_completed` tracing
/// event with `result="client_error"` and `output_bytes=0`.
#[test]
fn tracing_event_emitted_on_error() {
    use std::sync::{Arc, Mutex};
    use tracing_test_helpers::{CapturedEvent, CapturingSubscriber};

    let rt = make_runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let events: Arc<Mutex<Vec<CapturedEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let subscriber = CapturingSubscriber::new(Arc::clone(&events));

    tracing::subscriber::with_default(subscriber, || {
        metrics::with_local_recorder(&recorder, || {
            rt.block_on(router.oneshot(valid_markdown_request("")))
        })
        .expect("oneshot succeeds");
    });

    let captured = events.lock().unwrap();
    let conversion_event = captured
        .iter()
        .find(|e| e.fields.get("event").map(String::as_str) == Some("conversion_completed"))
        .expect("conversion_completed event was emitted");

    assert_eq!(
        conversion_event.fields.get("result").map(String::as_str),
        Some("client_error"),
        "result field"
    );
    assert_eq!(
        conversion_event
            .fields
            .get("error_class")
            .map(String::as_str),
        Some("empty_body"),
        "error_class field"
    );
    assert_eq!(
        conversion_event
            .fields
            .get("input_mime_type")
            .map(String::as_str),
        Some("text/markdown"),
        "input_mime_type field"
    );
    assert_eq!(
        conversion_event
            .fields
            .get("output_mime_type")
            .map(String::as_str),
        Some("none"),
        "output_mime_type field"
    );
    assert_eq!(
        conversion_event
            .fields
            .get("output_bytes")
            .map(String::as_str),
        Some("0"),
        "output_bytes=0 on error"
    );
}
