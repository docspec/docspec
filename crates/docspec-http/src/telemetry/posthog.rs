//! `PostHog` telemetry backend.
//!
//! Activation requires the `posthog` Cargo feature. Set `DOCSPEC_POSTHOG_API_KEY`
//! (or `POSTHOG_API_KEY`) in the environment. All other configuration is optional.

/// `PostHog` client configuration resolved from environment variables.
pub(in crate::telemetry) struct PostHogConfig {
    /// `PostHog` project API key.
    api_key: String,
    /// `PostHog` ingest host URL.
    host: String,
}

/// Resolves `PostHog` configuration from environment variables.
///
/// Reads `DOCSPEC_POSTHOG_API_KEY` first, then `POSTHOG_API_KEY` as fallback.
/// Reads `DOCSPEC_POSTHOG_HOST` first, then `POSTHOG_HOST` as fallback;
/// defaults to `https://us.i.posthog.com`.
///
/// Returns `None` when neither API key variable is set or non-empty.
pub(in crate::telemetry) fn configured_posthog_config() -> Option<PostHogConfig> {
    let api_key = ["DOCSPEC_POSTHOG_API_KEY", "POSTHOG_API_KEY"]
        .iter()
        .find_map(|name| match std::env::var(name) {
            Ok(value) if !value.is_empty() => Some(value),
            _ => None,
        })?;

    let host = ["DOCSPEC_POSTHOG_HOST", "POSTHOG_HOST"]
        .iter()
        .find_map(|name| match std::env::var(name) {
            Ok(value) if !value.is_empty() => Some(value),
            _ => None,
        })
        .unwrap_or_else(|| "https://us.i.posthog.com".to_owned());

    Some(PostHogConfig { api_key, host })
}

/// `PostHog` event sample rate read from `POSTHOG_SAMPLE_RATE`. Default `1.0`,
/// clamped to `[0.0, 1.0]`.
pub(crate) fn env_sample_rate() -> f32 {
    super::env_rate("POSTHOG_SAMPLE_RATE", 1.0)
}

/// Release identifier sent with every `PostHog` analytics event.
pub(crate) const RELEASE: &str = concat!("docspec-http@", env!("CARGO_PKG_VERSION"));

/// Initializes a `PostHog` client from the provided configuration.
///
/// Wires in the sample-rate disable flag and the `before_send` hook.
/// Returns `None` when the SDK refuses to build with the given options,
/// logging a `warn` with the underlying error so operators have a
/// diagnostic signal instead of silent telemetry loss.
pub(in crate::telemetry) async fn init_posthog(
    config: PostHogConfig,
) -> Option<posthog_rs::Client> {
    tracing::debug!(release = %RELEASE, "initializing posthog client");
    let options = match posthog_rs::ClientOptionsBuilder::default()
        .api_key(config.api_key)
        .host(config.host)
        .disabled(env_sample_rate() <= 0.0)
        .is_server(true)
        .disable_geoip(false)
        .before_send(before_send)
        .build()
    {
        Ok(opts) => opts,
        Err(error) => {
            tracing::warn!(%error, "posthog client options build failed; disabling posthog");
            return None;
        }
    };
    Some(posthog_rs::client(options).await)
}

/// Strips any event property whose key contains "body" (case-insensitive).
///
/// Iterates `Event::properties()` (a `&HashMap`) to collect every key whose
/// lowercased form contains the substring "body", then removes each via
/// `remove_prop` — a two-pass approach because the iterator borrows the map
/// immutably. Because the capture sites (`error.rs`, `handlers/conversion.rs`)
/// never insert body keys, this hook is redundant in normal operation but
/// guards against future regressions.
fn before_send(mut event: posthog_rs::Event) -> Option<posthog_rs::Event> {
    let keys_to_remove: Vec<String> = event
        .properties()
        .keys()
        .filter(|key| key.to_ascii_lowercase().contains("body"))
        .cloned()
        .collect();

    for key in keys_to_remove {
        event.remove_prop(&key);
    }
    Some(event)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        match ENV_MUTEX.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    #[test]
    fn configured_posthog_config_prefers_docspec_api_key() {
        let _env_guard = lock_env();
        std::env::set_var("DOCSPEC_POSTHOG_API_KEY", "docspec-key");
        std::env::set_var("POSTHOG_API_KEY", "posthog-key");

        let config = super::configured_posthog_config().expect("config should be Some");
        assert_eq!(config.api_key, "docspec-key");

        std::env::remove_var("DOCSPEC_POSTHOG_API_KEY");
        std::env::remove_var("POSTHOG_API_KEY");
    }

    #[test]
    fn configured_posthog_config_falls_back_to_posthog_api_key() {
        let _env_guard = lock_env();
        std::env::remove_var("DOCSPEC_POSTHOG_API_KEY");
        std::env::set_var("POSTHOG_API_KEY", "fallback-key");

        let config = super::configured_posthog_config().expect("config should be Some");
        assert_eq!(config.api_key, "fallback-key");

        std::env::remove_var("POSTHOG_API_KEY");
    }

    #[test]
    fn configured_posthog_config_returns_none_when_no_key() {
        let _env_guard = lock_env();
        std::env::remove_var("DOCSPEC_POSTHOG_API_KEY");
        std::env::remove_var("POSTHOG_API_KEY");

        assert!(super::configured_posthog_config().is_none());
    }

    #[test]
    fn configured_posthog_config_treats_empty_api_key_as_absent() {
        let _env_guard = lock_env();
        std::env::set_var("DOCSPEC_POSTHOG_API_KEY", "");
        std::env::remove_var("POSTHOG_API_KEY");

        assert!(super::configured_posthog_config().is_none());

        std::env::remove_var("DOCSPEC_POSTHOG_API_KEY");
    }

    #[test]
    fn configured_posthog_config_uses_default_host_when_absent() {
        let _env_guard = lock_env();
        std::env::set_var("DOCSPEC_POSTHOG_API_KEY", "test-key");
        std::env::remove_var("DOCSPEC_POSTHOG_HOST");
        std::env::remove_var("POSTHOG_HOST");

        let config = super::configured_posthog_config().expect("config should be Some");
        assert_eq!(config.host, "https://us.i.posthog.com");

        std::env::remove_var("DOCSPEC_POSTHOG_API_KEY");
    }

    #[test]
    fn configured_posthog_config_prefers_docspec_host() {
        let _env_guard = lock_env();
        std::env::set_var("DOCSPEC_POSTHOG_API_KEY", "test-key");
        std::env::set_var("DOCSPEC_POSTHOG_HOST", "https://docspec.i.posthog.com");
        std::env::set_var("POSTHOG_HOST", "https://fallback.i.posthog.com");

        let config = super::configured_posthog_config().expect("config should be Some");
        assert_eq!(config.host, "https://docspec.i.posthog.com");

        std::env::remove_var("DOCSPEC_POSTHOG_API_KEY");
        std::env::remove_var("DOCSPEC_POSTHOG_HOST");
        std::env::remove_var("POSTHOG_HOST");
    }

    #[test]
    fn env_sample_rate_defaults_to_one() {
        let _env_guard = lock_env();
        std::env::remove_var("POSTHOG_SAMPLE_RATE");
        assert_eq!(super::env_sample_rate().to_bits(), f32::to_bits(1.0));
    }

    #[test]
    fn env_sample_rate_clamps_high() {
        let _env_guard = lock_env();
        std::env::set_var("POSTHOG_SAMPLE_RATE", "5.0");
        assert_eq!(super::env_sample_rate().to_bits(), f32::to_bits(1.0));
        std::env::remove_var("POSTHOG_SAMPLE_RATE");
    }

    #[test]
    fn env_sample_rate_clamps_low() {
        let _env_guard = lock_env();
        std::env::set_var("POSTHOG_SAMPLE_RATE", "-0.5");
        assert_eq!(super::env_sample_rate().to_bits(), f32::to_bits(0.0));
        std::env::remove_var("POSTHOG_SAMPLE_RATE");
    }

    #[test]
    fn before_send_strips_known_body_keys() {
        let mut event = posthog_rs::Event::new("test", "user-123");
        event
            .insert_prop("request_body", "secret document")
            .expect("insert should succeed");
        event
            .insert_prop("response_body", "secret output")
            .expect("insert should succeed");
        event
            .insert_prop("result", "success")
            .expect("insert should succeed");

        let mut result = super::before_send(event).expect("hook should keep event");

        assert!(
            result.remove_prop("request_body").is_none(),
            "request_body should have been stripped"
        );
        assert!(
            result.remove_prop("response_body").is_none(),
            "response_body should have been stripped"
        );
        assert!(
            result.remove_prop("result").is_some(),
            "non-body properties should be preserved"
        );
    }

    #[test]
    fn before_send_strips_body_keys_case_insensitive() {
        let mut event = posthog_rs::Event::new("test", "user-123");
        event
            .insert_prop("RequestBody", "secret")
            .expect("insert should succeed");
        event
            .insert_prop("BODY", "secret")
            .expect("insert should succeed");
        event
            .insert_prop("response_body", "secret")
            .expect("insert should succeed");
        event
            .insert_prop("MyBodyField", "secret")
            .expect("insert should succeed");
        event
            .insert_prop("status", "ok")
            .expect("insert should succeed");

        let mut result = super::before_send(event).expect("hook should keep event");

        assert!(
            result.remove_prop("RequestBody").is_none(),
            "RequestBody (mixed case) should have been stripped"
        );
        assert!(
            result.remove_prop("BODY").is_none(),
            "BODY (uppercase) should have been stripped"
        );
        assert!(
            result.remove_prop("response_body").is_none(),
            "response_body (lowercase) should have been stripped"
        );
        assert!(
            result.remove_prop("MyBodyField").is_none(),
            "MyBodyField (contains body) should have been stripped"
        );
        assert!(
            result.remove_prop("status").is_some(),
            "status (no body substring) should be preserved"
        );
    }

    #[test]
    fn release_constant_contains_package_version() {
        assert!(
            super::RELEASE.starts_with("docspec-http@"),
            "RELEASE should start with docspec-http@"
        );
        assert!(
            super::RELEASE.len() > "docspec-http@".len(),
            "RELEASE should have a version after the @ sign"
        );
    }
}
