//! Memory-overhead invariant test for the `AnyReader` facade.
//!
//! `DocxReader` has both a streaming (`from_path`) and a buffering
//! (`from_reader`) constructor. This test pins the facade to the streaming one:
//! it fails if `AnyReader::from_path` ever routes DOCX through `from_reader`
//! again, which would silently make memory O(document size).
//!
//! Marked `#[ignore]` because it is slow; mirrors
//! `docspec-docx-reader/tests/memory.rs`.
//!
//! The test spawns a prebuilt release example, so build it first:
//!
//! ```text
//! cargo build --release --example memtest_facade_child -p docspec --features docx
//! cargo test -p docspec --test memory --all-features -- --ignored --nocapture
//! ```
#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::integer_division,
    clippy::print_stderr,
    clippy::tests_outside_test_module
)]

#[cfg(all(target_os = "linux", feature = "docx"))]
mod linux_only {
    use std::io::Write as _;

    fn synth_large_docx(doc_xml_size_mb: usize) -> Vec<u8> {
        let target_bytes = doc_xml_size_mb * 1024 * 1024;

        let header = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>"#;
        let para = "<w:p><w:r><w:t>test paragraph content for memory measurement</w:t></w:r></w:p>";
        let footer = "</w:body></w:document>";

        let overhead = header.len() + footer.len();
        let n_paras = (target_bytes.saturating_sub(overhead)) / para.len() + 1;

        let mut doc_xml = String::with_capacity(target_bytes + 1024);
        doc_xml.push_str(header);
        for _ in 0..n_paras {
            doc_xml.push_str(para);
        }
        doc_xml.push_str(footer);

        let rels_xml = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

        docspec_test_utils::synth_docx(rels_xml, &doc_xml)
    }

    /// Cargo's target directory, always absolute.
    ///
    /// `CARGO_TARGET_TMPDIR` is set at compile time for integration tests to
    /// `<target-dir>/tmp`, already resolved by Cargo. Reading `CARGO_TARGET_DIR`
    /// at runtime instead would break on relative values: Cargo resolves those
    /// against its invocation directory, while the test process runs from the
    /// package root.
    fn target_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .parent()
            .expect("CARGO_TARGET_TMPDIR is <target-dir>/tmp and always has a parent")
            .to_path_buf()
    }

    fn child_bin_path() -> std::path::PathBuf {
        target_dir().join("release/examples/memtest_facade_child")
    }

    fn run_child_and_get_peak_rss(docx_path: &std::path::Path) -> u64 {
        let bin = child_bin_path();

        assert!(
            bin.exists(),
            "memtest_facade_child not found at {}. Run: cargo build --release --example memtest_facade_child -p docspec --features docx",
            bin.display()
        );

        let output = std::process::Command::new(&bin)
            .arg(docx_path)
            .output()
            .expect("failed to spawn memtest_facade_child");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "memtest_facade_child failed:\n{stderr}"
        );

        stderr
            .lines()
            .find_map(|line| {
                line.strip_prefix("PEAK_RSS_KB=")?
                    .trim()
                    .parse::<u64>()
                    .ok()
            })
            .expect("PEAK_RSS_KB line must be present in child stderr")
    }

    /// A 50 MB `document.xml` must not cost 50 MB of resident memory.
    ///
    /// The 20 MB budget is calibrated against both measured behaviours: streaming
    /// via `from_path` peaks at ~3 MB, while delegating to `from_reader` buffers
    /// the whole main part and peaks at ~55 MB. A looser budget (say 80 MB) would
    /// admit the buffering regression this test exists to catch.
    #[test]
    #[ignore = "slow memory test, run manually after building memtest_facade_child"]
    fn facade_from_path_streams_docx() {
        let bytes = synth_large_docx(50);
        let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
        tmp.write_all(&bytes).expect("write docx");
        tmp.flush().expect("flush");
        drop(bytes);

        let rss_kib = run_child_and_get_peak_rss(tmp.path());
        let peak_mebibytes = rss_kib / 1024;
        eprintln!("facade, 50 MB doc.xml: peak RSS = {peak_mebibytes} MB ({rss_kib} kB)");

        assert!(
            rss_kib < 20_000,
            "Peak RSS {peak_mebibytes} MB exceeds 20 MB budget — AnyReader::from_path is buffering DOCX instead of streaming"
        );
    }
}
