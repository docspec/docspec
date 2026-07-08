//! Integration tests for `PostHog` capture behavior.
#![cfg(feature = "posthog")]
// Reason: integration tests use standard test patterns.
#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::await_holding_lock
)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Mutex;

use axum::response::IntoResponse as _;
use docspec_http::telemetry::posthog_client_slot;
use tower::ServiceExt as _;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    match ENV_MUTEX.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn dummy_error() -> std::io::Error {
    std::io::Error::other("dummy internal error")
}

/// Helper to initialize a `PostHog` client pointing at the wiremock server.
async fn init_test_posthog_client(
    mock_server: &wiremock::MockServer,
) -> std::sync::Arc<posthog_rs::Client> {
    std::env::set_var("DOCSPEC_POSTHOG_API_KEY", "phc_test_key");
    std::env::set_var("DOCSPEC_POSTHOG_HOST", mock_server.uri());
    std::env::remove_var("POSTHOG_SAMPLE_RATE");
    let client = docspec_http::telemetry::init_posthog_client_from_env()
        .await
        .expect("client should init with test key and mock host");
    let mut slot = posthog_client_slot().write().expect("slot write");
    *slot = Some(std::sync::Arc::clone(&client));
    drop(slot);
    client
}

/// Flush all pending `PostHog` events and clear the slot.
async fn flush_and_clear_slot(client: std::sync::Arc<posthog_rs::Client>) {
    client.shutdown().await;
    let mut slot = posthog_client_slot().write().expect("slot write");
    *slot = None;
}

fn install_posthog_client(client: std::sync::Arc<posthog_rs::Client>) {
    let mut slot = posthog_client_slot().write().expect("slot write");
    *slot = Some(client);
}

fn clear_posthog_client() {
    let mut slot = posthog_client_slot().write().expect("slot write");
    *slot = None;
}

/// Mount a catch-all stub on all `PostHog` ingest paths.
async fn mount_posthog_stub(mock_server: &wiremock::MockServer) {
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": 1})),
        )
        .mount(mock_server)
        .await;
}

/// Extracts the first event from a `PostHog` ingest request body.
fn find_event_in_posthog_body(body: &serde_json::Value) -> serde_json::Value {
    if let Some(batch) = body.get("batch") {
        if let Some(arr) = batch.as_array() {
            return arr.first().cloned().unwrap_or(serde_json::Value::Null);
        }
    }
    body.clone()
}

#[cfg(feature = "sentry")]
#[tokio::test]
async fn internal_error_dual_fire_includes_sentry_event_id() {
    let _env_guard = lock_env();
    let mock_server = wiremock::MockServer::start().await;
    mount_posthog_stub(&mock_server).await;
    let client = init_test_posthog_client(&mock_server).await;

    drop(docspec_http::error::HttpError::internal(dummy_error()).into_response());

    flush_and_clear_slot(client).await;

    let requests = mock_server.received_requests().await.unwrap_or_default();
    assert_eq!(requests.len(), 1, "exactly 1 PostHog capture for 500");

    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("valid JSON");
    let event = find_event_in_posthog_body(&body);
    assert!(
        event["properties"].get("sentry_event_id").is_some(),
        "sentry_event_id must be present in properties when sentry feature is active; body={body}"
    );
    let sentry_id = event["properties"]["sentry_event_id"]
        .as_str()
        .expect("sentry_event_id is str");
    assert_eq!(sentry_id.len(), 36, "sentry_event_id should be UUID format");
}

#[cfg(not(feature = "sentry"))]
#[tokio::test]
async fn internal_error_posthog_only_no_sentry_id() {
    let _env_guard = lock_env();
    let mock_server = wiremock::MockServer::start().await;
    mount_posthog_stub(&mock_server).await;
    let client = init_test_posthog_client(&mock_server).await;

    drop(docspec_http::error::HttpError::internal(dummy_error()).into_response());

    flush_and_clear_slot(client).await;

    let requests = mock_server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests.len(),
        1,
        "exactly 1 PostHog capture for 500 (posthog only)"
    );

    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("valid JSON");
    let event = find_event_in_posthog_body(&body);
    assert!(
        event["properties"].get("sentry_event_id").is_none(),
        "sentry_event_id must NOT be present when sentry feature is disabled; body={body}"
    );
}

#[cfg(feature = "sentry")]
#[tokio::test]
async fn unprocessable_error_dual_fire_includes_sentry_event_id() {
    let _env_guard = lock_env();
    let mock_server = wiremock::MockServer::start().await;
    mount_posthog_stub(&mock_server).await;
    let client = init_test_posthog_client(&mock_server).await;

    drop(
        docspec_http::error::HttpError::Unprocessable {
            detail: "test unprocessable error".to_owned(),
        }
        .into_response(),
    );

    flush_and_clear_slot(client).await;

    let requests = mock_server.received_requests().await.unwrap_or_default();
    assert_eq!(requests.len(), 1, "exactly 1 PostHog capture for 422");

    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("valid JSON");
    let event = find_event_in_posthog_body(&body);
    assert!(
        event["properties"].get("sentry_event_id").is_some(),
        "sentry_event_id must be present for 422; body={body}"
    );
}

#[tokio::test]
async fn client_errors_not_captured_by_posthog() {
    let _env_guard = lock_env();
    let mock_server = wiremock::MockServer::start().await;
    mount_posthog_stub(&mock_server).await;
    let client = init_test_posthog_client(&mock_server).await;

    drop(docspec_http::error::HttpError::EmptyBody.into_response());
    drop(docspec_http::error::HttpError::BodyNotUtf8.into_response());
    drop(
        docspec_http::error::HttpError::NotFound {
            method: "GET".to_owned(),
            path: "/unknown".to_owned(),
        }
        .into_response(),
    );
    drop(
        docspec_http::error::HttpError::MethodNotAllowed {
            allowed: "GET, POST",
        }
        .into_response(),
    );
    drop(docspec_http::error::HttpError::NotAcceptable.into_response());
    drop(docspec_http::error::HttpError::UnsupportedMediaType { received: None }.into_response());

    flush_and_clear_slot(client).await;

    let requests = mock_server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests.len(),
        0,
        "4xx errors must NOT trigger PostHog capture; body={requests:?}"
    );
}

#[tokio::test]
async fn successful_conversion_captures_conversion_completed() {
    let _env_guard = lock_env();
    let mock_server = wiremock::MockServer::start().await;
    mount_posthog_stub(&mock_server).await;
    let client = init_test_posthog_client(&mock_server).await;

    let app = common::router();
    let req = common::markdown_request("# Hello PostHog");

    let response = app.oneshot(req).await.expect("request succeeds");
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    flush_and_clear_slot(client).await;

    let requests = mock_server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests.len(),
        1,
        "exactly 1 conversion_completed PostHog capture"
    );

    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("valid JSON");
    let event = find_event_in_posthog_body(&body);

    assert_eq!(
        event["event"].as_str().unwrap_or(""),
        "conversion_completed"
    );
    assert_eq!(
        event["properties"]["result"].as_str().unwrap_or(""),
        "success"
    );
    assert_eq!(
        event["properties"]["error_class"].as_str().unwrap_or(""),
        "none"
    );
    assert_eq!(
        event["properties"]["input_mime_type"]
            .as_str()
            .unwrap_or(""),
        "text/markdown"
    );
    assert!(event["properties"]["input_bytes"].as_u64().unwrap_or(0) > 0);
    assert!(event["properties"]["output_bytes"].as_u64().unwrap_or(0) > 0);
    assert!(event["properties"]["duration_ms"].as_u64().is_some());
}

#[tokio::test]
async fn zero_sample_rate_sends_no_events() {
    let _env_guard = lock_env();
    let mock_server = wiremock::MockServer::start().await;
    mount_posthog_stub(&mock_server).await;

    std::env::set_var("DOCSPEC_POSTHOG_API_KEY", "phc_test_key");
    std::env::set_var("DOCSPEC_POSTHOG_HOST", mock_server.uri());
    std::env::set_var("POSTHOG_SAMPLE_RATE", "0");

    let client = docspec_http::telemetry::init_posthog_client_from_env()
        .await
        .expect("client should init even with sample rate 0");
    install_posthog_client(std::sync::Arc::clone(&client));

    drop(docspec_http::error::HttpError::internal(dummy_error()).into_response());

    client.shutdown().await;
    clear_posthog_client();
    std::env::remove_var("POSTHOG_SAMPLE_RATE");

    let requests = mock_server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests.len(),
        0,
        "POSTHOG_SAMPLE_RATE=0 must send zero events; received: {requests:?}"
    );
}

#[tokio::test]
async fn flush_on_shutdown_delivers_all_events_before_returning() {
    let _env_guard = lock_env();
    let mock_server = wiremock::MockServer::start().await;
    mount_posthog_stub(&mock_server).await;

    std::env::set_var("DOCSPEC_POSTHOG_API_KEY", "phc_test_key");
    std::env::set_var("DOCSPEC_POSTHOG_HOST", mock_server.uri());
    std::env::remove_var("POSTHOG_SAMPLE_RATE");

    let client = docspec_http::telemetry::init_posthog_client_from_env()
        .await
        .expect("client should init");
    install_posthog_client(client);

    drop(docspec_http::error::HttpError::internal(dummy_error()).into_response());

    docspec_http::telemetry::shutdown().await;
    clear_posthog_client();

    let requests = mock_server.received_requests().await.unwrap_or_default();
    assert_eq!(
        requests.len(),
        1,
        "flush-on-shutdown must deliver the queued event before returning; received: {requests:?}"
    );
}

#[tokio::test]
async fn install_posthog_client_none_clears_stale_client_from_slot() {
    let _env_guard = lock_env();
    let mock_server = wiremock::MockServer::start().await;
    mount_posthog_stub(&mock_server).await;

    let stale_client = init_test_posthog_client(&mock_server).await;
    assert!(
        posthog_client_slot().read().expect("slot read").is_some(),
        "stale client should be installed before the fix runs"
    );

    std::env::remove_var("DOCSPEC_POSTHOG_API_KEY");
    std::env::remove_var("POSTHOG_API_KEY");
    std::env::remove_var("DOCSPEC_POSTHOG_HOST");
    std::env::remove_var("POSTHOG_HOST");
    let fresh_client = docspec_http::telemetry::init_posthog_client_from_env().await;
    assert!(
        fresh_client.is_none(),
        "factory must return None when no API key is configured"
    );

    docspec_http::telemetry::install_posthog_client(fresh_client);

    assert!(
        posthog_client_slot()
            .read()
            .expect("slot read")
            .is_none(),
        "install_posthog_client(None) must clear the stale client so subsequent captures find no client"
    );

    stale_client.shutdown().await;
}

#[tokio::test]
async fn install_posthog_client_some_replaces_previous_client_in_slot() {
    let _env_guard = lock_env();
    let mock_server = wiremock::MockServer::start().await;
    mount_posthog_stub(&mock_server).await;

    let first_client = init_test_posthog_client(&mock_server).await;
    let second_client = docspec_http::telemetry::init_posthog_client_from_env()
        .await
        .expect("second init should succeed with same env");
    docspec_http::telemetry::install_posthog_client(Some(std::sync::Arc::clone(&second_client)));

    let stored = posthog_client_slot()
        .read()
        .expect("slot read")
        .as_ref()
        .map(std::sync::Arc::clone)
        .expect("slot should hold a client after install(Some)");
    assert!(
        std::sync::Arc::ptr_eq(&stored, &second_client),
        "slot must hold the second client after replacement"
    );
    assert!(
        !std::sync::Arc::ptr_eq(&stored, &first_client),
        "slot must NOT still hold the first client after replacement"
    );

    first_client.shutdown().await;
    second_client.shutdown().await;
    docspec_http::telemetry::install_posthog_client(None);
    std::env::remove_var("DOCSPEC_POSTHOG_API_KEY");
    std::env::remove_var("DOCSPEC_POSTHOG_HOST");
}
