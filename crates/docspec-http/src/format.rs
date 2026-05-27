//! MIME type constants and HTTP header negotiation for the conversion API.

use axum::http::HeaderValue;

use crate::error::HttpError;

/// Cache-Control header value applied to all responses.
pub const CACHE_CONTROL_VALUE: &str = "max-age=0, private, must-revalidate";

/// Health endpoint response body.
pub const HEALTH_BODY: &str = "Healthy.";

/// Health endpoint Content-Type header value.
pub const HEALTH_CONTENT_TYPE: &str = "text/plain; charset=utf-8";

/// The MIME type accepted as request body.
pub const INPUT_MIME_MARKDOWN: &str = "text/markdown";

/// Accepted alias for the output MIME type (input-only; server always returns primary).
pub const OUTPUT_MIME_ALIAS: &str = "application/vnd.blocknote+json";

/// Full output Content-Type header value including charset.
pub const OUTPUT_MIME_FULL: &str = "application/vnd.docspec.blocknote+json; charset=utf-8";

/// The primary output MIME type returned on success.
pub const OUTPUT_MIME_PRIMARY: &str = "application/vnd.docspec.blocknote+json";

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
    let Some(hv) = header_value else {
        return Ok(OUTPUT_MIME_PRIMARY);
    };
    let s = hv.to_str().ok().ok_or(HttpError::NotAcceptable)?;

    for part in s.split(',') {
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
/// Accepts `text/markdown` with any charset parameter.
/// Returns `Err` if the header is missing, malformed, or a different MIME type.
///
/// # Errors
///
/// Returns [`HttpError::UnsupportedMediaType`] with the received value (or `None` if missing).
#[inline]
pub fn validate_content_type(header_value: Option<&HeaderValue>) -> Result<(), HttpError> {
    let Some(hv) = header_value else {
        return Err(HttpError::UnsupportedMediaType { received: None });
    };
    let s = hv
        .to_str()
        .ok()
        .ok_or_else(|| HttpError::UnsupportedMediaType {
            received: Some("<invalid UTF-8>".to_owned()),
        })?;
    let mime: mime::Mime = s
        .parse()
        .ok()
        .ok_or_else(|| HttpError::UnsupportedMediaType {
            received: Some(s.to_owned()),
        })?;
    if mime.type_() == mime::TEXT && mime.subtype().as_str() == "markdown" {
        Ok(())
    } else {
        Err(HttpError::UnsupportedMediaType {
            received: Some(s.to_owned()),
        })
    }
}

#[cfg(test)]
mod tests {
    // Reason: test code legitimately uses unwrap for asserting expected-Ok results;
    // panicking here indicates a test bug, not a runtime error.
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn content_type_text_markdown_accepts() {
        let hv = HeaderValue::from_static("text/markdown");
        assert!(validate_content_type(Some(&hv)).is_ok());
    }

    #[test]
    fn content_type_with_charset_accepts() {
        let hv = HeaderValue::from_static("text/markdown; charset=utf-8");
        assert!(validate_content_type(Some(&hv)).is_ok());
    }

    #[test]
    fn content_type_text_plain_rejects_with_received() {
        let hv = HeaderValue::from_static("text/plain");
        let result = validate_content_type(Some(&hv));
        assert!(matches!(
            result,
            Err(HttpError::UnsupportedMediaType { received: Some(s) }) if s == "text/plain"
        ));
    }

    #[test]
    fn content_type_application_json_rejects() {
        let hv = HeaderValue::from_static("application/json");
        assert!(matches!(
            validate_content_type(Some(&hv)),
            Err(HttpError::UnsupportedMediaType { .. })
        ));
    }

    #[test]
    fn content_type_multipart_rejects() {
        let hv = HeaderValue::from_static("multipart/form-data; boundary=xxx");
        assert!(matches!(
            validate_content_type(Some(&hv)),
            Err(HttpError::UnsupportedMediaType { .. })
        ));
    }

    #[test]
    fn content_type_missing_rejects_with_none() {
        assert!(matches!(
            validate_content_type(None),
            Err(HttpError::UnsupportedMediaType { received: None })
        ));
    }

    #[test]
    fn accept_missing_returns_primary() {
        assert_eq!(negotiate_accept(None).unwrap(), OUTPUT_MIME_PRIMARY);
    }

    #[test]
    fn accept_wildcard_returns_primary() {
        let hv = HeaderValue::from_static("*/*");
        assert_eq!(negotiate_accept(Some(&hv)).unwrap(), OUTPUT_MIME_PRIMARY);
    }

    #[test]
    fn accept_primary_mime_returns_primary() {
        let hv = HeaderValue::from_static("application/vnd.docspec.blocknote+json");
        assert_eq!(negotiate_accept(Some(&hv)).unwrap(), OUTPUT_MIME_PRIMARY);
    }

    #[test]
    fn accept_alias_mime_returns_primary() {
        let hv = HeaderValue::from_static("application/vnd.blocknote+json");
        assert_eq!(negotiate_accept(Some(&hv)).unwrap(), OUTPUT_MIME_PRIMARY);
    }

    #[test]
    fn accept_application_json_rejects() {
        let hv = HeaderValue::from_static("application/json");
        assert!(matches!(
            negotiate_accept(Some(&hv)),
            Err(HttpError::NotAcceptable)
        ));
    }

    #[test]
    fn accept_list_with_alias_and_quality_accepts() {
        let hv = HeaderValue::from_static("text/html, application/vnd.blocknote+json;q=0.8");
        assert_eq!(negotiate_accept(Some(&hv)).unwrap(), OUTPUT_MIME_PRIMARY);
    }

    #[test]
    fn accept_incompatible_list_rejects() {
        let hv = HeaderValue::from_static("text/html, application/xml");
        assert!(matches!(
            negotiate_accept(Some(&hv)),
            Err(HttpError::NotAcceptable)
        ));
    }

    #[test]
    fn accept_application_wildcard_returns_primary() {
        let hv = HeaderValue::from_static("application/*");
        assert_eq!(negotiate_accept(Some(&hv)).unwrap(), OUTPUT_MIME_PRIMARY);
    }
}
