//! Snapshot regression tests for the pandoc DOCX corpus.
//!
//! Runs every `.docx` file in `tests/fixtures/docx/pandoc/` through
//! `DocxReader` and compares the event stream against committed snapshots.
//!
//! See TESTING.md § Snapshot Review for first-run and CI-mode workflow.
#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::panic,
    clippy::redundant_test_prefix,
    clippy::std_instead_of_core,
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    dead_code
)]

use std::io::Cursor;
use std::path::PathBuf;

use docspec_docx_reader::DocxReader;
use docspec_test_utils::capture;

fn fixture_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/docx/pandoc")
        .join(file_name)
}

fn assert_fixture_snapshot(file_name: &str) {
    let path = fixture_path(file_name);
    let snapshot = capture(&path, |bytes| DocxReader::from_reader(Cursor::new(bytes)));
    insta::with_settings!({
        snapshot_path => "../../../tests/snapshots/docx/pandoc",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_debug_snapshot!(file_name, snapshot);
    });
}

macro_rules! pandoc_fixture_test {
    ($test_name:ident, $file_name:literal) => {
        #[test]
        fn $test_name() {
            assert_fixture_snapshot($file_name);
        }
    };
}

include!(concat!(env!("OUT_DIR"), "/pandoc_corpus_tests.rs"));
