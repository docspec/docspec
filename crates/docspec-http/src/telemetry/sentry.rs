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
    let mut options = sentry::ClientOptions::new()
        .maybe_release(sentry::release_name!())
        .environment(env_environment())
        .sample_rate(env_sample_rate())
        .send_default_pii(false)
        .attach_stacktrace(true)
        .before_send(before_send);

    // Zero must stay `Disabled`, not `FixedRate(0.0)`: the latter still honours
    // an inherited sampling decision from an incoming trace.
    let traces_sample_rate = env_traces_sample_rate();
    if traces_sample_rate > 0.0 {
        options = options.traces_sample_rate(traces_sample_rate);
    }

    // Assigned directly because the builder's `dsn` takes a `&str` and panics on
    // parse failure; this DSN is already parsed.
    options.dsn = Some(parsed_data_source_name);
    options
}

fn env_environment() -> std::borrow::Cow<'static, str> {
    match std::env::var("SENTRY_ENVIRONMENT") {
        Ok(value) if !value.is_empty() => std::borrow::Cow::Owned(value),
        _ => std::borrow::Cow::Borrowed("production"),
    }
}

fn env_sample_rate() -> f32 {
    super::env_rate("SENTRY_SAMPLE_RATE", 1.0)
}

fn env_traces_sample_rate() -> f32 {
    super::env_rate("SENTRY_TRACES_SAMPLE_RATE", 0.0)
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
    // Reason: tests that parse a DSN with `?` still assert with `assert!`;
    // the lint only makes sense for production code.
    #![allow(clippy::panic_in_result_fn)]

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

    #[test]
    fn env_traces_sample_rate_ignores_non_finite() {
        let _env_guard = lock_env();
        std::env::set_var("SENTRY_TRACES_SAMPLE_RATE", "NaN");

        assert_eq!(super::env_traces_sample_rate().to_bits(), f32::to_bits(0.0));

        std::env::remove_var("SENTRY_TRACES_SAMPLE_RATE");
    }

    #[test]
    fn client_options_enables_attach_stacktrace() -> Result<(), Box<dyn core::error::Error>> {
        let _env_guard = lock_env();
        let dsn: sentry::types::Dsn = "https://public@example.com/1".parse()?;
        let opts = super::client_options(dsn);
        if opts.attach_stacktrace {
            Ok(())
        } else {
            Err("attach_stacktrace should be true".into())
        }
    }

    #[test]
    fn client_options_sets_dsn_release_environment_and_sample_rate(
    ) -> Result<(), Box<dyn core::error::Error>> {
        let _env_guard = lock_env();
        std::env::set_var("SENTRY_ENVIRONMENT", "staging");
        std::env::set_var("SENTRY_SAMPLE_RATE", "0.25");
        let dsn: sentry::types::Dsn = "https://public@example.com/1".parse()?;

        let opts = super::client_options(dsn.clone());

        std::env::remove_var("SENTRY_ENVIRONMENT");
        std::env::remove_var("SENTRY_SAMPLE_RATE");

        assert_eq!(opts.dsn, Some(dsn));
        assert_eq!(opts.release, sentry::release_name!());
        assert_eq!(
            opts.environment,
            Some(std::borrow::Cow::Borrowed("staging"))
        );
        assert!(!opts.send_default_pii);
        assert!(opts.before_send.is_some());
        assert!(matches!(
            opts.event_sampling_strategy,
            sentry::EventSamplingStrategy::FixedRate(rate) if rate.to_bits() == f32::to_bits(0.25)
        ));
        Ok(())
    }

    #[test]
    fn client_options_leaves_traces_disabled_at_zero() -> Result<(), Box<dyn core::error::Error>> {
        let _env_guard = lock_env();
        std::env::remove_var("SENTRY_TRACES_SAMPLE_RATE");
        let dsn: sentry::types::Dsn = "https://public@example.com/1".parse()?;

        let opts = super::client_options(dsn);

        assert!(matches!(
            opts.traces_sampling_strategy,
            sentry::TracesSamplingStrategy::Disabled
        ));
        Ok(())
    }

    #[test]
    fn client_options_sets_fixed_traces_rate_above_zero() -> Result<(), Box<dyn core::error::Error>>
    {
        let _env_guard = lock_env();
        std::env::set_var("SENTRY_TRACES_SAMPLE_RATE", "0.5");
        let dsn: sentry::types::Dsn = "https://public@example.com/1".parse()?;

        let opts = super::client_options(dsn);

        std::env::remove_var("SENTRY_TRACES_SAMPLE_RATE");

        assert!(matches!(
            opts.traces_sampling_strategy,
            sentry::TracesSamplingStrategy::FixedRate(rate) if rate.to_bits() == f32::to_bits(0.5)
        ));
        Ok(())
    }
}
