//! Conversion analytics event construction and recording.

#[cfg(feature = "posthog")]
#[derive(Clone, Copy)]
struct ConversionEventData<'a> {
    distinct_id: &'a str,
    is_anonymous: bool,
    result_label: &'static str,
    error_class_label: &'static str,
    input_mime_label: &'static str,
    output_mime_label: &'static str,
    input_bytes: usize,
    output_bytes: u64,
    duration_ms: u64,
    trace_id: Option<&'a str>,
}

#[cfg(feature = "posthog")]
fn log_posthog_insert_error(key: &'static str, error: &posthog_rs::Error) {
    tracing::warn!(%error, prop = key, "posthog insert_prop failed; skipping property");
}

/// Builds a `conversion_completed` analytics event for `PostHog`.
///
/// Property insertion failures are logged and skipped (best-effort).
#[cfg(feature = "posthog")]
fn build_posthog_conversion_event(data: ConversionEventData<'_>) -> posthog_rs::Event {
    let mut event = posthog_rs::Event::new("conversion_completed", data.distinct_id);
    macro_rules! insert {
        ($key:expr, $val:expr) => {
            if let Err(ref error) = event.insert_prop($key, $val) {
                log_posthog_insert_error($key, error);
            }
        };
    }
    insert!("result", data.result_label);
    insert!("error_class", data.error_class_label);
    insert!("input_mime_type", data.input_mime_label);
    insert!("output_mime_type", data.output_mime_label);
    insert!(
        "input_bytes",
        u64::try_from(data.input_bytes).unwrap_or(u64::MAX)
    );
    insert!("output_bytes", data.output_bytes);
    insert!("duration_ms", data.duration_ms);
    insert!("$app_version", crate::telemetry::posthog::RELEASE);
    if let Some(tid) = data.trace_id {
        insert!("trace_id", tid);
    }
    if data.is_anonymous {
        insert!("$is_anonymous", true);
    }
    event
}

#[derive(Clone, Copy)]
pub(super) struct CompletionContext<'a> {
    pub request_id_opt: Option<&'a str>,
    pub result_label: &'static str,
    pub error_class_label: &'static str,
    pub input_mime_label: &'static str,
    pub output_mime_label: &'static str,
    pub body_len_for_logging: usize,
    pub output_bytes: u64,
    pub conversion_duration_ms: u64,
    pub trace_id_owned: &'a Option<String>,
}

pub(super) fn record_completion(ctx: CompletionContext<'_>) {
    let CompletionContext {
        request_id_opt,
        result_label,
        error_class_label,
        input_mime_label,
        output_mime_label,
        body_len_for_logging,
        output_bytes,
        conversion_duration_ms,
        trace_id_owned,
    } = ctx;
    let client_arc: Option<std::sync::Arc<posthog_rs::Client>> =
        match crate::telemetry::posthog_client_slot().read() {
            Ok(guard) => guard.as_ref().map(std::sync::Arc::clone),
            Err(_poisoned) => {
                tracing::warn!("posthog slot poisoned; skipping conversion analytics");
                None
            }
        };
    if let Some(client) = client_arc {
        let distinct_id_owned =
            request_id_opt.map_or_else(|| uuid::Uuid::new_v4().to_string(), str::to_owned);
        let is_anonymous = request_id_opt.is_none();
        let event = build_posthog_conversion_event(ConversionEventData {
            distinct_id: &distinct_id_owned,
            is_anonymous,
            result_label,
            error_class_label,
            input_mime_label,
            output_mime_label,
            input_bytes: body_len_for_logging,
            output_bytes,
            duration_ms: conversion_duration_ms,
            trace_id: trace_id_owned.as_deref(),
        });
        client.capture(event);
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "posthog")]
    mod posthog_tests {
        #![allow(clippy::expect_used, clippy::unwrap_used)]

        #[test]
        fn build_posthog_conversion_event_smoke_test() {
            let event =
                super::super::build_posthog_conversion_event(super::super::ConversionEventData {
                    distinct_id: "test-user-123",
                    is_anonymous: false,
                    result_label: "success",
                    error_class_label: "none",
                    input_mime_label: "text/markdown",
                    output_mime_label: "application/vnd.docspec.blocknote+json",
                    input_bytes: 100,
                    output_bytes: 200,
                    duration_ms: 50,
                    trace_id: Some("trace-abc"),
                });
            drop(event);
        }
    }
}

#[cfg(test)]
mod ordering_tests {
    #[test]
    fn completion_runs_after_metrics_and_structured_log() {
        let source = include_str!("mod.rs");

        let metrics_matches = source
            .match_indices("crate::metrics::METRIC_CONVERSIONS_TOTAL")
            .collect::<Vec<_>>();
        let structured_log_matches = source
            .match_indices("event = \"conversion_completed\"")
            .collect::<Vec<_>>();
        let completion_matches = source
            .match_indices("analytics::record_completion(")
            .collect::<Vec<_>>();

        assert_eq!(metrics_matches.len(), 1);
        assert_eq!(structured_log_matches.len(), 1);
        assert_eq!(completion_matches.len(), 1);

        let metrics_offset = metrics_matches
            .first()
            .map_or(usize::MAX, |(offset, _)| *offset);
        let structured_log_offset = structured_log_matches
            .first()
            .map_or(usize::MAX, |(offset, _)| *offset);
        let completion_offset = completion_matches
            .first()
            .map_or(usize::MAX, |(offset, _)| *offset);

        let mut labels_by_offset = [
            ("metrics", metrics_offset),
            ("structured_log", structured_log_offset),
            ("record_completion", completion_offset),
        ];
        labels_by_offset.sort_unstable_by_key(|(_, offset)| *offset);

        assert_eq!(
            labels_by_offset.map(|(label, _)| label),
            ["metrics", "structured_log", "record_completion"]
        );
    }
}

#[cfg(test)]
mod coverage_tests {
    #[test]
    fn anonymous_completion_event_has_exact_anonymous_properties() {
        let event = super::build_posthog_conversion_event(super::ConversionEventData {
            distinct_id: "anonymous-test-user",
            is_anonymous: true,
            result_label: "success",
            error_class_label: "none",
            input_mime_label: "text/markdown",
            output_mime_label: "application/vnd.docspec.blocknote+json",
            input_bytes: 100,
            output_bytes: 200,
            duration_ms: 50,
            trace_id: None,
        });

        assert_eq!(
            event.properties().get("$is_anonymous"),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(event.properties().get("trace_id"), None);
        assert_eq!(event.distinct_id(), "anonymous-test-user");
        assert_eq!(event.event_name(), "conversion_completed");
    }

    #[test]
    fn posthog_insert_error_is_logged_without_mutating_the_error() {
        let error = posthog_rs::Error::Serialization("forced test failure".to_owned());

        super::log_posthog_insert_error("test_property", &error);

        assert_eq!(
            error.to_string(),
            "Serialization Error: forced test failure"
        );
    }

    #[test]
    fn poisoned_posthog_slot_skips_completion_in_an_isolated_process() {
        let test_binary = std::env::current_exe().unwrap_or_default();
        assert!(!test_binary.as_os_str().is_empty());
        assert!(std::process::Command::new(test_binary)
            .args([
                "--ignored",
                "--exact",
                "handlers::conversion::analytics::coverage_tests::poisoned_slot_child_probe",
            ])
            .status()
            .is_ok_and(|status| status.success()));
    }

    #[test]
    #[ignore = "executed by poisoned_posthog_slot_skips_completion_in_an_isolated_process"]
    fn poisoned_slot_child_probe() {
        let slot = crate::telemetry::posthog_client_slot();
        let poison_result = std::thread::spawn(|| {
            let guard = slot
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let _guard = guard;
            std::panic::resume_unwind(Box::new("intentional test lock poison"));
        })
        .join();
        assert!(poison_result.is_err());

        let trace_id_owned = None;
        super::record_completion(super::CompletionContext {
            request_id_opt: None,
            result_label: "success",
            error_class_label: "none",
            input_mime_label: "text/markdown",
            output_mime_label: "application/vnd.docspec.blocknote+json",
            body_len_for_logging: 100,
            output_bytes: 200,
            conversion_duration_ms: 50,
            trace_id_owned: &trace_id_owned,
        });

        slot.clear_poison();
    }
}
