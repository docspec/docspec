//! Prometheus metrics for the `docspec-http` server.
//!
//! Each pod exposes its own `/metrics` endpoint. The Prometheus server
//! scrapes each pod independently — no aggregation happens in-process.
//!
//! # Design
//!
//! This module owns all metric-name and label constants. Middleware (see
//! [`middleware`]) records values; this module only declares and describes
//! them. Call [`install_global`] once at startup before accepting requests,
//! then mount the closure returned by [`metrics_handler`] at `/metrics`.

pub mod middleware;

use axum::http::header::CONTENT_TYPE;
use metrics::{describe_counter, describe_histogram};
use metrics_exporter_prometheus::{
    BuildError, Matcher, PrometheusBuilder, PrometheusHandle, PrometheusRecorder,
};

// ─── Metric-name constants ─────────────────────────────────────────────────

/// Counter: total number of HTTP requests received.
pub const METRIC_HTTP_REQUESTS_TOTAL: &str = "docspec_http_requests_total";

/// Histogram: HTTP request latency in seconds.
pub const METRIC_HTTP_REQUEST_DURATION_SECONDS: &str = "docspec_http_request_duration_seconds";

/// Histogram: HTTP request body size in bytes.
pub const METRIC_HTTP_REQUEST_BODY_BYTES: &str = "docspec_http_request_body_bytes";

/// Counter: total number of document conversions.
pub const METRIC_CONVERSIONS_TOTAL: &str = "docspec_conversions_total";

/// Histogram: document conversion duration in seconds.
pub const METRIC_CONVERSION_DURATION_SECONDS: &str = "docspec_conversion_duration_seconds";

/// Histogram: document conversion output size in bytes.
pub const METRIC_CONVERSION_OUTPUT_BYTES: &str = "docspec_conversion_output_bytes";

// ─── Label key constants ───────────────────────────────────────────────────

/// Label key for the HTTP request method (GET, POST, …).
pub const LABEL_METHOD: &str = "method";

/// Label key for the matched route path pattern.
pub const LABEL_PATH: &str = "path";

/// Label key for the HTTP response status code.
pub const LABEL_STATUS: &str = "status";

/// Label key for the conversion outcome category.
pub const LABEL_RESULT: &str = "result";

/// Label key for the error class when a conversion fails.
pub const LABEL_ERROR_CLASS: &str = "error_class";

/// Label key for the input MIME type of the conversion request.
pub const LABEL_INPUT_MIME_TYPE: &str = "input_mime_type";

/// Label key for the output MIME type produced by the conversion.
pub const LABEL_OUTPUT_MIME_TYPE: &str = "output_mime_type";

// ─── Label value constants ─────────────────────────────────────────────────

/// Value for [`LABEL_PATH`] when no route was matched by the router.
pub const PATH_UNKNOWN: &str = "unknown";

/// Value for [`LABEL_RESULT`] when a conversion succeeds.
pub const RESULT_SUCCESS: &str = "success";

/// Value for [`LABEL_RESULT`] when a conversion fails due to a client error.
pub const RESULT_CLIENT_ERROR: &str = "client_error";

/// Value for [`LABEL_RESULT`] when a conversion fails due to a server error.
pub const RESULT_SERVER_ERROR: &str = "server_error";

/// Value for [`LABEL_ERROR_CLASS`] when no error occurred.
pub const ERROR_CLASS_NONE: &str = "none";

/// Value for [`LABEL_INPUT_MIME_TYPE`] when the request was text/markdown.
pub const INPUT_MIME_MARKDOWN: &str = "text/markdown";

/// Value for [`LABEL_INPUT_MIME_TYPE`] when the request was text/html.
pub const INPUT_MIME_HTML: &str = "text/html";

/// Value for [`LABEL_INPUT_MIME_TYPE`] when the Content-Type header was present but not a supported reader format.
pub const INPUT_MIME_UNSUPPORTED: &str = "unsupported";

/// Value for [`LABEL_INPUT_MIME_TYPE`] when the Content-Type header was absent.
pub const INPUT_MIME_NONE: &str = "none";

/// Value for [`LABEL_OUTPUT_MIME_TYPE`] when the conversion produced `BlockNote` JSON.
pub const OUTPUT_MIME_BLOCKNOTE: &str = "application/vnd.docspec.blocknote+json";

/// Value for [`LABEL_OUTPUT_MIME_TYPE`] when the conversion produced `oxa.dev` JSON.
pub const OUTPUT_MIME_OXA: &str = "application/vnd.oxa+json";

/// Value for [`LABEL_OUTPUT_MIME_TYPE`] when no output was produced (any error path).
pub const OUTPUT_MIME_NONE: &str = "none";

// ─── Histogram bucket arrays ───────────────────────────────────────────────

/// Latency histogram buckets for HTTP request duration, in seconds.
pub const HTTP_LATENCY_BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Body-size histogram buckets for HTTP request bodies, in bytes.
pub const HTTP_BODY_SIZE_BUCKETS: [f64; 12] = [
    100.0, 200.0, 400.0, 800.0, 1_600.0, 3_200.0, 6_400.0, 12_800.0, 25_600.0, 51_200.0, 102_400.0,
    204_800.0,
];

/// Latency histogram buckets for document conversion duration, in seconds.
///
/// Shares the same breakpoints as [`HTTP_LATENCY_BUCKETS`].
pub const CONVERSION_DURATION_BUCKETS: [f64; 11] = HTTP_LATENCY_BUCKETS;

/// Size histogram buckets for conversion output bytes.
///
/// Shares the same breakpoints as [`HTTP_BODY_SIZE_BUCKETS`] for symmetry
/// between request body size and conversion output size in dashboards.
pub const CONVERSION_OUTPUT_BYTES_BUCKETS: [f64; 12] = HTTP_BODY_SIZE_BUCKETS;

// ─── Recorder builder ──────────────────────────────────────────────────────

fn configure_buckets(builder: PrometheusBuilder) -> Result<PrometheusBuilder, BuildError> {
    builder
        .set_buckets_for_metric(
            Matcher::Full(METRIC_HTTP_REQUEST_DURATION_SECONDS.to_owned()),
            &HTTP_LATENCY_BUCKETS,
        )?
        .set_buckets_for_metric(
            Matcher::Full(METRIC_HTTP_REQUEST_BODY_BYTES.to_owned()),
            &HTTP_BODY_SIZE_BUCKETS,
        )?
        .set_buckets_for_metric(
            Matcher::Full(METRIC_CONVERSION_DURATION_SECONDS.to_owned()),
            &CONVERSION_DURATION_BUCKETS,
        )?
        .set_buckets_for_metric(
            Matcher::Full(METRIC_CONVERSION_OUTPUT_BYTES.to_owned()),
            &CONVERSION_OUTPUT_BYTES_BUCKETS,
        )
}

/// Builds a configured [`PrometheusRecorder`] and its [`PrometheusHandle`]
/// **without** installing it globally.
///
/// Use this in tests together with [`metrics::with_local_recorder`] to keep
/// test runs isolated from one another and from the global recorder.
///
/// # Errors
///
/// Returns [`BuildError`] when bucket values are empty or otherwise invalid.
#[inline]
pub fn build_recorder() -> Result<(PrometheusRecorder, PrometheusHandle), BuildError> {
    let recorder = configure_buckets(PrometheusBuilder::new())?.build_recorder();
    let handle = recorder.handle();
    Ok((recorder, handle))
}

/// Installs the Prometheus recorder globally and registers HELP text for every
/// metric.
///
/// Call **once** at server startup before accepting requests. Returns the
/// [`PrometheusHandle`] needed to scrape the `/metrics` endpoint.
///
/// # Errors
///
/// Returns [`BuildError`] when bucket configuration or recorder installation
/// fails (e.g., a global recorder is already installed).
#[inline]
pub fn install_global() -> Result<PrometheusHandle, BuildError> {
    let handle = configure_buckets(PrometheusBuilder::new())?.install_recorder()?;

    describe_counter!(METRIC_HTTP_REQUESTS_TOTAL, "Total HTTP requests received.");
    describe_histogram!(
        METRIC_HTTP_REQUEST_DURATION_SECONDS,
        "HTTP request latency in seconds."
    );
    describe_histogram!(
        METRIC_HTTP_REQUEST_BODY_BYTES,
        "HTTP request body size in bytes, labeled by input MIME type."
    );
    describe_counter!(
        METRIC_CONVERSIONS_TOTAL,
        "Total document conversions, labeled by result, error class, and input/output MIME type."
    );
    describe_histogram!(
        METRIC_CONVERSION_DURATION_SECONDS,
        "Document conversion duration in seconds, labeled by result and input/output MIME type."
    );
    describe_histogram!(
        METRIC_CONVERSION_OUTPUT_BYTES,
        "Document conversion output size in bytes (recorded only on successful conversions), labeled by input/output MIME type."
    );

    Ok(handle)
}

// ─── Metrics endpoint handler ──────────────────────────────────────────────

/// Returns a clone-safe handler closure that renders Prometheus metrics as an
/// HTTP response with the correct Content-Type.
///
/// Mount the returned closure at `/metrics`. The response Content-Type is
/// `text/plain; version=0.0.4; charset=utf-8` as required by the Prometheus
/// exposition format specification.
#[inline]
pub fn metrics_handler(
    handle: PrometheusHandle,
) -> impl Fn() -> core::future::Ready<axum::response::Response> + Clone + Send + 'static {
    move || {
        let output = handle.render();
        core::future::ready(
            axum::http::Response::builder()
                .header(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")
                .body(axum::body::Body::from(output))
                .unwrap_or_else(|_| {
                    let mut response = axum::response::Response::new(axum::body::Body::from(
                        "failed to render metrics response",
                    ));
                    *response.status_mut() = axum::http::StatusCode::INTERNAL_SERVER_ERROR;
                    response
                }),
        )
    }
}

#[cfg(test)]
mod handler_tests {
    #![allow(
        clippy::tests_outside_test_module,
        clippy::unwrap_used,
        clippy::expect_used
    )]

    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn metrics_handler_returns_200() {
        let (recorder, handle) = build_recorder().expect("test recorder builds");
        let result = metrics::with_local_recorder(&recorder, || {
            let handler = metrics_handler(handle);
            handler()
        });
        let response = result.await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn metrics_handler_content_type_is_prometheus_text() {
        let (recorder, handle) = build_recorder().expect("test recorder builds");
        let result = metrics::with_local_recorder(&recorder, || {
            let handler = metrics_handler(handle);
            handler()
        });
        let response = result.await;
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .expect("content-type header present")
            .to_str()
            .expect("content-type is valid str");
        assert_eq!(content_type, "text/plain; version=0.0.4; charset=utf-8");
    }

    #[tokio::test]
    async fn metrics_handler_body_is_valid_utf8() {
        let (recorder, handle) = build_recorder().expect("test recorder builds");
        let result = metrics::with_local_recorder(&recorder, || {
            let handler = metrics_handler(handle);
            handler()
        });
        let response = result.await;
        let body_bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body readable");
        let _body_str = String::from_utf8(body_bytes.to_vec()).expect("body is valid UTF-8");
    }
}
