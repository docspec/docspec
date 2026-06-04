//! In-process integration tests for the `docspec-http` router.

// Reason: integration tests use standard test patterns with expect/unwrap.
#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use axum::{
    body::{Body, Bytes},
    http::{header, StatusCode},
    Router,
};
use serde_json::Value;
use tower::ServiceExt as _;

const CACHE_CONTROL: &str = "max-age=0, private, must-revalidate";
const DOCX_HELLO_XML: &str = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#;
const DOCX_MIME: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const DOCX_RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const HEALTH_CT: &str = "text/plain; charset=utf-8";
const OUTPUT_MIME: &str = "application/vnd.docspec.blocknote+json; charset=utf-8";
const OUTPUT_MIME_HTML: &str = "text/html; charset=utf-8";
const OUTPUT_MIME_OXA: &str = "application/vnd.oxa+json; charset=utf-8";
const PROBLEM_JSON_CT: &str = "application/problem+json; charset=utf-8";

fn app() -> Router {
    docspec_http::router::router()
}

fn post_docx(body: impl Into<Body>) -> axum::http::Request<Body> {
    common::request("POST", "/conversion", &[("content-type", DOCX_MIME)], body)
}

fn post_html(body: impl Into<Body>) -> axum::http::Request<Body> {
    common::request(
        "POST",
        "/conversion",
        &[("content-type", "text/html")],
        body,
    )
}

fn post_markdown(body: impl Into<Body>) -> axum::http::Request<Body> {
    common::markdown_request(body)
}

fn synth_hello_docx() -> Vec<u8> {
    docspec_test_fixtures::synth_docx(DOCX_RELS, DOCX_HELLO_XML)
}

async fn response_body_text(body: axum::body::Body) -> String {
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("body read");
    String::from_utf8(bytes.to_vec()).expect("UTF-8 body")
}

async fn response_body_json(body: axum::body::Body) -> Value {
    let text = response_body_text(body).await;
    serde_json::from_str(&text).expect("valid JSON")
}

fn hello_blocknote_json() -> Value {
    serde_json::json!([{
        "type": "heading",
        "props": { "level": 1, "textAlignment": "left" },
        "content": [{ "type": "text", "text": "Hello", "styles": {} }],
        "children": [],
    }])
}

#[tokio::test]
async fn post_conversion_happy_path() {
    let response = app()
        .oneshot(post_markdown("# Hello"))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        OUTPUT_MIME
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("cache-control present"),
        CACHE_CONTROL
    );
    let request_id = response
        .headers()
        .get("x-request-id")
        .expect("x-request-id present")
        .to_str()
        .expect("ASCII header value");
    let parsed = uuid::Uuid::parse_str(request_id).expect("valid UUID");
    assert_eq!(parsed.get_version(), Some(uuid::Version::Random));

    let body = response_body_json(response.into_body()).await;
    assert_eq!(body, hello_blocknote_json());
}

#[tokio::test]
async fn post_conversion_echoes_request_id() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "text/markdown"), ("x-request-id", "my-id")],
        Body::from("# Hello"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(
        response
            .headers()
            .get("x-request-id")
            .expect("x-request-id present"),
        "my-id"
    );
}

#[tokio::test]
async fn post_conversion_echoes_trace_id() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "text/markdown"), ("x-trace-id", "trace-1")],
        Body::from("# Hello"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(
        response
            .headers()
            .get("x-trace-id")
            .expect("x-trace-id present"),
        "trace-1"
    );
}

#[tokio::test]
async fn post_conversion_no_trace_id_generated() {
    let response = app()
        .oneshot(post_markdown("# Hello"))
        .await
        .expect("request succeeds");

    assert!(
        response.headers().get("x-trace-id").is_none(),
        "x-trace-id must NOT be generated when absent from request"
    );
}

#[tokio::test]
async fn post_conversion_empty_body() {
    let response = app()
        .oneshot(post_markdown(""))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("cache-control present"),
        CACHE_CONTROL
    );

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Bad Request",
            "status": 400,
            "detail": "Request body is empty",
        })
    );
}

#[tokio::test]
async fn post_conversion_missing_content_type() {
    let request = common::request("POST", "/conversion", &[], Body::from("# Hello"));

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        PROBLEM_JSON_CT
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("cache-control present"),
        CACHE_CONTROL
    );

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Unsupported Media Type",
            "status": 415,
            "detail": "Content-Type must be text/markdown, text/html, or application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        })
    );
}

#[tokio::test]
async fn post_conversion_wrong_content_type() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "application/json")],
        Body::from("{}"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Unsupported Media Type",
            "status": 415,
            "detail": "Content-Type must be text/markdown, text/html, or application/vnd.openxmlformats-officedocument.wordprocessingml.document, got application/json",
        })
    );
}

#[tokio::test]
async fn post_conversion_multipart_content_type() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "multipart/form-data; boundary=x")],
        Body::from("data"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Unsupported Media Type",
            "status": 415,
            "detail": "Content-Type must be text/markdown, text/html, or application/vnd.openxmlformats-officedocument.wordprocessingml.document, got multipart/form-data; boundary=x",
        })
    );
}

#[tokio::test]
async fn post_conversion_wrong_accept() {
    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", "text/markdown"),
            ("accept", "application/json"),
        ],
        Body::from("# Hello"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Not Acceptable",
            "status": 406,
            "detail": "Accept header must include application/vnd.docspec.blocknote+json, application/vnd.blocknote+json, application/vnd.oxa+json, text/html, application/*, or */*",
        })
    );
}

#[tokio::test]
async fn post_conversion_missing_accept() {
    let response = app()
        .oneshot(post_markdown("# Hello"))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(body, hello_blocknote_json());
}

#[tokio::test]
async fn post_conversion_wildcard_accept() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "text/markdown"), ("accept", "*/*")],
        Body::from("# Hello"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(body, hello_blocknote_json());
}

#[tokio::test]
async fn post_conversion_alias_accept() {
    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", "text/markdown"),
            ("accept", "application/vnd.blocknote+json"),
        ],
        Body::from("# Hello"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(body, hello_blocknote_json());
}

#[tokio::test]
async fn post_conversion_invalid_utf8() {
    let request = common::markdown_request(Body::from(Bytes::from_static(&[0xFF, 0xFE])));

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Bad Request",
            "status": 400,
            "detail": "Request body is not valid UTF-8",
        })
    );
}

#[tokio::test]
async fn options_conversion_returns_204() {
    let request = common::empty_request("OPTIONS", "/conversion");

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get(header::ALLOW)
            .expect("allow header present"),
        "POST, OPTIONS"
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("cache-control present"),
        CACHE_CONTROL
    );
}

#[tokio::test]
async fn put_conversion_returns_405() {
    let request = common::empty_request("PUT", "/conversion");

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        response
            .headers()
            .get(header::ALLOW)
            .expect("allow header present"),
        "POST, OPTIONS"
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("cache-control present"),
        CACHE_CONTROL
    );
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        PROBLEM_JSON_CT
    );

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Method Not Allowed",
            "status": 405,
            "detail": "Method not allowed. Allowed methods: POST, OPTIONS.",
        })
    );
}

#[tokio::test]
async fn delete_conversion_returns_405() {
    let request = common::empty_request("DELETE", "/conversion");

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        response
            .headers()
            .get(header::ALLOW)
            .expect("allow header present"),
        "POST, OPTIONS"
    );

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Method Not Allowed",
            "status": 405,
            "detail": "Method not allowed. Allowed methods: POST, OPTIONS.",
        })
    );
}

#[tokio::test]
async fn get_health_returns_200() {
    let request = common::empty_request("GET", "/health");

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        HEALTH_CT
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("cache-control present"),
        CACHE_CONTROL
    );

    let body = response_body_text(response.into_body()).await;
    assert_eq!(body, "Healthy.");
}

#[tokio::test]
async fn head_health_returns_204() {
    let request = common::empty_request("HEAD", "/health");

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("cache-control present"),
        CACHE_CONTROL
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body read");
    assert!(body_bytes.is_empty(), "HEAD response must have empty body");
}

#[tokio::test]
async fn options_health_returns_204() {
    let request = common::empty_request("OPTIONS", "/health");

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get(header::ALLOW)
            .expect("allow header present"),
        "GET, HEAD, OPTIONS"
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("cache-control present"),
        CACHE_CONTROL
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body read");
    assert!(body_bytes.is_empty(), "OPTIONS 204 must have empty body");
}

#[tokio::test]
async fn put_health_returns_405() {
    let request = common::empty_request("PUT", "/health");

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        response
            .headers()
            .get(header::ALLOW)
            .expect("allow header present"),
        "GET, HEAD, OPTIONS"
    );

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Method Not Allowed",
            "status": 405,
            "detail": "Method not allowed. Allowed methods: GET, HEAD, OPTIONS.",
        })
    );
}

#[tokio::test]
async fn unknown_path_returns_404() {
    let request = common::empty_request("GET", "/unknown");

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        PROBLEM_JSON_CT
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("cache-control present"),
        CACHE_CONTROL
    );

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Not Found",
            "status": 404,
            "detail": "No route matches GET /unknown",
        })
    );
}

#[tokio::test]
async fn post_conversion_html_output_happy_path() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "text/markdown"), ("accept", "text/html")],
        Body::from("Hello world"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        OUTPUT_MIME_HTML
    );

    let body_text = response_body_text(response.into_body()).await;
    assert_eq!(body_text, "<html><body><p>Hello world</p></body></html>");
}

#[tokio::test]
async fn post_conversion_html_input_to_html_output_happy_path() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "text/html"), ("accept", "text/html")],
        Body::from("<p>Hello</p>"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        OUTPUT_MIME_HTML
    );

    let body_text = response_body_text(response.into_body()).await;
    assert_eq!(body_text, "<html><body><p>Hello</p></body></html>");
}

#[tokio::test]
async fn post_conversion_oxa_happy_path() {
    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", "text/markdown"),
            ("accept", "application/vnd.oxa+json"),
        ],
        Body::from("Hello world"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        OUTPUT_MIME_OXA
    );

    let body_text = response_body_text(response.into_body()).await;
    assert_eq!(
        body_text,
        r#"{"type":"Document","children":[{"type":"Paragraph","children":[{"type":"Text","value":"Hello world"}]}]}"#
    );
}

#[tokio::test]
async fn post_conversion_wildcard_accept_defaults_to_blocknote_not_oxa() {
    // Regression guard: `*/*` MUST select BlockNote so pre-oxa clients keep their responses.
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "text/markdown"), ("accept", "*/*")],
        Body::from("# Hello"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        OUTPUT_MIME
    );

    let body = response_body_json(response.into_body()).await;
    assert_eq!(body, hello_blocknote_json());
}

#[tokio::test]
async fn post_conversion_html_happy_path() {
    let response = app()
        .oneshot(post_html("<p>Hello</p>"))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        OUTPUT_MIME
    );

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!([{
            "type": "paragraph",
            "props": { "textAlignment": "left" },
            "content": [{ "type": "text", "text": "Hello", "styles": {} }],
            "children": [],
        }])
    );
}

#[tokio::test]
async fn post_conversion_html_with_utf8_charset_happy_path() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "text/html; charset=utf-8")],
        Body::from("<p>Hello</p>"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!([{
            "type": "paragraph",
            "props": { "textAlignment": "left" },
            "content": [{ "type": "text", "text": "Hello", "styles": {} }],
            "children": [],
        }])
    );
}

#[tokio::test]
async fn post_conversion_html_to_oxa_happy_path() {
    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", "text/html"),
            ("accept", "application/vnd.oxa+json"),
        ],
        Body::from("<p>Hello</p>"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        OUTPUT_MIME_OXA
    );

    let body_text = response_body_text(response.into_body()).await;
    assert_eq!(
        body_text,
        r#"{"type":"Document","children":[{"type":"Paragraph","children":[{"type":"Text","value":"Hello"}]}]}"#
    );
}

#[tokio::test]
async fn post_conversion_html_empty_body() {
    let response = app()
        .oneshot(post_html(""))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Bad Request",
            "status": 400,
            "detail": "Request body is empty",
        })
    );
}

#[tokio::test]
async fn query_string_not_in_404_detail() {
    let request = common::empty_request("GET", "/unknown?secret=password");

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Not Found",
            "status": 404,
            "detail": "No route matches GET /unknown",
        })
    );
}

#[tokio::test]
async fn post_conversion_docx_happy_path() {
    let docx_bytes = synth_hello_docx();
    let response = app()
        .oneshot(post_docx(docx_bytes))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        OUTPUT_MIME
    );

    let response_body = response_body_json(response.into_body()).await;
    assert_eq!(
        response_body,
        serde_json::json!([{
            "type": "paragraph",
            "props": { "textAlignment": "left" },
            "content": [{ "type": "text", "text": "Hello", "styles": {} }],
            "children": [],
        }])
    );
}

#[tokio::test]
async fn post_conversion_docx_to_oxa_happy_path() {
    let body = synth_hello_docx();
    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", DOCX_MIME),
            ("accept", "application/vnd.oxa+json"),
        ],
        body,
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        OUTPUT_MIME_OXA
    );

    let body_text = response_body_text(response.into_body()).await;
    assert_eq!(
        body_text,
        r#"{"type":"Document","children":[{"type":"Paragraph","children":[{"type":"Text","value":"Hello"}]}]}"#
    );
}

#[tokio::test]
async fn post_conversion_docx_empty_body() {
    let response = app()
        .oneshot(post_docx(""))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Bad Request",
            "status": 400,
            "detail": "Request body is empty",
        })
    );
}

#[tokio::test]
async fn post_conversion_docx_invalid_zip() {
    let response = app()
        .oneshot(post_docx("not a zip archive"))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let response_body = response_body_json(response.into_body()).await;
    assert_eq!(
        response_body
            .get("status")
            .and_then(serde_json::Value::as_u64),
        Some(422)
    );
    assert_eq!(
        response_body
            .get("title")
            .and_then(serde_json::Value::as_str),
        Some("Unprocessable Entity")
    );
}

#[tokio::test]
async fn post_conversion_octet_stream_rejected() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "application/octet-stream")],
        Body::from("some bytes"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Unsupported Media Type",
            "status": 415,
            "detail": "Content-Type must be text/markdown, text/html, or application/vnd.openxmlformats-officedocument.wordprocessingml.document, got application/octet-stream",
        })
    );
}

#[tokio::test]
async fn post_conversion_docx_with_charset_param_rejected() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "application/vnd.openxmlformats-officedocument.wordprocessingml.document; charset=utf-8")],
        Body::from("some bytes"),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Unsupported Media Type",
            "status": 415,
            "detail": "Content-Type must be text/markdown, text/html, or application/vnd.openxmlformats-officedocument.wordprocessingml.document, got application/vnd.openxmlformats-officedocument.wordprocessingml.document; charset=utf-8",
        })
    );
}

#[tokio::test]
async fn post_conversion_oversized_body_rejected() {
    let limited_router = docspec_http::router::router_with_body_limit(100);
    let body = vec![b'x'; 200];
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "text/markdown")],
        body,
    );

    let response = limited_router
        .oneshot(request)
        .await
        .expect("request completes");

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn post_conversion_markdown_with_invalid_utf8_still_400() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", "text/markdown")],
        Body::from(b"\xFF\xFE\x00".to_vec()),
    );

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Bad Request",
            "status": 400,
            "detail": "Request body is not valid UTF-8",
        })
    );
}
