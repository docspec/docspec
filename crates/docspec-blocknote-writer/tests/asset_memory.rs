//! Memory slope test for BUG-1: asset encoding must not buffer proportionally to asset size.
//!
//! Uses `peak_alloc` (heap-only allocator wrapper) to measure heap allocations.
//! The `base64::write::EncoderWriter` has a fixed 4 KB stack buffer — no heap growth.
//! The original bug allocated a `Vec<u8>` proportional to asset size (~134 MB for 100 MB).
//!
//! `peak_alloc` is process-global state; use `#[serial]` to prevent test pollution.

#![allow(clippy::expect_used)]
#![allow(clippy::print_stderr)]
#![allow(clippy::tests_outside_test_module)]
#![allow(clippy::integer_division)]
#![allow(clippy::std_instead_of_alloc)]

use std::borrow::Cow;
use std::fs::File;
use std::io::{self, BufReader, Write};
use std::path::PathBuf;

use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::{AssetProvider, Event, EventSink as _, ImageSource};
use peak_alloc::PeakAlloc;
use serial_test::serial;

#[global_allocator]
static PEAK_ALLOC: PeakAlloc = PeakAlloc;

/// File-backed asset provider that streams from disk in 64 KB chunks.
/// Asset bytes come from the OS file system, never from a Rust-heap Vec.
struct FileAssetProvider {
    content_type: String,
    path: PathBuf,
}

impl AssetProvider for FileAssetProvider {
    fn content_type(&self, _asset_id: &str) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(&self.content_type))
    }

    fn stream_to(&self, _asset_id: &str, writer: &mut dyn Write) -> Option<io::Result<u64>> {
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) => return Some(Err(e)),
        };
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        Some(io::copy(&mut reader, writer))
    }
}

fn ensure_fixtures(fixture_dir: &std::path::Path) {
    let path_1mb = fixture_dir.join("1mb.bin");
    let path_100mb = fixture_dir.join("100mb.bin");
    if !path_1mb.exists() || !path_100mb.exists() {
        let status = std::process::Command::new("bash")
            .arg(fixture_dir.join("gen.sh"))
            .status()
            .expect("gen.sh failed to start");
        assert!(status.success(), "gen.sh exited with error");
    }
}

fn measure_kb(fixture_path: &std::path::Path) -> usize {
    let provider = FileAssetProvider {
        path: fixture_path.to_path_buf(),
        content_type: "application/octet-stream".to_string(),
    };

    PEAK_ALLOC.reset_peak_usage();

    let mut output = io::sink();
    let mut writer = BlockNoteWriter::with_assets(&mut output, &provider);

    writer
        .handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        })
        .expect("StartDocument");
    writer
        .handle_event(Event::Image {
            source: ImageSource::Asset {
                asset_id: "test-asset".to_string(),
            },
            alt: None,
            decorative: false,
            id: None,
            title: None,
        })
        .expect("Image");
    writer
        .handle_event(Event::EndDocument)
        .expect("EndDocument");
    writer.finish().expect("finish");

    PEAK_ALLOC.peak_usage() / 1024
}

#[test]
#[serial]
fn asset_memory_slope_within_64kb_across_100x_scale() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = manifest_dir.join("../../tests/fixtures/streaming");
    let evidence_dir = manifest_dir.join("../../.sisyphus/evidence");

    ensure_fixtures(&fixture_dir);

    let peak_1mb = measure_kb(&fixture_dir.join("1mb.bin"));
    let peak_100mb = measure_kb(&fixture_dir.join("100mb.bin"));
    let delta = peak_100mb.saturating_sub(peak_1mb);

    eprintln!("peak_1mb={peak_1mb} KB, peak_100mb={peak_100mb} KB, delta={delta} KB");

    if let Ok(mut f) = std::fs::File::create(evidence_dir.join("task-13-memory.txt")) {
        writeln!(
            f,
            "peak_1mb={peak_1mb} KB, peak_100mb={peak_100mb} KB, delta={delta} KB"
        )
        .unwrap_or(());
    }

    assert!(peak_1mb > 0, "peak_1mb should be > 0 (test ran something)");
    assert!(
        peak_1mb < 50_000,
        "peak_1mb={peak_1mb} KB should be < 50 MB for a 1 MB asset"
    );
    assert!(
        delta < 64,
        "BUG-1 REGRESSION: heap delta {delta} KB >= 64 KB. \
         The asset path is buffering proportional to asset size. \
         Check write_asset_as_data_uri_keyed."
    );
}

#[test]
#[serial]
fn asset_output_is_base64_ascii_only() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_dir = manifest_dir.join("../../tests/fixtures/streaming");
    ensure_fixtures(&fixture_dir);

    let provider = FileAssetProvider {
        path: fixture_dir.join("1mb.bin"),
        content_type: "application/octet-stream".to_string(),
    };

    let mut output = Vec::new();
    let mut writer = BlockNoteWriter::with_assets(&mut output, &provider);

    writer
        .handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        })
        .expect("StartDocument");
    writer
        .handle_event(Event::Image {
            source: ImageSource::Asset {
                asset_id: "test-asset".to_string(),
            },
            alt: None,
            decorative: false,
            id: None,
            title: None,
        })
        .expect("Image");
    writer
        .handle_event(Event::EndDocument)
        .expect("EndDocument");
    writer.finish().expect("finish");

    assert!(
        output.iter().all(u8::is_ascii),
        "base64 output contains non-ASCII bytes"
    );
    let json = String::from_utf8(output).expect("output must be valid UTF-8");
    assert!(
        json.contains("data:application/octet-stream;base64,"),
        "output must contain data URI prefix"
    );
}
