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

mod common;

use axum::{body::Body, http::Request};
use tower::ServiceExt as _;

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
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("# Hello World")))
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
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("")))
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
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = common::accepted_markdown_request(Body::from(b"\xFF\xFE\x00".to_vec()));

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
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", "application/json"),
            ("accept", "application/vnd.docspec.blocknote+json"),
        ],
        Body::from(r#"{"hello": "world"}"#),
    );

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
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", "text/markdown"),
            ("accept", "application/json"),
        ],
        Body::from("# Hello"),
    );

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
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("# Hello")))
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
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("")))
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
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    // "# Hello" = '#' + ' ' + 'H' + 'e' + 'l' + 'l' + 'o' = 7 bytes
    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("# Hello")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_http_request_body_bytes_sum{input_mime_type="text/markdown"} 7"#,
    );
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
    let rt = common::runtime();
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
    let rt = common::runtime();
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
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("# Hello")))
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
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("")))
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
    let rt = common::runtime();
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

// ─── Test 14: HTML success records HTML output_mime_type ─────────────────────

/// A successful conversion to HTML records `output_mime_type="text/html"`
/// on `docspec_conversions_total`.
#[test]
fn html_success_records_html_output_mime() {
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = Request::builder()
        .method("POST")
        .uri("/conversion")
        .header("content-type", "text/markdown")
        .header("accept", "text/html")
        .body(Body::from("Hello"))
        .unwrap();

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="success",error_class="none",input_mime_type="text/markdown",output_mime_type="text/html"} 1"#,
    );
}

// ─── Test 15: oxa success records oxa output_mime_type ───────────────────────

/// A successful conversion to oxa records `output_mime_type="application/vnd.oxa+json"`
/// on `docspec_conversions_total`.
#[test]
fn oxa_success_records_oxa_output_mime() {
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = Request::builder()
        .method("POST")
        .uri("/conversion")
        .header("content-type", "text/markdown")
        .header("accept", "application/vnd.oxa+json")
        .body(Body::from("Hello"))
        .unwrap();

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="success",error_class="none",input_mime_type="text/markdown",output_mime_type="application/vnd.oxa+json"} 1"#,
    );
}

// ─── Test 16: Pandoc native success records pandoc output_mime_type ───────────

/// A successful conversion to Pandoc native records
/// `output_mime_type="application/vnd.pandoc.native"` on
/// `docspec_conversions_total`.
#[test]
fn pandoc_native_success_records_pandoc_native_output_mime() {
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = Request::builder()
        .method("POST")
        .uri("/conversion")
        .header("content-type", "text/markdown")
        .header("accept", "application/vnd.pandoc.native")
        .body(Body::from("Hello"))
        .unwrap();

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="success",error_class="none",input_mime_type="text/markdown",output_mime_type="application/vnd.pandoc.native"} 1"#,
    );
}

// ─── Test 17: DOCX success increments conversions_total ──────────────────────

/// A successful DOCX conversion increments `docspec_conversions_total` with
/// the full DOCX MIME as `input_mime_type` and `result="success"`.
#[test]
fn docx_success_increments_conversions_total() {
    let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#;
    let bytes = common::synth_docx(common::SIMPLE_RELS, doc_xml);

    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::docx_request(bytes)))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="success",error_class="none",input_mime_type="application/vnd.openxmlformats-officedocument.wordprocessingml.document",output_mime_type="application/vnd.docspec.blocknote+json"} 1"#,
    );
}

// ─── Test 18: DOCX input records DOCX input_mime_type ────────────────────────

/// A successful DOCX conversion records the full DOCX MIME as `input_mime_type`
/// in the rendered Prometheus metrics.
#[test]
fn metrics_label_for_docx_input() {
    let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#;
    let bytes = common::synth_docx(common::SIMPLE_RELS, doc_xml);

    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::docx_request(bytes)))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert!(
        rendered
            .contains("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "DOCX MIME should appear in metrics: {rendered}"
    );
}
