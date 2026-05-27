//! Integration tests for the `DocSpec` HTTP API.

#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::single_call_fn
)]
// Reason: Integration test code uses unwrap, index notation, and small helpers for assertion clarity.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use bytes::Bytes;
use docspec_http::router::router;
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

async fn collect_body(body: Body) -> Vec<u8> {
    body.collect().await.unwrap().to_bytes().to_vec()
}

fn markdown_request(body: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/convert")
        .header(header::CONTENT_TYPE, "text/markdown")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn charset_parameter_accepted() {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/convert")
        .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
        .body(Body::from("# Hello\n"))
        .unwrap();
    let resp = router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn concurrent_ten_requests() {
    let mut set = tokio::task::JoinSet::new();
    for i in 0..10 {
        let req = markdown_request(&format!("# Request {i}\n"));
        set.spawn(router().oneshot(req));
    }
    while let Some(result) = set.join_next().await {
        let resp = result.unwrap().unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn empty_body_returns_200_empty_array() {
    let resp = router().oneshot(markdown_request("")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = collect_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json.is_array());
}

#[tokio::test]
async fn happy_path_returns_200_blocknote() {
    let resp = router()
        .oneshot(markdown_request("# Hello\n"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/vnd.docspec.blocknote+json"
    );
    let bytes = collect_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json.is_array());
}

#[tokio::test]
async fn health_returns_204_no_content() {
    let req = Request::builder()
        .method(Method::GET)
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(resp.headers().get(header::CONTENT_TYPE).is_none());
    let bytes = collect_body(resp.into_body()).await;
    assert!(bytes.is_empty());
}

#[tokio::test]
async fn invalid_utf8_returns_400() {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/convert")
        .header(header::CONTENT_TYPE, "text/markdown")
        .body(Body::from(Bytes::from(vec![0xFF, 0xFE, 0x00])))
        .unwrap();
    let resp = router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn large_markdown_returns_200() {
    let large_body = "# Heading\n\nParagraph text. ".repeat(40_000);
    let resp = router()
        .oneshot(markdown_request(&large_body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = collect_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json.is_array());
}

#[tokio::test]
async fn missing_content_type_returns_415() {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/convert")
        .body(Body::from("# Hello\n"))
        .unwrap();
    let resp = router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn unacceptable_accept_returns_406() {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/convert")
        .header(header::CONTENT_TYPE, "text/markdown")
        .header(header::ACCEPT, "text/html")
        .body(Body::from("# Hello\n"))
        .unwrap();
    let resp = router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_ACCEPTABLE);
}

#[tokio::test]
async fn unknown_route_returns_404_problem_json() {
    let req = Request::builder()
        .method(Method::GET)
        .uri("/unknown")
        .body(Body::empty())
        .unwrap();
    let resp = router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
    let bytes = collect_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["type"], "https://docspec.dev/errors/not-found");
}

#[tokio::test]
async fn wildcard_accept_returns_200() {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/convert")
        .header(header::CONTENT_TYPE, "text/markdown")
        .header(header::ACCEPT, "*/*")
        .body(Body::from("# Hello\n"))
        .unwrap();
    let resp = router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn wrong_content_type_returns_415() {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/convert")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let resp = router().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
}
