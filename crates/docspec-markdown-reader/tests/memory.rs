//! Memory-overhead invariant tests for HTML translation.
//!
//! These tests verify that HTML translation adds bounded overhead on top of
//! pulldown-cmark's baseline. They are marked `#[ignore]` because they are
//! slow and should not run in normal CI.
//!
//! Run with: `cargo test --test memory -- --ignored --nocapture`.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::integer_division,
    clippy::print_stdout,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

use docspec_markdown_reader::{EventSource as _, MarkdownReader};

/// Read a named field from `/proc/self/status` in kB (Linux only).
fn proc_status_kb(field: &str) -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in status.lines() {
        if line.starts_with(field) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return parts[1].parse().unwrap_or(0);
            }
        }
    }
    0
}

/// Verify that processing 1 MB of `<b>x</b>` patterns stays under 200 MB peak RSS overhead.
///
/// The 200 MB budget measures the **overhead** introduced by reading and translating
/// the input — not the absolute process RSS (which includes the debug test binary
/// baseline). We capture `VmRSS` before allocation as a baseline and compare against
/// `VmHWM` (peak RSS since process start) after processing.
#[test]
#[ignore = "slow memory test, run manually"]
fn html_translation_bounded_overhead() {
    let baseline_kb = proc_status_kb("VmRSS:");

    // Generate ~1 MB input
    let n = 1_000_000 / "<b>x</b>".len();
    let input = "<b>x</b>".repeat(n);

    let mut reader = MarkdownReader::from_str(&input);
    while reader.next_event().unwrap().is_some() {}

    let peak_kb = proc_status_kb("VmHWM:");
    let overhead_mb = peak_kb.saturating_sub(baseline_kb) / 1024;
    println!("Peak RSS overhead: {overhead_mb} MB");

    assert!(
        overhead_mb < 200,
        "Peak RSS overhead {overhead_mb} MB exceeds 200 MB budget"
    );
    println!("memory budget OK: {overhead_mb} MB < 200 MB");
}

/// Verify that deeply nested `<b>` tags (beyond `MAX_HTML_STYLE_DEPTH`) complete without OOM.
#[test]
#[ignore = "slow memory test, run manually"]
fn html_deeply_nested_no_oom() {
    let mut input = "<b>".repeat(10_000);
    input.push_str(&"</b>".repeat(10_000));

    let mut reader = MarkdownReader::from_str(&input);
    let mut count: usize = 0;
    while reader.next_event().unwrap().is_some() {
        count += 1;
    }

    // Stack depth is bounded at 32, so at most 32 StartTextStyle + 32 EndTextStyle
    // plus document/paragraph wrapper events
    println!("Total events: {count}");
    assert!(count < 200, "Unexpected event count: {count}");
}
