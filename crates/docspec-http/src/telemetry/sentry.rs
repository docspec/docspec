//! Sentry telemetry backend internals.

use core::str::FromStr as _;

/// Initializes the Sentry SDK with sanitized client options.
pub(in crate::telemetry) fn init_sentry(
    data_source_name: &str,
) -> Option<::sentry::ClientInitGuard> {
    sentry::types::Dsn::from_str(data_source_name).map_or_else(
        |_| {
            eprintln!("warning: invalid Sentry DSN format; Sentry disabled");
            None
        },
        |parsed_data_source_name| Some(sentry::init(client_options(parsed_data_source_name))),
    )
}

fn client_options(parsed_data_source_name: sentry::types::Dsn) -> sentry::ClientOptions {
    sentry::ClientOptions {
        dsn: Some(parsed_data_source_name),
        release: sentry::release_name!(),
        environment: Some(env_environment()),
        sample_rate: env_sample_rate(),
        traces_sample_rate: env_traces_sample_rate(),
        send_default_pii: false,
        before_send: Some(std::sync::Arc::new(before_send)),
        ..Default::default()
    }
}

fn env_environment() -> std::borrow::Cow<'static, str> {
    match std::env::var("SENTRY_ENVIRONMENT") {
        Ok(value) if !value.is_empty() => std::borrow::Cow::Owned(value),
        _ => std::borrow::Cow::Borrowed("production"),
    }
}

fn env_sample_rate() -> f32 {
    env_rate("SENTRY_SAMPLE_RATE", 1.0)
}

fn env_traces_sample_rate() -> f32 {
    env_rate("SENTRY_TRACES_SAMPLE_RATE", 0.0)
}

fn env_rate(name: &str, default: f32) -> f32 {
    std::env::var(name).map_or(default, |value| {
        value.parse::<f32>().map_or_else(
            |_| {
                eprintln!(
                    "warning: {name} invalid or out-of-range; clamped to default {default:.1}"
                );
                default
            },
            |rate| rate.clamp(0.0, 1.0),
        )
    })
}

fn before_send(
    mut event: sentry::protocol::Event<'static>,
) -> Option<sentry::protocol::Event<'static>> {
    if let Some(request) = event.request.as_mut() {
        let _ = request.data.take();
    }
    event
        .extra
        .retain(|extra_key, _| !extra_key.to_lowercase().contains("body"));
    Some(event)
}

/// Returns the configured Sentry tracing layer.
pub(in crate::telemetry) fn tracing_layer<S>() -> sentry::integrations::tracing::SentryLayer<S>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    sentry::integrations::tracing::layer().event_filter(|metadata| {
        use sentry::integrations::tracing::EventFilter;

        match *metadata.level() {
            tracing::Level::ERROR => EventFilter::Event,
            tracing::Level::WARN | tracing::Level::INFO | tracing::Level::DEBUG => {
                EventFilter::Breadcrumb
            }
            tracing::Level::TRACE => EventFilter::Ignore,
        }
    })
}

/// Returns the Sentry tower hub binding layer.
pub(in crate::telemetry) fn tower_new_layer(
) -> sentry::integrations::tower::NewSentryLayer<axum::http::Request<axum::body::Body>> {
    sentry::integrations::tower::NewSentryLayer::new_from_top()
}

/// Returns the Sentry HTTP request enrichment layer.
pub(in crate::telemetry) fn tower_http_layer() -> sentry::integrations::tower::SentryHttpLayer {
    sentry::integrations::tower::SentryHttpLayer::new()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        match ENV_MUTEX.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    #[test]
    fn before_send_strips_request_data() {
        let event: sentry::protocol::Event<'static> = sentry::protocol::Event {
            request: Some(sentry::protocol::Request {
                data: Some(String::from("# secret document body")),
                ..sentry::protocol::Request::default()
            }),
            ..sentry::protocol::Event::default()
        };

        let stripped = super::before_send(event);

        assert!(matches!(
            stripped
                .as_ref()
                .and_then(|stripped_event| stripped_event.request.as_ref()),
            Some(request) if request.data.is_none()
        ));
    }

    #[test]
    fn before_send_strips_body_keys_in_extra() {
        let mut event: sentry::protocol::Event<'static> = sentry::protocol::Event::default();
        event
            .extra
            .insert(String::from("request_body"), serde_json::json!("secret"));
        event
            .extra
            .insert(String::from("response_body"), serde_json::json!("secret"));
        event
            .extra
            .insert(String::from("other"), serde_json::json!("kept"));

        let stripped = super::before_send(event);

        assert!(
            matches!(stripped, Some(stripped_event) if stripped_event.extra.len() == 1 && stripped_event.extra.contains_key("other"))
        );
    }

    #[test]
    fn env_sample_rate_clamps_high() {
        let _env_guard = lock_env();
        std::env::set_var("SENTRY_SAMPLE_RATE", "5.0");

        assert_eq!(super::env_sample_rate().to_bits(), f32::to_bits(1.0));

        std::env::remove_var("SENTRY_SAMPLE_RATE");
    }

    #[test]
    fn env_sample_rate_clamps_low() {
        let _env_guard = lock_env();
        std::env::set_var("SENTRY_SAMPLE_RATE", "-0.5");

        assert_eq!(super::env_sample_rate().to_bits(), f32::to_bits(0.0));

        std::env::remove_var("SENTRY_SAMPLE_RATE");
    }

    #[test]
    fn env_sample_rate_defaults_to_one() {
        let _env_guard = lock_env();
        std::env::remove_var("SENTRY_SAMPLE_RATE");

        assert_eq!(super::env_sample_rate().to_bits(), f32::to_bits(1.0));
    }

    #[test]
    fn env_traces_sample_rate_defaults_to_zero() {
        let _env_guard = lock_env();
        std::env::remove_var("SENTRY_TRACES_SAMPLE_RATE");

        assert_eq!(super::env_traces_sample_rate().to_bits(), f32::to_bits(0.0));
    }

    #[test]
    fn env_environment_defaults_to_production() {
        let _env_guard = lock_env();
        std::env::remove_var("SENTRY_ENVIRONMENT");

        assert_eq!(super::env_environment(), "production");
    }

    #[test]
    fn init_sentry_returns_none_on_invalid_dsn() {
        assert!(super::init_sentry("not-a-dsn").is_none());
    }
}
