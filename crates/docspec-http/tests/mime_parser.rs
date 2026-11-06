//! Unit tests for the `mime_parser` module.

#![allow(clippy::panic, clippy::tests_outside_test_module, clippy::unwrap_used)]

use axum::http::HeaderValue;
use docspec::{InputFormat, OutputFormat};
use docspec_http::error::HttpError;
use docspec_http::mime_parser::{negotiate_accept, validate_content_type};

// ─── validate_content_type: markdown ────────────────────────────────────────

#[test]
fn content_type_text_markdown_returns_markdown_format() {
    let header = HeaderValue::from_static("text/markdown");
    assert_eq!(
        validate_content_type(Some(&header)).unwrap(),
        InputFormat::Markdown
    );
}

#[test]
fn content_type_text_markdown_with_utf8_charset_returns_markdown() {
    let header = HeaderValue::from_static("text/markdown; charset=utf-8");
    assert_eq!(
        validate_content_type(Some(&header)).unwrap(),
        InputFormat::Markdown
    );
}

#[test]
fn content_type_text_markdown_charset_is_case_insensitive() {
    let header = HeaderValue::from_static("text/markdown; charset=UTF-8");
    assert_eq!(
        validate_content_type(Some(&header)).unwrap(),
        InputFormat::Markdown
    );
}

#[test]
fn content_type_text_markdown_with_non_utf8_charset_rejects() {
    let header = HeaderValue::from_static("text/markdown; charset=iso-8859-1");
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "text/markdown; charset=iso-8859-1"
    ));
}

#[test]
fn content_type_text_markdown_with_unknown_param_rejects() {
    let header = HeaderValue::from_static("text/markdown; boundary=xyz");
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "text/markdown; boundary=xyz"
    ));
}

#[test]
fn content_type_text_markdown_with_charset_and_unknown_param_rejects() {
    let header = HeaderValue::from_static("text/markdown; charset=utf-8; format=fixed");
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "text/markdown; charset=utf-8; format=fixed"
    ));
}

// ─── validate_content_type: html ────────────────────────────────────────────

#[test]
fn content_type_text_html_returns_html_format() {
    let header = HeaderValue::from_static("text/html");
    assert_eq!(
        validate_content_type(Some(&header)).unwrap(),
        InputFormat::Html
    );
}

#[test]
fn content_type_text_html_with_utf8_charset_returns_html() {
    let header = HeaderValue::from_static("text/html; charset=utf-8");
    assert_eq!(
        validate_content_type(Some(&header)).unwrap(),
        InputFormat::Html
    );
}

#[test]
fn content_type_text_html_charset_is_case_insensitive() {
    let header = HeaderValue::from_static("text/html; charset=UTF-8");
    assert_eq!(
        validate_content_type(Some(&header)).unwrap(),
        InputFormat::Html
    );
}

#[test]
fn content_type_text_html_with_non_utf8_charset_rejects() {
    let header = HeaderValue::from_static("text/html; charset=iso-8859-1");
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "text/html; charset=iso-8859-1"
    ));
}

#[test]
fn content_type_text_html_with_unknown_param_rejects() {
    let header = HeaderValue::from_static("text/html; boundary=xyz");
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "text/html; boundary=xyz"
    ));
}

// ─── validate_content_type: rejections ──────────────────────────────────────

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
fn content_type_application_xhtml_rejects() {
    let header = HeaderValue::from_static("application/xhtml+xml");
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(text) }) if text == "application/xhtml+xml"
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

// ─── negotiate_accept ───────────────────────────────────────────────────────

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
fn accept_html_mime_returns_html() {
    let header = HeaderValue::from_static("text/html");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Html);
}

#[test]
fn accept_html_mime_with_quality_returns_html() {
    let header = HeaderValue::from_static("text/html;q=0.8");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Html);
}

#[test]
fn accept_oxa_primary_mime_returns_oxa() {
    let header = HeaderValue::from_static("application/vnd.oxa+json");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Oxa);
}

#[test]
fn accept_pandoc_native_primary_mime_returns_pandoc_native() {
    let header = HeaderValue::from_static("application/vnd.pandoc.native");
    assert_eq!(
        negotiate_accept(Some(&header)).unwrap(),
        OutputFormat::PandocNative
    );
}

#[test]
fn accept_pandoc_native_mime_with_quality_returns_pandoc_native() {
    let header = HeaderValue::from_static("application/vnd.pandoc.native;q=0.8");
    assert_eq!(
        negotiate_accept(Some(&header)).unwrap(),
        OutputFormat::PandocNative
    );
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
    let header = HeaderValue::from_static("text/plain, application/vnd.blocknote+json;q=0.8");
    assert_eq!(
        negotiate_accept(Some(&header)).unwrap(),
        OutputFormat::Blocknote
    );
}

#[test]
fn accept_incompatible_list_rejects() {
    let header = HeaderValue::from_static("text/plain, application/xml");
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

// ─── negotiate_accept: q weights (RFC 9110 §12.5.1) ─────────────────────────

#[test]
fn accept_q_zero_means_not_acceptable() {
    let header = HeaderValue::from_static("text/html;q=0");
    assert!(matches!(
        negotiate_accept(Some(&header)),
        Err(HttpError::NotAcceptable)
    ));
}

#[test]
fn accept_prefers_higher_q_over_list_order() {
    let header = HeaderValue::from_static("text/html;q=0.1, application/vnd.oxa+json;q=1");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Oxa);
}

#[test]
fn accept_wildcard_low_q_does_not_beat_explicit_type() {
    let header = HeaderValue::from_static("*/*;q=0.1, application/vnd.oxa+json");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Oxa);
}

#[test]
fn accept_q_name_is_case_insensitive() {
    let header = HeaderValue::from_static("text/html; Q=0, application/vnd.oxa+json; Q=0.5");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Oxa);
}

#[test]
fn accept_malformed_q_counts_as_full_weight() {
    let header = HeaderValue::from_static("text/html;q=abc, application/vnd.oxa+json;q=0.9");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Html);
}

#[test]
fn accept_q_above_one_is_malformed_so_list_order_breaks_the_tie() {
    let header = HeaderValue::from_static("text/html;q=1, application/vnd.oxa+json;q=5");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Html);
}

#[test]
fn accept_exact_q_zero_is_not_overridden_by_wildcard() {
    let header = HeaderValue::from_static("application/vnd.docspec.blocknote+json;q=0, */*");
    assert!(matches!(
        negotiate_accept(Some(&header)),
        Err(HttpError::NotAcceptable)
    ));
}

#[test]
fn accept_exact_q_zero_with_wildcard_falls_back_to_next_acceptable_format() {
    let header = HeaderValue::from_static(
        "application/vnd.docspec.blocknote+json;q=0, */*, application/vnd.oxa+json;q=0.5",
    );
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Oxa);
}

#[test]
fn accept_wildcard_q_zero_does_not_exclude_an_exact_type() {
    let header = HeaderValue::from_static("*/*;q=0, application/vnd.docspec.blocknote+json");
    assert_eq!(
        negotiate_accept(Some(&header)).unwrap(),
        OutputFormat::Blocknote
    );
}

#[test]
fn accept_application_wildcard_is_more_specific_than_any() {
    let header = HeaderValue::from_static("*/*, application/*;q=0");
    assert!(matches!(
        negotiate_accept(Some(&header)),
        Err(HttpError::NotAcceptable)
    ));
}

#[test]
fn accept_html_q_zero_with_text_wildcard_rejects() {
    let header = HeaderValue::from_static("text/html;q=0, text/*");
    assert!(matches!(
        negotiate_accept(Some(&header)),
        Err(HttpError::NotAcceptable)
    ));
}

#[test]
fn accept_q_with_three_decimals_is_honoured() {
    let header = HeaderValue::from_static("text/html;q=0.001, application/vnd.oxa+json;q=0.002");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Oxa);
}

#[test]
fn accept_q_with_four_decimals_is_malformed_not_zero() {
    let header = HeaderValue::from_static("text/html;q=0.0001");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Html);
}

#[test]
fn accept_q_with_sign_is_malformed_not_zero() {
    let header = HeaderValue::from_static("text/html;q=+0");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Html);
}

#[test]
fn accept_q_one_with_nonzero_decimals_is_malformed() {
    let header = HeaderValue::from_static("text/html;q=1.001, application/vnd.oxa+json;q=0.999");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Html);
}

#[test]
fn accept_q_zero_with_trailing_dot_and_zeros_excludes() {
    let header = HeaderValue::from_static("text/html;q=0., application/vnd.oxa+json;q=0.000");
    assert!(matches!(
        negotiate_accept(Some(&header)),
        Err(HttpError::NotAcceptable)
    ));
}

#[test]
fn accept_q_one_with_three_zeros_is_full_weight() {
    let header = HeaderValue::from_static("text/html;q=0.999, application/vnd.oxa+json;q=1.000");
    assert_eq!(negotiate_accept(Some(&header)).unwrap(), OutputFormat::Oxa);
}

#[test]
fn accept_only_q_zero_matches_rejects_even_with_unknown_types() {
    let header = HeaderValue::from_static("text/plain, */*;q=0");
    assert!(matches!(
        negotiate_accept(Some(&header)),
        Err(HttpError::NotAcceptable)
    ));
}

// ─── validate_content_type: DOCX ─────────────────────────────────────────────

#[test]
fn content_type_docx_returns_docx_format() {
    let header = HeaderValue::from_static(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    );
    assert_eq!(
        validate_content_type(Some(&header)).unwrap(),
        InputFormat::Docx
    );
}

#[test]
fn content_type_docx_with_charset_rejects() {
    let header = HeaderValue::from_static(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document; charset=utf-8",
    );
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(_) })
    ));
}

#[test]
fn content_type_docx_with_arbitrary_param_rejects() {
    let header = HeaderValue::from_static(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document; foo=bar",
    );
    assert!(matches!(
        validate_content_type(Some(&header)),
        Err(HttpError::UnsupportedMediaType { received: Some(_) })
    ));
}

#[test]
fn content_type_odt_returns_odt_format() {
    let header = HeaderValue::from_static("application/vnd.oasis.opendocument.text");
    assert_eq!(
        validate_content_type(Some(&header)).unwrap(),
        InputFormat::Odt
    );
}

#[test]
fn content_type_odt_with_param_rejects() {
    let header = HeaderValue::from_static("application/vnd.oasis.opendocument.text; charset=utf-8");
    let Err(HttpError::UnsupportedMediaType { received }) = validate_content_type(Some(&header))
    else {
        panic!("expected UnsupportedMediaType");
    };
    assert_eq!(
        received.as_deref(),
        Some("application/vnd.oasis.opendocument.text; charset=utf-8")
    );
}

#[test]
fn bucket_input_mime_odt_returns_odt_label() {
    let header = HeaderValue::from_static("application/vnd.oasis.opendocument.text");
    assert_eq!(
        docspec_http::mime_parser::bucket_input_mime(Some(&header)),
        docspec_http::mime::MIME_ODT
    );
}

#[test]
fn bucket_input_mime_docx_returns_docx_label() {
    let header = HeaderValue::from_static(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    );
    assert_eq!(
        docspec_http::mime_parser::bucket_input_mime(Some(&header)),
        docspec_http::mime::MIME_DOCX
    );
}

#[test]
fn bucket_input_mime_docx_with_params_returns_docx_label() {
    let header = HeaderValue::from_static(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document; charset=utf-8",
    );
    assert_eq!(
        docspec_http::mime_parser::bucket_input_mime(Some(&header)),
        docspec_http::mime::MIME_DOCX
    );
}
