//! In-process integration tests for the `docspec-http` router.

// Reason: integration tests use standard test patterns with expect/unwrap.
#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

mod common;

use axum::{
    body::{Body, Bytes},
    http::{header, StatusCode},
    Router,
};
use serde_json::{json, Value};

use docspec_test_utils::assert_json_eq;
use tower::ServiceExt as _;

const CACHE_CONTROL: &str = "max-age=0, private, must-revalidate";
const OUTPUT_MIME: &str = "application/vnd.docspec.blocknote+json";
const OUTPUT_MIME_HTML: &str = "text/html; charset=utf-8";
const OUTPUT_MIME_OXA: &str = "application/vnd.oxa+json";
const OUTPUT_MIME_PANDOC_NATIVE: &str = "application/vnd.pandoc.native; charset=utf-8";
const PROBLEM_JSON_CT: &str = "application/problem+json";
const HEALTH_CT: &str = "text/plain; charset=utf-8";

fn app() -> Router {
    docspec_http::router::router()
}

fn post_markdown(body: impl Into<Body>) -> axum::http::Request<Body> {
    common::markdown_request(body)
}

fn post_html(body: impl Into<Body>) -> axum::http::Request<Body> {
    common::request(
        "POST",
        "/conversion",
        &[("content-type", "text/html")],
        body,
    )
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

/// The `instance` every problem body carries: the response's `X-Request-ID`
/// as a `urn:uuid:` URI. The router assigns a UUID when the client sent none.
fn instance_of(response: &axum::response::Response) -> String {
    let id = response
        .headers()
        .get("x-request-id")
        .expect("x-request-id present")
        .to_str()
        .expect("ascii request id");
    format!("urn:uuid:{id}")
}

fn hello_blocknote_json() -> Value {
    serde_json::json!([{
        "type": "heading",
        "props": { "level": 1 },
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

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Bad Request",
            "status": 400,
            "detail": "Request body is empty",
            "instance": instance,
        })
    );
}

#[tokio::test]
async fn post_conversion_accepts_body_over_axum_default_limit() {
    // Reason: axum's `Bytes` extractor caps request bodies at
    // `const DEFAULT_LIMIT: usize = 2_097_152;` (2 MiB) by default. The
    // router disables this via `DefaultBodyLimit::disable()` to honor the
    // README's `Body size: No limit` pledge. A body of 2 MiB + 1 byte
    // would 413 before this fix — verifying 200 OK here is the regression
    // guard.
    const TWO_MIB: usize = 2 * 1024 * 1024;
    let large_body = "a".repeat(TWO_MIB + 1);

    let response = app()
        .oneshot(post_markdown(large_body))
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

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Unsupported Media Type",
            "status": 415,
            "detail": "Content-Type must be text/markdown, text/html, or application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "instance": instance,
        })
    );
}

fn find_image_url(blocks: &[Value]) -> Option<String> {
    for block in blocks {
        if block.get("type").and_then(Value::as_str) == Some("image") {
            if let Some(url) = block
                .get("props")
                .and_then(|p| p.get("url"))
                .and_then(Value::as_str)
            {
                return Some(url.to_owned());
            }
        }
        if let Some(children) = block.get("children").and_then(Value::as_array) {
            if let Some(url) = find_image_url(children) {
                return Some(url);
            }
        }
    }
    None
}

#[tokio::test]
async fn http_convert_docx_with_image() {
    let docx_bytes = docspec_test_utils::synth_docx_with_image_png();
    let response = app()
        .oneshot(common::docx_request(docx_bytes))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_body_json(response.into_body()).await;
    let blocks = body.as_array().expect("response is a JSON array");

    let image_url = find_image_url(blocks).expect("at least one image block with a url");
    assert!(
        image_url.starts_with("data:image/png;base64,"),
        "expected data URI but got: {image_url}"
    );
}

const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

fn hello_docx_bytes() -> Vec<u8> {
    let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#;
    common::synth_docx(common::SIMPLE_RELS, doc_xml)
}

#[tokio::test]
async fn post_conversion_docx_happy_path() {
    let response = app()
        .oneshot(common::docx_request(hello_docx_bytes()))
        .await
        .expect("request succeeds");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body_json(response.into_body()).await;
    let body_str = body.to_string();
    assert!(
        body_str.contains("paragraph"),
        "expected paragraph in: {body_str}"
    );
}

#[tokio::test]
async fn post_conversion_docx_with_charset_param_rejected_415() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", &format!("{DOCX_MIME}; charset=utf-8"))],
        hello_docx_bytes(),
    );
    let response = app().oneshot(request).await.expect("request succeeds");
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn post_conversion_docx_with_arbitrary_param_rejected_415() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", &format!("{DOCX_MIME}; foo=bar"))],
        hello_docx_bytes(),
    );
    let response = app().oneshot(request).await.expect("request succeeds");
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn post_conversion_docx_invalid_zip_returns_422() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", DOCX_MIME)],
        b"not a zip".to_vec(),
    );
    let response = app().oneshot(request).await.expect("request succeeds");
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn post_conversion_docx_truncated_between_paragraphs_returns_422() {
    // document.xml ends after a complete paragraph but before </w:body>: the
    // document is the problem, so this must be 422, never 500.
    let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>whole</w:t></w:r></w:p>"#;
    let response = app()
        .oneshot(common::docx_request(common::synth_docx(
            common::SIMPLE_RELS,
            doc_xml,
        )))
        .await
        .expect("request succeeds");
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        PROBLEM_JSON_CT
    );
    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        json!({
            "type": "https://problems.docspec.dev/malformed-input",
            "title": "Unprocessable Content",
            "status": 422,
            "detail": "parse error: malformed document.xml: unexpected EOF inside <w:body>",
            "instance": instance,
        })
    );
}

#[tokio::test]
async fn client_uuid_request_id_becomes_instance() {
    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", "text/markdown"),
            ("x-request-id", "0F8FAD5B-D9CB-469F-A165-70867728950E"),
        ],
        Body::empty(),
    );
    let response = app().oneshot(request).await.expect("request succeeds");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        json!({
            "type": "about:blank",
            "title": "Bad Request",
            "status": 400,
            "detail": "Request body is empty",
            "instance": "urn:uuid:0f8fad5b-d9cb-469f-a165-70867728950e",
        })
    );
}

#[tokio::test]
async fn non_uuid_request_id_yields_no_instance() {
    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", "text/markdown"),
            ("x-request-id", "job-42"),
        ],
        Body::empty(),
    );
    let response = app().oneshot(request).await.expect("request succeeds");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers().get("x-request-id").unwrap(), "job-42");
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        json!({
            "type": "about:blank",
            "title": "Bad Request",
            "status": 400,
            "detail": "Request body is empty",
        })
    );
}

mod limits {
    use super::*;
    use docspec_http::server::Limits;
    use std::time::Duration;

    fn limited(max_body_bytes: Option<u64>, conversion_timeout: Option<Duration>) -> Router {
        docspec_http::router::router_with_limits(Limits {
            max_body_bytes,
            conversion_timeout,
        })
    }

    fn markdown_with_length(body: &'static str) -> axum::http::Request<Body> {
        common::request(
            "POST",
            "/conversion",
            &[
                ("content-type", "text/markdown"),
                ("content-length", &body.len().to_string()),
            ],
            Body::from(body),
        )
    }

    #[tokio::test]
    async fn declared_length_over_the_limit_is_413_with_exact_body() {
        let response = limited(Some(8), None)
            .oneshot(markdown_with_length("# twelve bytes"))
            .await
            .expect("request succeeds");
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            PROBLEM_JSON_CT
        );
        let instance = instance_of(&response);
        let body = response_body_json(response.into_body()).await;
        assert_eq!(
            body,
            json!({
                "type": "about:blank",
                "title": "Content Too Large",
                "status": 413,
                "detail": "Request body exceeds the 8-byte limit",
                "instance": instance,
            })
        );
    }

    #[tokio::test]
    async fn undeclared_length_over_the_limit_is_413_while_streaming() {
        // No Content-Length header: the body is refused as it crosses the limit.
        let response = limited(Some(8), None)
            .oneshot(post_markdown("# twelve bytes"))
            .await
            .expect("request succeeds");
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let instance = instance_of(&response);
        let body = response_body_json(response.into_body()).await;
        assert_eq!(
            body,
            json!({
                "type": "about:blank",
                "title": "Content Too Large",
                "status": 413,
                "detail": "Request body exceeds the 8-byte limit",
                "instance": instance,
            })
        );
    }

    #[tokio::test]
    async fn body_at_the_limit_converts() {
        let response = limited(Some(7), None)
            .oneshot(markdown_with_length("# Hello"))
            .await
            .expect("request succeeds");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn no_limit_accepts_a_large_body() {
        let large = "# Hello\n\n".repeat(50_000);
        let response = limited(None, None)
            .oneshot(post_markdown(large))
            .await
            .expect("request succeeds");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn zero_timeout_is_503_with_retry_after_and_exact_body() {
        let response = limited(None, Some(Duration::ZERO))
            .oneshot(post_markdown("# Hello"))
            .await
            .expect("request succeeds");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers().get(header::RETRY_AFTER).unwrap(), "1");
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            PROBLEM_JSON_CT
        );
        let instance = instance_of(&response);
        let body = response_body_json(response.into_body()).await;
        assert_eq!(
            body,
            json!({
                "type": "https://problems.docspec.dev/conversion-timeout",
                "title": "Service Unavailable",
                "status": 503,
                "detail": "Conversion exceeded the 1-second limit and was abandoned",
                "instance": instance,
            })
        );
    }

    #[tokio::test]
    async fn generous_timeout_converts() {
        let response = limited(None, Some(Duration::from_secs(3600)))
            .oneshot(post_markdown("# Hello"))
            .await
            .expect("request succeeds");
        assert_eq!(response.status(), StatusCode::OK);
    }

    /// A DOCX whose body is `depth` nested tables, one past the reader's cap.
    fn nested_tables_docx(depth: usize) -> Vec<u8> {
        let mut body = String::new();
        for _ in 0..depth {
            body.push_str("<w:tbl><w:tr><w:tc>");
        }
        body.push_str("<w:p><w:r><w:t>deep</w:t></w:r></w:p>");
        for _ in 0..depth {
            body.push_str("</w:tc></w:tr></w:tbl>");
        }
        let doc_xml = format!(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
        );
        common::synth_docx(common::SIMPLE_RELS, &doc_xml)
    }

    #[tokio::test]
    async fn reader_cap_is_422_resource_limit() {
        let response = app()
            .oneshot(common::docx_request(nested_tables_docx(101)))
            .await
            .expect("request succeeds");
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let instance = instance_of(&response);
        let body = response_body_json(response.into_body()).await;
        assert_eq!(
            body,
            json!({
                "type": "https://problems.docspec.dev/resource-limit",
                "title": "Unprocessable Content",
                "status": 422,
                "detail": "table nesting exceeds the depth limit of 100 (possible zip bomb)",
                "instance": instance,
            })
        );
    }
}

#[tokio::test]
async fn post_conversion_docx_empty_body_returns_400() {
    let request = common::request(
        "POST",
        "/conversion",
        &[("content-type", DOCX_MIME)],
        b"".to_vec(),
    );
    let response = app().oneshot(request).await.expect("request succeeds");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Unsupported Media Type",
            "status": 415,
            "detail": "Content-Type must be text/markdown, text/html, or application/vnd.openxmlformats-officedocument.wordprocessingml.document, got application/json",
            "instance": instance,
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

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Unsupported Media Type",
            "status": 415,
            "detail": "Content-Type must be text/markdown, text/html, or application/vnd.openxmlformats-officedocument.wordprocessingml.document, got multipart/form-data; boundary=x",
            "instance": instance,
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

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Not Acceptable",
            "status": 406,
            "detail": "Accept header must include application/vnd.docspec.blocknote+json, application/vnd.blocknote+json, application/vnd.oxa+json, application/vnd.pandoc.native, text/html, application/*, or */*",
            "instance": instance,
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

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Bad Request",
            "status": 400,
            "detail": "Request body is not valid UTF-8",
            "instance": instance,
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

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Method Not Allowed",
            "status": 405,
            "detail": "Method not allowed. Allowed methods: POST, OPTIONS.",
            "instance": instance,
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

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Method Not Allowed",
            "status": 405,
            "detail": "Method not allowed. Allowed methods: POST, OPTIONS.",
            "instance": instance,
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

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Method Not Allowed",
            "status": 405,
            "detail": "Method not allowed. Allowed methods: GET, HEAD, OPTIONS.",
            "instance": instance,
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

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Not Found",
            "status": 404,
            "detail": "No route matches GET /unknown",
            "instance": instance,
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
    assert_json_eq(
        &body_text,
        json!({
            "type": "Document",
            "children": [
                {"type": "Paragraph", "children": [{"type": "Text", "value": "Hello world"}]}
            ]
        }),
    );
}

#[tokio::test]
async fn post_conversion_pandoc_native_happy_path() {
    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", "text/markdown"),
            ("accept", "application/vnd.pandoc.native"),
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
        OUTPUT_MIME_PANDOC_NATIVE
    );

    let body_text = response_body_text(response.into_body()).await;
    assert_eq!(body_text, r#"[Para [Str "Hello world"]]"#);
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
    assert_json_eq(
        &body_text,
        json!({
            "type": "Document",
            "children": [{"type": "Paragraph", "children": [{"type": "Text", "value": "Hello"}]}]
        }),
    );
}

#[tokio::test]
async fn post_conversion_html_empty_body() {
    let response = app()
        .oneshot(post_html(""))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Bad Request",
            "status": 400,
            "detail": "Request body is empty",
            "instance": instance,
        })
    );
}

#[tokio::test]
async fn query_string_not_in_404_detail() {
    let request = common::empty_request("GET", "/unknown?secret=password");

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let instance = instance_of(&response);
    let body = response_body_json(response.into_body()).await;
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Not Found",
            "status": 404,
            "detail": "No route matches GET /unknown",
            "instance": instance,
        })
    );
}
