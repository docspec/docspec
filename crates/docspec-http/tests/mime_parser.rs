//! Unit tests for the `mime_parser` module.

#![allow(clippy::tests_outside_test_module, clippy::unwrap_used)]

use axum::http::HeaderValue;
use docspec::OutputFormat;
use docspec_http::error::HttpError;
use docspec_http::mime_parser::{negotiate_accept, validate_content_type};

#[test]
fn content_type_text_markdown_accepts() {
    let header = HeaderValue::from_static("text/markdown");
    assert!(validate_content_type(Some(&header)).is_ok());
}

#[test]
fn content_type_with_utf8_charset_accepts() {
    let header = HeaderValue::from_static("text/markdown; charset=utf-8");
    assert!(validate_content_type(Some(&header)).is_ok());
}

#[test]
fn content_type_with_utf8_charset_case_insensitive() {
    let header = HeaderValue::from_static("text/markdown; charset=UTF-8");
    assert!(validate_content_type(Some(&header)).is_ok());
}

#[test]
fn content_type_with_non_utf8_charset_rejects() {
    let header = HeaderValue::from_static("text/markdown; charset=iso-8859-1");
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "text/markdown; charset=iso-8859-1"
    ));
}

#[test]
fn content_type_with_unknown_param_rejects() {
    let header = HeaderValue::from_static("text/markdown; boundary=xyz");
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "text/markdown; boundary=xyz"
    ));
}

#[test]
fn content_type_with_charset_and_unknown_param_rejects() {
    let header = HeaderValue::from_static("text/markdown; charset=utf-8; format=fixed");
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "text/markdown; charset=utf-8; format=fixed"
    ));
}

#[test]
fn content_type_text_plain_rejects_with_received() {
    let header = HeaderValue::from_static("text/plain");
    let result = validate_content_type(Some(&header));
    assert!(matches!(
        result,
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "text/plain"
    ));
}

#[test]
fn content_type_application_json_rejects() {
    let header = HeaderValue::from_static("application/json");
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "application/json"
    ));
}

#[test]
fn content_type_multipart_rejects() {
    let header = HeaderValue::from_static("multipart/form-data; boundary=xxx");
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "multipart/form-data; boundary=xxx"
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
fn accept_missing_returns_blocknote() {
    assert_eq!(negotiate_accept(None).unwrap(), OutputFormat::Blocknote);
}

#[test]
fn accept_wildcard_returns_blocknote() {
    let header = HeaderValue::from_static("*/*");
    assert_eq!(
        negotiate_accept(Some(&header)).unwrap(),
        OutputFormat::Blocknote
    );
}

#[test]
fn accept_primary_mime_returns_blocknote() {
    let header = HeaderValue::from_static("application/vnd.docspec.blocknote+json");
    assert_eq!(
        negotiate_accept(Some(&header)).unwrap(),
        OutputFormat::Blocknote
    );
}

#[test]
fn accept_alias_mime_returns_blocknote() {
    let header = HeaderValue::from_static("application/vnd.blocknote+json");
    assert_eq!(
        negotiate_accept(Some(&header)).unwrap(),
        OutputFormat::Blocknote
    );
}

#[test]
fn accept_oxa_primary_mime_returns_oxa() {
    let header = HeaderValue::from_static("application/vnd.oxa+json");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Oxa);
}

#[test]
fn accept_list_oxa_first_returns_oxa() {
    let header = HeaderValue::from_static(
        "application/vnd.oxa+json, application/vnd.docspec.blocknote+json",
    );
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Oxa);
}

#[test]
fn accept_application_json_rejects() {
    let header = HeaderValue::from_static("application/json");
    assert!(matches!(
        negotiate_accept(Some(&header)),
        Err(HttpError::NotAcceptable)
    ));
}

#[test]
fn accept_list_with_alias_and_quality_accepts() {
    let header = HeaderValue::from_static("text/html, application/vnd.blocknote+json;q=0.8");
    assert_eq!(
        negotiate_accept(Some(&header)).unwrap(),
        OutputFormat::Blocknote
    );
}

#[test]
fn accept_incompatible_list_rejects() {
    let header = HeaderValue::from_static("text/html, application/xml");
    assert!(matches!(
        negotiate_accept(Some(&header)),
        Err(HttpError::NotAcceptable)
    ));
}

#[test]
fn accept_application_wildcard_returns_blocknote() {
    let header = HeaderValue::from_static("application/*");
    assert_eq!(
        negotiate_accept(Some(&header)).unwrap(),
        OutputFormat::Blocknote
    );
}
