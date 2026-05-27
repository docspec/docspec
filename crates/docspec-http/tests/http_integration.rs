//! In-process integration tests for the `docspec-http` router.

// Reason: integration tests use standard test patterns with expect/unwrap.
#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used
)]

use axum::{
    body::{Body, Bytes},
    http::{header, Method, Request, StatusCode},
    Router,
};
use serde_json::Value;
use tower::ServiceExt as _;

const CACHE_CONTROL: &str = "max-age=0, private, must-revalidate";
const OUTPUT_MIME: &str = "application/vnd.docspec.blocknote+json; charset=utf-8";
const PROBLEM_JSON_CT: &str = "application/problem+json; charset=utf-8";
const HEALTH_CT: &str = "text/plain; charset=utf-8";

fn app() -> Router {
    docspec_http::router::router()
}

fn post_markdown(body: impl Into<Body>) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/conversion")
        .header(header::CONTENT_TYPE, "text/markdown")
        .body(body.into())
        .unwrap()
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

fn assert_problem_json(body_json: &Value) {
    let obj = body_json.as_object().expect("JSON object");
    assert_eq!(obj.len(), 4, "RFC 7807 must have exactly 4 fields");
    assert!(obj.contains_key("type"), "missing 'type'");
    assert!(obj.contains_key("title"), "missing 'title'");
    assert!(obj.contains_key("status"), "missing 'status'");
    assert!(obj.contains_key("detail"), "missing 'detail'");
    assert!(!obj.contains_key("instance"), "must not have 'instance'");
    assert_eq!(
        obj.get("type").and_then(|v| v.as_str()),
        Some("about:blank"),
        "type URI must be 'about:blank'"
    );
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
    assert!(
        response.headers().get("x-request-id").is_some(),
        "x-request-id must be present"
    );

    let body = response_body_json(response.into_body()).await;
    assert!(body.is_array(), "response body must be a JSON array");
}

#[tokio::test]
async fn post_conversion_echoes_request_id() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/conversion")
        .header(header::CONTENT_TYPE, "text/markdown")
        .header("x-request-id", "my-id")
        .body(Body::from("# Hello"))
        .unwrap();

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
    let request = Request::builder()
        .method(Method::POST)
        .uri("/conversion")
        .header(header::CONTENT_TYPE, "text/markdown")
        .header("x-trace-id", "trace-1")
        .body(Body::from("# Hello"))
        .unwrap();

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
    let detail = body
        .get("detail")
        .expect("detail field present")
        .as_str()
        .expect("detail is a string");
    assert!(
        detail.contains("Request body is empty"),
        "unexpected detail: {detail}"
    );
}

#[tokio::test]
async fn post_conversion_missing_content_type() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/conversion")
        .body(Body::from("# Hello"))
        .unwrap();

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
    assert_problem_json(&body);
}

#[tokio::test]
async fn post_conversion_wrong_content_type() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/conversion")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn post_conversion_multipart_content_type() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/conversion")
        .header(header::CONTENT_TYPE, "multipart/form-data; boundary=x")
        .body(Body::from("data"))
        .unwrap();

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn post_conversion_wrong_accept() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/conversion")
        .header(header::CONTENT_TYPE, "text/markdown")
        .header(header::ACCEPT, "application/json")
        .body(Body::from("# Hello"))
        .unwrap();

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
}

#[tokio::test]
async fn post_conversion_missing_accept() {
    let response = app()
        .oneshot(post_markdown("# Hello"))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn post_conversion_wildcard_accept() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/conversion")
        .header(header::CONTENT_TYPE, "text/markdown")
        .header(header::ACCEPT, "*/*")
        .body(Body::from("# Hello"))
        .unwrap();

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn post_conversion_alias_accept() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/conversion")
        .header(header::CONTENT_TYPE, "text/markdown")
        .header(header::ACCEPT, "application/vnd.blocknote+json")
        .body(Body::from("# Hello"))
        .unwrap();

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn post_conversion_invalid_utf8() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/conversion")
        .header(header::CONTENT_TYPE, "text/markdown")
        .body(Body::from(Bytes::from_static(&[0xFF, 0xFE])))
        .unwrap();

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response_body_json(response.into_body()).await;
    let detail = body
        .get("detail")
        .expect("detail field present")
        .as_str()
        .expect("detail is a string");
    assert!(
        detail.contains("not valid UTF-8"),
        "unexpected detail: {detail}"
    );
}

#[tokio::test]
async fn options_conversion_returns_204() {
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/conversion")
        .body(Body::empty())
        .unwrap();

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get(header::ALLOW)
            .expect("allow header present"),
        "POST"
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
    let request = Request::builder()
        .method(Method::PUT)
        .uri("/conversion")
        .body(Body::empty())
        .unwrap();

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        response
            .headers()
            .get(header::ALLOW)
            .expect("allow header present"),
        "POST"
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
}

#[tokio::test]
async fn delete_conversion_returns_405() {
    let request = Request::builder()
        .method(Method::DELETE)
        .uri("/conversion")
        .body(Body::empty())
        .unwrap();

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        response
            .headers()
            .get(header::ALLOW)
            .expect("allow header present"),
        "POST"
    );
}

#[tokio::test]
async fn get_health_returns_200() {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/health")
        .body(Body::empty())
        .unwrap();

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
    let request = Request::builder()
        .method(Method::HEAD)
        .uri("/health")
        .body(Body::empty())
        .unwrap();

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
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/health")
        .body(Body::empty())
        .unwrap();

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
}

#[tokio::test]
async fn put_health_returns_405() {
    let request = Request::builder()
        .method(Method::PUT)
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        response
            .headers()
            .get(header::ALLOW)
            .expect("allow header present"),
        "GET, HEAD, OPTIONS"
    );
}

#[tokio::test]
async fn unknown_path_returns_404() {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/unknown")
        .body(Body::empty())
        .unwrap();

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
    assert_problem_json(&body);
}

#[tokio::test]
async fn query_string_not_in_404_detail() {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/unknown?secret=password")
        .body(Body::empty())
        .unwrap();

    let response = app().oneshot(request).await.expect("request succeeds");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = response_body_json(response.into_body()).await;
    let detail = body
        .get("detail")
        .expect("detail field present")
        .as_str()
        .expect("detail is a string");
    assert!(
        !detail.contains("secret"),
        "query param 'secret' leaked into detail: {detail}"
    );
    assert!(
        !detail.contains("password"),
        "query param 'password' leaked into detail: {detail}"
    );
}
