//! In-process integration tests for Sentry capture behavior.

// Reason: integration tests use standard test patterns with expect/unwrap/indexing.
#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]

use std::sync::Mutex;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    match ENV_MUTEX.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[test]
fn internal_error_captured_with_request_id_tag() {
    let _env_guard = lock_env();
    let events = sentry::test::with_captured_events(|| {
        use axum::response::IntoResponse as _;
        drop(docspec_http::error::HttpError::Internal.into_response());
    });
    assert_eq!(
        events.len(),
        1,
        "expected exactly 1 captured event for Internal error"
    );
    let event = events.first().expect("expected event");
    assert_eq!(event.level, sentry::Level::Error);
    assert!(
        event
            .request
            .as_ref()
            .and_then(|r| r.data.as_ref())
            .is_none(),
        "request body must not be captured (G2)"
    );
}

#[test]
fn unprocessable_error_captured() {
    let _env_guard = lock_env();
    let events = sentry::test::with_captured_events(|| {
        use axum::response::IntoResponse as _;
        drop(
            docspec_http::error::HttpError::Unprocessable {
                detail: "heading level jumped from 1 to 3".to_owned(),
            }
            .into_response(),
        );
    });
    assert_eq!(
        events.len(),
        1,
        "expected exactly 1 captured event for Unprocessable error"
    );
    assert_eq!(
        events.first().expect("expected event").level,
        sentry::Level::Error
    );
}

#[test]
fn client_errors_not_captured() {
    let _env_guard = lock_env();
    let events = sentry::test::with_captured_events(|| {
        use axum::response::IntoResponse as _;
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
        drop(
            docspec_http::error::HttpError::UnsupportedMediaType { received: None }.into_response(),
        );
    });
    assert_eq!(events.len(), 0, "4xx errors must not be captured (G4)");
}

#[test]
fn telemetry_layers_none_when_dsn_absent() {
    let _env_guard = lock_env();
    std::env::remove_var("DOCSPEC_SENTRY_DSN");
    std::env::remove_var("SENTRY_DSN");

    let _telemetry_guard = docspec_http::telemetry::init();

    let tracing = docspec_http::telemetry::tracing_layer::<tracing_subscriber::Registry>();
    assert!(tracing.is_none(), "tracing_layer must be None when no DSN");

    let new_layer = docspec_http::telemetry::tower_new_layer();
    assert!(
        new_layer.is_none(),
        "tower_new_layer must be None when no DSN"
    );

    let http_layer = docspec_http::telemetry::tower_http_layer();
    assert!(
        http_layer.is_none(),
        "tower_http_layer must be None when no DSN"
    );
}

#[test]
fn body_not_in_extras_or_contexts() {
    let _env_guard = lock_env();
    let events = sentry::test::with_captured_events(|| {
        use axum::response::IntoResponse as _;
        drop(docspec_http::error::HttpError::Internal.into_response());
    });
    assert_eq!(events.len(), 1);
    let event = events.first().expect("expected event");
    for key in event.extra.keys() {
        assert!(
            !key.to_lowercase().contains("body"),
            "extra key '{key}' contains 'body' \u{2014} body data must not be captured"
        );
    }
    for key in event.contexts.keys() {
        assert!(
            !key.to_lowercase().contains("body"),
            "context key '{key}' contains 'body' \u{2014} body data must not be captured"
        );
    }
    assert!(
        event
            .request
            .as_ref()
            .and_then(|r| r.data.as_ref())
            .is_none(),
        "request.data must be None (G2)"
    );
}
