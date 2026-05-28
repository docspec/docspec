//! HTTP `Accept` negotiation and `Content-Type` validation for the conversion API.

use axum::http::HeaderValue;

use crate::error::HttpError;
use crate::format::{OUTPUT_MIME_ALIAS, OUTPUT_MIME_PRIMARY};

/// Negotiates the `Accept` header for the `/conversion` endpoint.
///
/// Returns the primary output MIME type string for:
/// - Missing `Accept` header (HTTP default is `*/*`)
/// - `Accept: */*`
/// - `Accept: application/*`
/// - `Accept: application/vnd.docspec.blocknote+json`
/// - `Accept: application/vnd.blocknote+json` (alias)
///
/// Returns `Err(HttpError::NotAcceptable)` for any other value.
///
/// Quality parameters (`q=...`) are stripped and ignored.
///
/// # Errors
///
/// Returns [`HttpError::NotAcceptable`] if no acceptable MIME type found.
#[inline]
pub fn negotiate_accept(header_value: Option<&HeaderValue>) -> Result<&'static str, HttpError> {
    // Missing Accept == */* per RFC 7231 §5.3.2
    let Some(header_val) = header_value else {
        return Ok(OUTPUT_MIME_PRIMARY);
    };
    let header_str = header_val
        .to_str()
        .map_err(|_err| HttpError::NotAcceptable)?;

    for part in header_str.split(',') {
        let type_part = part.trim().split(';').next().map_or("", str::trim);
        if type_part.eq_ignore_ascii_case("*/*")
            || type_part.eq_ignore_ascii_case("application/*")
            || type_part.eq_ignore_ascii_case(OUTPUT_MIME_PRIMARY)
            || type_part.eq_ignore_ascii_case(OUTPUT_MIME_ALIAS)
        {
            return Ok(OUTPUT_MIME_PRIMARY);
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
