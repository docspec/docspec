//! Memory slope test for BUG-3: file input must not scale heap with file size.
//!
//! Uses `peak_alloc` (heap-only allocator wrapper) so that mmap'd file pages — which
//! bypass the global allocator — are correctly excluded. The slope test asserts that
//! switching from a 1 MB to a 100 MB input does NOT proportionally grow heap usage.
//!
//! `peak_alloc` is process-global state; use `#[serial]` to prevent test pollution.

// Test files opt out of expect/unwrap denial and stderr printing.
// Integration test files in tests/ are inherently test-only; the lint is overly strict here.
#![allow(clippy::expect_used)]
#![allow(clippy::print_stderr)]
#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::integer_division)]

use std::io::Write as _;

use peak_alloc::PeakAlloc;
use serial_test::serial;

/// Global allocator for heap peak tracking in this test binary.
///
/// Must be at file scope (not inside a test function) to take effect.
#[global_allocator]
static PEAK_ALLOC: PeakAlloc = PeakAlloc;

/// Heap slope test: 100× bigger file must not produce 100× bigger heap peak.
///
/// Measures ONLY the input-loading step (`load_input`), not the full pipeline.
/// The `memmap2` path does not allocate Rust heap proportional to file size —
/// the mmap'd pages are managed by the kernel and bypass the global allocator.
/// The original `.read_to_string()` would allocate a `String` matching file size.
#[test]
#[serial]
fn heap_delta_stays_below_1mb_across_100x_input_scale() {
    fn measure_kb(path: &std::path::Path) -> usize {
        // Reset the peak counter immediately before the measurement to clear
        // any baseline pollution from prior allocations.
        PEAK_ALLOC.reset_peak_usage();

        let _loaded = docspec_cli::load_input(Some(path)).expect("load_input failed");
        // Intentionally NOT calling run_pipeline — BUG-3's fix is specifically
        // about the input-loading path (mmap vs read_to_string). The pulldown-cmark
        // parser allocates O(input_size) heap regardless of the input source,
        // so including the pipeline would mask the actual fix measurement.

        PEAK_ALLOC.peak_usage() / 1024
    }

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = manifest_dir.join("../../tests/fixtures/streaming");
    let evidence_dir = manifest_dir.join("../../.sisyphus/evidence");

    let path_1mb = fixture_dir.join("1mb.md");
    let path_100mb = fixture_dir.join("100mb.md");

    if !path_1mb.exists() || !path_100mb.exists() {
        let status = std::process::Command::new("bash")
            .arg(fixture_dir.join("gen.sh"))
            .status()
            .expect("gen.sh failed to start");
        assert!(status.success(), "gen.sh exited with error");
    }

    let peak_1mb = measure_kb(&path_1mb);
    let peak_100mb = measure_kb(&path_100mb);
    let delta = peak_100mb.saturating_sub(peak_1mb);

    eprintln!("peak_1mb={peak_1mb} KB, peak_100mb={peak_100mb} KB, delta={delta} KB");

    if let Ok(mut f) = std::fs::File::create(evidence_dir.join("task-10-memory.txt")) {
        writeln!(
            f,
            "peak_1mb={peak_1mb} KB, peak_100mb={peak_100mb} KB, delta={delta} KB"
        )
        .unwrap_or(());
    }

    assert!(peak_1mb > 0, "peak_1mb should be > 0 (test ran something)");
    assert!(
        peak_1mb < 50_000,
        "peak_1mb={peak_1mb} KB should be < 50 MB for a 1 MB input"
    );

    assert!(
        delta < 1024,
        "BUG-3 REGRESSION: heap delta {delta} KB >= 1024 KB (1 MB). \
         The mmap path leaks proportional to file size. Check load_input."
    );
}
