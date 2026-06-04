//! HTTP `Accept` negotiation and `Content-Type` validation for the conversion API.

use axum::http::HeaderValue;
use docspec::OutputFormat;

use crate::error::HttpError;
use crate::format::{OUTPUT_MIME_ALIAS, OUTPUT_MIME_OXA_PRIMARY, OUTPUT_MIME_PRIMARY};

/// Negotiates the `Accept` header for the `/conversion` endpoint.
///
/// Returns [`OutputFormat::Oxa`] for `Accept: application/vnd.oxa+json`, and
/// [`OutputFormat::Blocknote`] for the `BlockNote` MIMEs, `application/*`, `*/*`, and
/// missing `Accept`. Wildcards default to `BlockNote` for back-compat with pre-oxa
/// clients. When `Accept` lists multiple types, the first whose bare MIME matches a
/// supported value wins (case-insensitive); `q=...` is stripped.
///
/// # Errors
///
/// Returns [`HttpError::NotAcceptable`] if no acceptable MIME type found.
#[inline]
pub fn negotiate_accept(header_value: Option<&HeaderValue>) -> Result<OutputFormat, HttpError> {
    // Missing Accept == */* per RFC 7231 §5.3.2
    let Some(header_val) = header_value else {
        return Ok(OutputFormat::Blocknote);
    };
    let header_str = header_val
        .to_str()
        .map_err(|_err| HttpError::NotAcceptable)?;

    for part in header_str.split(',') {
        let type_part = part.trim().split(';').next().map_or("", str::trim);
        if type_part.eq_ignore_ascii_case(OUTPUT_MIME_OXA_PRIMARY) {
            return Ok(OutputFormat::Oxa);
        }
        if type_part.eq_ignore_ascii_case("*/*")
            || type_part.eq_ignore_ascii_case("application/*")
            || type_part.eq_ignore_ascii_case(OUTPUT_MIME_PRIMARY)
            || type_part.eq_ignore_ascii_case(OUTPUT_MIME_ALIAS)
        {
            return Ok(OutputFormat::Blocknote);
        }
    }
    Err(HttpError::NotAcceptable)
}

/// Validates the `Content-Type` header for the `/conversion` endpoint.
///
/// Accepts `text/markdown` with no charset, or `text/markdown; charset=utf-8`
/// (case-insensitive). Any other charset is rejected — the handler always
/// decodes the body as UTF-8, so a non-UTF-8 charset is unsupportable.
/// Returns `Err` if the header is missing, malformed, the MIME type is not
/// `text/markdown`, or the charset is anything other than `utf-8`.
///
/// # Errors
///
/// Returns [`HttpError::UnsupportedMediaType`] with the received value (or `None` if missing).
#[inline]
pub fn validate_content_type(header_value: Option<&HeaderValue>) -> Result<(), HttpError> {
    let Some(header_val) = header_value else {
        return Err(HttpError::UnsupportedMediaType { received: None });
    };
    let header_str = header_val
        .to_str()
        .ok()
        .ok_or_else(|| HttpError::UnsupportedMediaType {
            received: Some("<invalid header value>".to_owned()),
        })?;
    let parsed: mime::Mime =
        header_str
            .parse()
            .ok()
            .ok_or_else(|| HttpError::UnsupportedMediaType {
                received: Some(header_str.to_owned()),
            })?;
    if parsed.type_() != mime::TEXT || parsed.subtype().as_str() != "markdown" {
        return Err(HttpError::UnsupportedMediaType {
            received: Some(header_str.to_owned()),
        });
    }
    if let Some(charset) = parsed.get_param(mime::CHARSET) {
        if !charset.as_str().eq_ignore_ascii_case("utf-8") {
            return Err(HttpError::UnsupportedMediaType {
                received: Some(header_str.to_owned()),
            });
        }
    }
    // Strict: only the optional charset parameter is allowed. Unknown params
    // (e.g. `boundary`, `format`) cause 415 to prevent accidental acceptance
    // of unrelated media types that happen to share the text/markdown prefix.
    for (name, _) in parsed.params() {
        if name != mime::CHARSET {
            return Err(HttpError::UnsupportedMediaType {
                received: Some(header_str.to_owned()),
            });
        }
    }
    Ok(())
}

/// Returns the bounded `input_mime_type` label value for a `Content-Type` header.
///
/// This function is intentionally MORE permissive than [`validate_content_type`]:
/// it returns [`crate::metrics::INPUT_MIME_MARKDOWN`] for any `text/markdown`
/// value regardless of charset or other parameters, because the label answers
/// "what did the client try to send?" rather than "is it valid?".
///
/// # Label values
///
/// - [`crate::metrics::INPUT_MIME_NONE`] — header absent
/// - [`crate::metrics::INPUT_MIME_MARKDOWN`] — `text/markdown` (any params)
/// - [`crate::metrics::INPUT_MIME_UNSUPPORTED`] — anything else
#[must_use]
#[inline]
pub fn bucket_input_mime(header_value: Option<&HeaderValue>) -> &'static str {
    let Some(header_val) = header_value else {
        return crate::metrics::INPUT_MIME_NONE;
    };
    let Ok(header_str) = header_val.to_str() else {
        return crate::metrics::INPUT_MIME_UNSUPPORTED;
    };
    let Ok(parsed) = header_str.parse::<mime::Mime>() else {
        return crate::metrics::INPUT_MIME_UNSUPPORTED;
    };
    if parsed.type_() == mime::TEXT && parsed.subtype().as_str() == "markdown" {
        crate::metrics::INPUT_MIME_MARKDOWN
    } else {
        crate::metrics::INPUT_MIME_UNSUPPORTED
    }
}

/// Returns the bounded `output_mime_type` label value for a conversion outcome.
///
/// `chosen_format` is `None` for any error path, and `Some(format)` on success.
#[inline]
#[must_use]
pub fn bucket_output_mime(chosen_format: Option<OutputFormat>) -> &'static str {
    match chosen_format {
        None => crate::metrics::OUTPUT_MIME_NONE,
        Some(OutputFormat::Blocknote) => crate::metrics::OUTPUT_MIME_BLOCKNOTE,
        Some(OutputFormat::Oxa) => crate::metrics::OUTPUT_MIME_OXA,
    }
}

#[cfg(test)]
mod bucket_tests {
    #![allow(
        clippy::tests_outside_test_module,
        clippy::unwrap_used,
        clippy::expect_used
    )]

    use super::*;
    use axum::http::HeaderValue;

    // ─── bucket_input_mime tests ───────────────────────────────────────────

    #[test]
    fn bucket_input_mime_none_when_header_absent() {
        assert_eq!(bucket_input_mime(None), crate::metrics::INPUT_MIME_NONE);
    }

    #[test]
    fn bucket_input_mime_markdown_when_text_markdown() {
        let val = HeaderValue::from_static("text/markdown");
        assert_eq!(
            bucket_input_mime(Some(&val)),
            crate::metrics::INPUT_MIME_MARKDOWN
        );
    }

    #[test]
    fn bucket_input_mime_markdown_when_text_markdown_with_charset() {
        let val = HeaderValue::from_static("text/markdown; charset=utf-8");
        assert_eq!(
            bucket_input_mime(Some(&val)),
            crate::metrics::INPUT_MIME_MARKDOWN
        );
    }

    #[test]
    fn bucket_input_mime_markdown_case_insensitive() {
        let val = HeaderValue::from_static("TEXT/MARKDOWN");
        assert_eq!(
            bucket_input_mime(Some(&val)),
            crate::metrics::INPUT_MIME_MARKDOWN
        );
    }

    #[test]
    fn bucket_input_mime_unsupported_when_other_format() {
        let val = HeaderValue::from_static("application/pdf");
        assert_eq!(
            bucket_input_mime(Some(&val)),
            crate::metrics::INPUT_MIME_UNSUPPORTED
        );
    }

    #[test]
    fn bucket_input_mime_unsupported_when_malformed() {
        let val = HeaderValue::from_static("not a mime type at all");
        assert_eq!(
            bucket_input_mime(Some(&val)),
            crate::metrics::INPUT_MIME_UNSUPPORTED
        );
    }

    #[test]
    fn bucket_input_mime_unsupported_when_non_ascii() {
        let val = HeaderValue::from_bytes(&[0xFF, 0xFE]).unwrap();
        assert_eq!(
            bucket_input_mime(Some(&val)),
            crate::metrics::INPUT_MIME_UNSUPPORTED
        );
    }

    // ─── bucket_output_mime tests ──────────────────────────────────────────

    #[test]
    fn bucket_output_mime_blocknote_when_blocknote_succeeded() {
        assert_eq!(
            bucket_output_mime(Some(OutputFormat::Blocknote)),
            crate::metrics::OUTPUT_MIME_BLOCKNOTE
        );
    }

    #[test]
    fn bucket_output_mime_oxa_when_oxa_succeeded() {
        assert_eq!(
            bucket_output_mime(Some(OutputFormat::Oxa)),
            crate::metrics::OUTPUT_MIME_OXA
        );
    }

    #[test]
    fn bucket_output_mime_none_when_no_format_chosen() {
        assert_eq!(bucket_output_mime(None), crate::metrics::OUTPUT_MIME_NONE);
    }
}
