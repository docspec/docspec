//! Tests for the `OpenAPI` document served at `GET /openapi.json`.

// Reason: integration tests use standard test patterns with expect/unwrap.
#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt as _;

const MIME_OPENAPI: &str = "application/vnd.oai.openapi+json;version=3.1";

fn app() -> Router {
    docspec_http::router::router()
}

async fn get_json(uri: &str) -> Value {
    let response = app()
        .oneshot(common::empty_request("GET", uri))
        .await
        .expect("request succeeds");
    assert_eq!(response.status(), StatusCode::OK, "GET {uri}");
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body read");
    serde_json::from_slice(&bytes).expect("valid JSON")
}

async fn openapi() -> Value {
    get_json("/openapi.json").await
}

/// Validation errors of `instance` against `schema`, as readable strings.
fn errors(schema: &Value, instance: &Value) -> Vec<String> {
    let validator = jsonschema::validator_for(schema).expect("valid JSON Schema");
    validator
        .iter_errors(instance)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect()
}

#[tokio::test]
async fn get_openapi_returns_openapi_document() {
    let response = app()
        .oneshot(common::empty_request("GET", "/openapi.json"))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        MIME_OPENAPI
    );
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body read");
    let document: Value = serde_json::from_slice(&bytes).expect("valid JSON");
    assert_eq!(document.get("openapi"), Some(&json!("3.1.0")));
    assert_eq!(
        document.get("info"),
        Some(&json!({
            "title": "DocSpec HTTP API",
            "description": "Streaming document conversion. Errors are RFC 9457 problem details. \
                            `HEAD` is served wherever `GET` is. Undocumented paths answer 404 and \
                            other undocumented methods 405, both as problem details. `GET /metrics` is served only when metrics are \
                            enabled and is not described here.",
            "version": env!("CARGO_PKG_VERSION"),
        }))
    );
}

#[tokio::test]
async fn head_openapi_is_served_like_get() {
    let response = app()
        .oneshot(common::empty_request("HEAD", "/openapi.json"))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type present"),
        MIME_OPENAPI
    );
}

#[tokio::test]
async fn post_openapi_returns_405() {
    let response = app()
        .oneshot(common::empty_request("POST", "/openapi.json"))
        .await
        .expect("request succeeds");

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        response
            .headers()
            .get(header::ALLOW)
            .expect("allow header present"),
        "GET, HEAD"
    );
}

#[tokio::test]
async fn openapi_document_is_valid_openapi_3_1() {
    let schema: Value =
        serde_json::from_str(include_str!("fixtures/oas-3.1-schema.json")).expect("schema");

    assert_eq!(errors(&schema, &openapi().await), Vec::<String>::new());
}

#[test]
fn oas_schema_rejects_document_without_info() {
    let schema: Value =
        serde_json::from_str(include_str!("fixtures/oas-3.1-schema.json")).expect("schema");

    assert_eq!(
        errors(&schema, &json!({"openapi": "3.1.0", "paths": {}})),
        [r#""info" is a required property at "#]
    );
}

#[tokio::test]
async fn openapi_documents_exactly_the_served_operations() {
    let document = openapi().await;
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .expect("paths");

    let documented: Vec<(String, String)> = paths
        .iter()
        .flat_map(|(path, item)| {
            item.as_object()
                .expect("path item")
                .keys()
                .map(move |method| (path.clone(), method.clone()))
        })
        .collect();
    assert_eq!(
        documented,
        [
            ("/.well-known/docspec", "get"),
            ("/conversion", "get"),
            ("/conversion", "options"),
            ("/conversion", "post"),
            ("/health", "get"),
            ("/health", "head"),
            ("/health", "options"),
            ("/openapi.json", "get"),
        ]
        .map(|(path, method)| (path.to_owned(), method.to_owned()))
    );

    for (path, method) in documented {
        let request = Request::builder()
            .method(method.to_uppercase().as_str())
            .uri(&path)
            .body(Body::empty())
            .expect("request");
        let status = app()
            .oneshot(request)
            .await
            .expect("request succeeds")
            .status();
        assert!(
            status != StatusCode::NOT_FOUND && status != StatusCode::METHOD_NOT_ALLOWED,
            "{method} {path} is documented but answered {status}"
        );
    }
}

/// Validates the live body of `uri` against the named component schema.
async fn assert_matches_component(uri: &str, component: &str) {
    let document = openapi().await;
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": format!("#/components/schemas/{component}"),
        "components": document.get("components").expect("components"),
    });

    assert_eq!(
        errors(&schema, &get_json(uri).await),
        Vec::<String>::new(),
        "GET {uri} body does not match the {component} schema"
    );
}

#[tokio::test]
async fn get_conversion_body_matches_its_schema() {
    assert_matches_component("/conversion", "ConversionFormats").await;
}

#[tokio::test]
async fn well_known_docspec_body_matches_its_schema() {
    assert_matches_component("/.well-known/docspec", "WellKnown").await;
}

#[tokio::test]
async fn problem_body_matches_its_schema() {
    let document = openapi().await;
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/components/schemas/Problem",
        "components": document.get("components").expect("components"),
    });
    let response = app()
        .oneshot(common::request(
            "POST",
            "/conversion",
            &[("content-type", "application/json")],
            Body::from("{}"),
        ))
        .await
        .expect("request succeeds");
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body read");
    let problem: Value = serde_json::from_slice(&bytes).expect("valid JSON");

    assert_eq!(errors(&schema, &problem), Vec::<String>::new());
}
