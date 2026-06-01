//! Smoke tests for the `with_test_recorder` helper.
#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

#[test]
fn with_test_recorder_returns_usable_handle() {
    let (handle, ()) = common::with_test_recorder(|| {});
    let _body = handle.render();
}

#[test]
fn with_test_recorder_counter_appears_in_render() {
    let (handle, ()) = common::with_test_recorder(|| {
        metrics::counter!("docspec_test_counter_total").increment(1);
    });
    let body = handle.render();
    let lines: Vec<&str> = body.lines().collect();
    // Assert exact line equality — no substring matching
    let type_line = "# TYPE docspec_test_counter_total counter".to_string();
    assert!(
        lines.iter().any(|l| *l == type_line),
        "Expected exact line '{type_line}' in rendered output:\n{body}"
    );
}
