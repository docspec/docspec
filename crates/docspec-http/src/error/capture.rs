//! Telemetry capture for HTTP errors.

use axum::http::StatusCode;

use super::HttpError;

/// Constructs a `PostHog` `$exception` event for an HTTP error.
///
/// The `sentry_event_id` property correlates this event with the corresponding
/// Sentry event when both backends are active. Each call uses a fresh `distinct_id`
/// (per-event UUID with `$is_anonymous = true`) because the sync `IntoResponse`
/// context does not have access to the request ID.
#[cfg(feature = "posthog")]
fn build_posthog_error_event(detail: &str, sentry_event_id: Option<String>) -> posthog_rs::Event {
    let distinct_id = uuid::Uuid::new_v4().to_string();
    let mut event = posthog_rs::Event::new("$exception", &distinct_id);
    drop(event.insert_prop("$exception_message", detail));
    drop(event.insert_prop("$is_anonymous", true));
    if let Some(sentry_id) = sentry_event_id {
        drop(event.insert_prop("sentry_event_id", sentry_id));
    }
    event
}

#[cfg(feature = "sentry")]
#[inline]
fn capture_sentry(
    detail: &str,
    source: Option<&(dyn core::error::Error + Send + Sync + 'static)>,
) -> sentry::types::Uuid {
    source.map_or_else(
        || sentry::capture_message(detail, sentry::Level::Error),
        sentry::capture_error,
    )
}

#[cfg(any(feature = "sentry", feature = "posthog"))]
fn capture_error_telemetry(
    status: StatusCode,
    detail: &str,
    source: Option<&(dyn core::error::Error + Send + Sync + 'static)>,
) {
    if status != StatusCode::INTERNAL_SERVER_ERROR && status != StatusCode::UNPROCESSABLE_ENTITY {
        return;
    }

    #[cfg(not(feature = "sentry"))]
    let _ = source;

    #[cfg(all(feature = "sentry", not(feature = "posthog")))]
    {
        let _ = capture_sentry(detail, source);
    }
    #[cfg(all(feature = "sentry", feature = "posthog"))]
    let sentry_id_string: Option<String> = Some(capture_sentry(detail, source).to_string());
    #[cfg(all(not(feature = "sentry"), feature = "posthog"))]
    let sentry_id_string: Option<String> = None;
    #[cfg(feature = "posthog")]
    {
        let client_arc: Option<std::sync::Arc<posthog_rs::Client>> =
            match crate::telemetry::posthog_client_slot().read() {
                Ok(guard) => guard.as_ref().map(std::sync::Arc::clone),
                Err(_poisoned) => {
                    tracing::warn!("posthog slot poisoned; skipping error capture");
                    None
                }
            };
        if let Some(client) = client_arc {
            let event = build_posthog_error_event(detail, sentry_id_string);
            client.capture(event);
        }
    }
}

impl HttpError {
    /// Capture this error to configured telemetry backends (Sentry, `PostHog`)
    /// when the variant is a 500 or 422.
    #[cfg(any(feature = "sentry", feature = "posthog"))]
    pub(crate) fn capture_to_telemetry(&self) {
        match self {
            Self::Internal { source } => {
                capture_error_telemetry(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An unexpected error occurred during conversion",
                    Some(source.as_ref()),
                );
            }
            Self::Unprocessable { detail } => {
                capture_error_telemetry(StatusCode::UNPROCESSABLE_ENTITY, detail.as_str(), None);
            }
            Self::BodyNotUtf8
            | Self::EmptyBody
            | Self::MethodNotAllowed { .. }
            | Self::NotAcceptable
            | Self::NotFound { .. }
            | Self::UnsupportedMediaType { .. } => {}
        }
    }
}
