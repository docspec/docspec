//! Document conversion request handlers.

use axum::{
    body::{Body, Bytes},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
    response::IntoResponse,
};
use docspec::{InputFormat, OutputFormat};
use docspec_core::{EventSink as _, EventSource as _};

use crate::{error::HttpError, mime_parser};

enum Utf8Prevalidation {
    Required,
    SkippedBinary,
    SkippedFutureFormat,
}

/// Handle `OPTIONS /conversion` — returns allowed methods.
#[allow(clippy::unused_async)]
// Reason: Axum handlers are async for route consistency even when no await is needed.
#[inline]
pub async fn options_conversion() -> impl IntoResponse {
    (
        StatusCode::NO_CONTENT,
        [(header::ALLOW, HeaderValue::from_static("POST, OPTIONS"))],
    )
}

/// Handle `POST /conversion` — convert markdown or HTML to `BlockNote` JSON, HTML, `oxa.dev` JSON, or Pandoc native.
///
/// The input reader is selected by the request's `Content-Type` header (see
/// [`crate::mime_parser::validate_content_type`]). The output writer is
/// selected by the request's `Accept` header (see
/// [`crate::mime_parser::negotiate_accept`]).
///
/// The request body is buffered, then converted to completion inside
/// `spawn_blocking`, then returned in a single response. Conversion errors
/// surface as 422 (parse / sink errors) or 500 (finalize errors) **before**
/// any response body is sent — no truncated `200 OK` on failure.
///
/// `request_id` is accepted as `Option<Extension<RequestId>>` so the handler
/// remains usable for downstream consumers that mount it standalone without
/// the [`tower_http::request_id::SetRequestIdLayer`]. When the extension is
/// absent, the `request_id` field is **omitted** from the structured
/// `conversion_completed` event rather than logged as an empty string —
/// "no correlation id supplied" is a distinct state from "supplied empty".
/// The same treatment applies to `trace_id`, which is only set when the
/// upstream `X-Trace-ID` header is present.
///
/// Conversion outcome metrics are recorded with intentionally different scopes:
///
/// - `docspec_conversions_total` and `docspec_conversion_duration_seconds` are
///   recorded for **every** request to this endpoint — including early
///   validation failures — so that all outcomes are visible in dashboards.
/// - `docspec_http_request_body_bytes` is recorded only after `Content-Type`
///   and `Accept` validation pass and the body is confirmed non-empty.
/// - `docspec_conversion_output_bytes` is recorded only on successful
///   conversions (failed conversions produce no output).
///
/// # Errors
///
/// Returns [`HttpError`] when request headers or body are invalid, the
/// conversion fails, or the response cannot be constructed.
#[inline]
pub async fn post_conversion(
    request_id: Option<axum::extract::Extension<tower_http::request_id::RequestId>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response<Body>, HttpError> {
    let input_mime_label = crate::mime_parser::bucket_input_mime(headers.get(header::CONTENT_TYPE));
    let trace_id_owned: Option<String> = headers
        .get(axum::http::HeaderName::from_static("x-trace-id"))
        .and_then(|header_value| header_value.to_str().ok())
        .map(str::to_owned);
    let body_len_for_logging = body.len();

    let conversion_start = std::time::Instant::now();
    let outcome = do_conversion(input_mime_label, headers, body).await;
    let conversion_duration = conversion_start.elapsed();
    let conversion_duration_secs = conversion_duration.as_secs_f64();
    let conversion_duration_ms =
        u64::try_from(conversion_duration.as_millis().min(u128::from(u64::MAX)))
            .unwrap_or(u64::MAX);

    let (response_or_error, output_bytes, chosen_format) = match outcome {
        Ok((response, bytes, format)) => (Ok(response), bytes, Some(format)),
        Err(http_error) => (Err(http_error), 0, None),
    };
    let conversion_ok = response_or_error.is_ok();
    let output_mime_label = crate::mime_parser::bucket_output_mime(chosen_format);

    let (result_label, error_class_label) = match &response_or_error {
        Ok(_) => (
            crate::metrics::RESULT_SUCCESS,
            crate::metrics::ERROR_CLASS_NONE,
        ),
        Err(http_error) => (http_error.result_class(), http_error.error_class()),
    };

    metrics::counter!(
        crate::metrics::METRIC_CONVERSIONS_TOTAL,
        crate::metrics::LABEL_RESULT => result_label,
        crate::metrics::LABEL_ERROR_CLASS => error_class_label,
        crate::metrics::LABEL_INPUT_MIME_TYPE => input_mime_label,
        crate::metrics::LABEL_OUTPUT_MIME_TYPE => output_mime_label,
    )
    .increment(1);

    metrics::histogram!(
        crate::metrics::METRIC_CONVERSION_DURATION_SECONDS,
        crate::metrics::LABEL_RESULT => result_label,
        crate::metrics::LABEL_INPUT_MIME_TYPE => input_mime_label,
        crate::metrics::LABEL_OUTPUT_MIME_TYPE => output_mime_label,
    )
    .record(conversion_duration_secs);

    if conversion_ok {
        // Reason: u64 → f64 is lossy at extreme values but bounded by realistic output sizes.
        #[allow(clippy::cast_precision_loss, clippy::as_conversions)]
        let output_bytes_f64 = output_bytes as f64;
        metrics::histogram!(
            crate::metrics::METRIC_CONVERSION_OUTPUT_BYTES,
            crate::metrics::LABEL_INPUT_MIME_TYPE => input_mime_label,
            crate::metrics::LABEL_OUTPUT_MIME_TYPE => output_mime_label,
        )
        .record(output_bytes_f64);
    }

    let request_id_opt: Option<&str> = request_id
        .as_ref()
        .and_then(|axum::extract::Extension(req_id)| req_id.header_value().to_str().ok());
    tracing::info!(
        event = "conversion_completed",
        result = result_label,
        error_class = error_class_label,
        input_mime_type = input_mime_label,
        output_mime_type = output_mime_label,
        input_bytes = body_len_for_logging,
        output_bytes,
        duration_ms = conversion_duration_ms,
        request_id = request_id_opt,
        trace_id = trace_id_owned.as_deref(),
    );

    response_or_error
}

/// Perform the actual validation and conversion without recording outcome metrics.
///
/// Body size is recorded here because it is only known after header validation
/// succeeds and the body is confirmed non-empty.
async fn do_conversion(
    input_mime_label: &'static str,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(Response<Body>, u64, OutputFormat), HttpError> {
    let input_format = mime_parser::validate_content_type(headers.get(header::CONTENT_TYPE))?;
    let output_format = mime_parser::negotiate_accept(headers.get(header::ACCEPT))?;

    if body.is_empty() {
        return Err(HttpError::EmptyBody);
    }

    // Reason: Body sizes are bounded by request memory and never approach the 2^53
    // f64 precision limit, so the cast is exact in practice. The Prometheus histogram
    // API requires f64; usize has no native lossless f64 conversion. Workspace
    // clippy bans both lints below as a general policy; this is the single
    // documented false-positive exception for bounded numeric metric recording.
    #[allow(clippy::cast_precision_loss, clippy::as_conversions)]
    let body_len_bytes = body.len() as f64;
    metrics::histogram!(
        crate::metrics::METRIC_HTTP_REQUEST_BODY_BYTES,
        crate::metrics::LABEL_INPUT_MIME_TYPE => input_mime_label,
    )
    .record(body_len_bytes);

    let utf8_prevalidation = match input_format {
        InputFormat::Markdown | InputFormat::Html => Utf8Prevalidation::Required,
        InputFormat::Docx => {
            // Binary format; UTF-8 prevalidation does not apply.
            Utf8Prevalidation::SkippedBinary
        }
        _ => {
            // Future InputFormat variants. Conservative policy: skip UTF-8
            // prevalidation and let the reader surface its own error.
            // Update this arm deliberately when a new format is added.
            Utf8Prevalidation::SkippedFutureFormat
        }
    };

    if matches!(utf8_prevalidation, Utf8Prevalidation::Required) {
        core::str::from_utf8(&body).map_err(|_error| {
            tracing::debug!("request body is not valid UTF-8");
            HttpError::BodyNotUtf8
        })?;
    }

    let join_result = tokio::task::spawn_blocking(move || -> Result<(Vec<u8>, u64), HttpError> {
        let mut output_buffer = Vec::new();
        let mut reader = docspec::AnyReader::from_reader(input_format, std::io::Cursor::new(body))
            .map_err(|error| {
                tracing::debug!(error = %error, "reader construction failed");
                HttpError::Unprocessable {
                    detail: error.to_string(),
                }
            })?;
        let mut sink = docspec::AnyWriter::new(output_format, &mut output_buffer);

        loop {
            match reader.next_event() {
                Ok(Some(event)) => sink.handle_event(event).map_err(|error| {
                    tracing::debug!(error = %error, "conversion sink failed");
                    HttpError::Unprocessable {
                        detail: error.to_string(),
                    }
                })?,
                Ok(None) => break,
                Err(error) => {
                    tracing::debug!(error = %error, "reader failed");
                    return Err(HttpError::Unprocessable {
                        detail: error.to_string(),
                    });
                }
            }
        }

        sink.finish().map_err(|error| {
            tracing::debug!(error = %error, "conversion sink finish failed");
            HttpError::Internal
        })?;

        // Capture byte count before output_buffer is consumed by Body::from.
        // u64::try_from is lossless on 64-bit targets (usize ≤ u64::MAX).
        let output_bytes =
            u64::try_from(output_buffer.len()).map_err(|_conversion_error| HttpError::Internal)?;
        Ok((output_buffer, output_bytes))
    })
    .await;

    let content_type = match output_format {
        OutputFormat::Blocknote => {
            HeaderValue::from_static("application/vnd.docspec.blocknote+json; charset=utf-8")
        }
        OutputFormat::Html => HeaderValue::from_static("text/html; charset=utf-8"),
        OutputFormat::Oxa => HeaderValue::from_static("application/vnd.oxa+json; charset=utf-8"),
        OutputFormat::PandocNative => {
            HeaderValue::from_static("application/vnd.pandoc.native; charset=utf-8")
        }
        _ => HeaderValue::from_static("application/vnd.docspec.blocknote+json; charset=utf-8"),
    };

    match join_result {
        Ok(Ok((output, output_bytes))) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(output))
            .map(|response| (response, output_bytes, output_format))
            .map_err(|error| {
                tracing::error!(error = %error, "failed to build conversion response");
                HttpError::Internal
            }),
        Ok(Err(http_error)) => Err(http_error),
        Err(join_error) => {
            tracing::error!(error = %join_error, "spawn_blocking join failed");
            Err(HttpError::Internal)
        }
    }
}
