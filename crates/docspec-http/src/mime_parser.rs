//! HTTP `Accept` negotiation and `Content-Type` validation for the conversion API.

use axum::http::HeaderValue;
use docspec::{InputFormat, OutputFormat};

use crate::error::HttpError;
use crate::format::{
    OUTPUT_MIME_ALIAS, OUTPUT_MIME_HTML_PRIMARY, OUTPUT_MIME_OXA_PRIMARY,
    OUTPUT_MIME_PANDOC_NATIVE_PRIMARY, OUTPUT_MIME_PRIMARY,
};

/// Negotiates the `Accept` header for the `/conversion` endpoint.
///
/// Returns [`OutputFormat::Oxa`] for `Accept: application/vnd.oxa+json`,
/// [`OutputFormat::PandocNative`] for `Accept: application/vnd.pandoc.native`,
/// [`OutputFormat::Html`] for `Accept: text/html`, and [`OutputFormat::Blocknote`]
/// for the `BlockNote` MIMEs, `application/*`, `*/*`, and missing `Accept`.
/// Wildcards default to `BlockNote` for back-compat with pre-oxa clients. When
/// `Accept` lists multiple types, the first whose bare MIME matches a supported
/// value wins (case-insensitive); `q=...` is stripped.
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
        if type_part.eq_ignore_ascii_case(OUTPUT_MIME_PANDOC_NATIVE_PRIMARY) {
            return Ok(OutputFormat::PandocNative);
        }
        if type_part.eq_ignore_ascii_case(OUTPUT_MIME_HTML_PRIMARY) {
            return Ok(OutputFormat::Html);
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

/// Validates the `Content-Type` header for the `/conversion` endpoint and
/// resolves it to the matching reader format.
///
/// Accepts `text/markdown` ([`InputFormat::Markdown`]) and `text/html`
/// ([`InputFormat::Html`]), each with no charset, or with `charset=utf-8`
/// (case-insensitive). Any other charset is rejected — the handler always
/// decodes the body as UTF-8, so a non-UTF-8 charset is unsupportable.
/// Returns `Err` if the header is missing, malformed, the MIME type is
/// neither `text/markdown` nor `text/html`, the charset is anything other
/// than `utf-8`, or an unknown parameter is present.
///
/// # Errors
///
/// Returns [`HttpError::UnsupportedMediaType`] with the received value (or `None` if missing).
#[inline]
pub fn validate_content_type(header_value: Option<&HeaderValue>) -> Result<InputFormat, HttpError> {
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
    let format = match (parsed.type_(), parsed.subtype().as_str()) {
        (mime::TEXT, "markdown") => InputFormat::Markdown,
        (mime::TEXT, "html") => InputFormat::Html,
        _ => {
            return Err(HttpError::UnsupportedMediaType {
                received: Some(header_str.to_owned()),
            });
        }
    };
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
    Ok(format)
}

/// Returns the bounded `input_mime_type` label value for a `Content-Type` header.
///
/// This function is intentionally MORE permissive than [`validate_content_type`]:
/// it returns the matching label for any `text/markdown` or `text/html` value
/// regardless of charset or other parameters, because the label answers
/// "what did the client try to send?" rather than "is it valid?".
///
/// # Label values
///
/// - [`crate::metrics::INPUT_MIME_NONE`] — header absent
/// - [`crate::metrics::INPUT_MIME_MARKDOWN`] — `text/markdown` (any params)
/// - [`crate::metrics::INPUT_MIME_HTML`] — `text/html` (any params)
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
    match (parsed.type_(), parsed.subtype().as_str()) {
        (mime::TEXT, "markdown") => crate::metrics::INPUT_MIME_MARKDOWN,
        (mime::TEXT, "html") => crate::metrics::INPUT_MIME_HTML,
        _ => crate::metrics::INPUT_MIME_UNSUPPORTED,
    }
}

/// Returns the bounded `output_mime_type` label value for a conversion outcome.
///
/// `chosen_format` is `None` for any error path, and `Some(format)` on success.
#[inline]
#[must_use]
pub fn bucket_output_mime(chosen_format: Option<OutputFormat>) -> &'static str {
    match chosen_format {
        Some(OutputFormat::Blocknote) => crate::metrics::OUTPUT_MIME_BLOCKNOTE,
        Some(OutputFormat::Html) => crate::metrics::OUTPUT_MIME_HTML,
        Some(OutputFormat::Oxa) => crate::metrics::OUTPUT_MIME_OXA,
        Some(OutputFormat::PandocNative) => crate::metrics::OUTPUT_MIME_PANDOC_NATIVE,
        None | Some(_) => crate::metrics::OUTPUT_MIME_NONE,
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
    fn bucket_input_mime_html_when_text_html() {
        let val = HeaderValue::from_static("text/html");
        assert_eq!(
            bucket_input_mime(Some(&val)),
            crate::metrics::INPUT_MIME_HTML
        );
    }

    #[test]
    fn bucket_input_mime_html_when_text_html_with_charset() {
        let val = HeaderValue::from_static("text/html; charset=utf-8");
        assert_eq!(
            bucket_input_mime(Some(&val)),
            crate::metrics::INPUT_MIME_HTML
        );
    }

    #[test]
    fn bucket_input_mime_html_case_insensitive() {
        let val = HeaderValue::from_static("TEXT/HTML");
        assert_eq!(
            bucket_input_mime(Some(&val)),
            crate::metrics::INPUT_MIME_HTML
        );
    }

    #[test]
    fn bucket_input_mime_html_with_non_utf8_charset_still_buckets_html() {
        let val = HeaderValue::from_static("text/html; charset=iso-8859-1");
        assert_eq!(
            bucket_input_mime(Some(&val)),
            crate::metrics::INPUT_MIME_HTML
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
    fn bucket_output_mime_html_when_html_succeeded() {
        assert_eq!(
            bucket_output_mime(Some(OutputFormat::Html)),
            crate::metrics::OUTPUT_MIME_HTML
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
    fn bucket_output_mime_pandoc_native_when_pandoc_native_succeeded() {
        assert_eq!(
            bucket_output_mime(Some(OutputFormat::PandocNative)),
            crate::metrics::OUTPUT_MIME_PANDOC_NATIVE
        );
    }

    #[test]
    fn bucket_output_mime_none_when_no_format_chosen() {
        assert_eq!(bucket_output_mime(None), crate::metrics::OUTPUT_MIME_NONE);
    }
}
