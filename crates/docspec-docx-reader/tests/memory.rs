//! Memory-overhead invariant tests for DOCX streaming.
//!
//! These tests verify that DocxReader::from_path adds bounded overhead regardless
//! of document.xml size. They are marked `#[ignore]` because they are slow and
//! should not run in normal CI.
//!
//! Run with: `cargo test -p docspec-docx-reader --test memory -- --ignored --nocapture`.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::integer_division,
    clippy::print_stderr,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

#[cfg(target_os = "linux")]
mod linux_only {

    fn synth_large_docx(doc_xml_size_mb: usize) -> Vec<u8> {
        let target_bytes = doc_xml_size_mb * 1024 * 1024;

        let header = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>"#;

        let para = "<w:p><w:r><w:t>test paragraph content for memory measurement</w:t></w:r></w:p>";
        let footer = "</w:body></w:document>";

        let para_bytes = para.len();
        let overhead = header.len() + footer.len();
        let n_paras = (target_bytes.saturating_sub(overhead)) / para_bytes + 1;

        let mut doc_xml = String::with_capacity(target_bytes + 1024);
        doc_xml.push_str(header);
        for _ in 0..n_paras {
            doc_xml.push_str(para);
        }
        doc_xml.push_str(footer);

        let rels_xml = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

        docspec_test_utils::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Stored,
                rels_xml.as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Stored,
                doc_xml.as_bytes(),
            ),
        ])
    }

    fn child_bin_path() -> std::path::PathBuf {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR must be set (run via cargo test)");
        let workspace_root = std::path::Path::new(&manifest_dir).join("../..");
        workspace_root.join("target/release/examples/memtest_child")
    }

    fn run_child_and_get_peak_rss(docx_path: &std::path::Path) -> u64 {
        let bin = child_bin_path();
        assert!(
            bin.exists(),
            "memtest_child binary not found at {bin:?}. Run: cargo build --release --example memtest_child -p docspec-docx-reader"
        );

        let output = std::process::Command::new(&bin)
            .arg(docx_path)
            .output()
            .expect("failed to spawn memtest_child");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "memtest_child failed:\n{stderr}");

        for line in stderr.lines() {
            if let Some(rest) = line.strip_prefix("PEAK_RSS_KB=") {
                if let Ok(kb) = rest.trim().parse::<u64>() {
                    return kb;
                }
            }
        }
        panic!("PEAK_RSS_KB not found in child stderr:\n{stderr}");
    }

    /// Verify that streaming via from_path adds bounded overhead (< 80 MB peak RSS)
    /// even when processing a 50 MB word/document.xml.
    ///
    /// Proof: if buffering were still in effect, the child would need 50 MB just for
    /// the document.xml buffer, pushing it over the 80 MB threshold.
    ///
    /// Prerequisite: `cargo build --release --example memtest_child -p docspec-docx-reader`
    #[test]
    #[ignore = "slow memory test, run manually after building memtest_child"]
    fn streaming_doc_xml_bounded_overhead() {
        let bytes = synth_large_docx(50);
        let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
        use std::io::Write;
        tmp.write_all(&bytes).expect("write docx");
        tmp.flush().expect("flush");
        drop(bytes);

        let peak_kb = run_child_and_get_peak_rss(tmp.path());
        let peak_mb = peak_kb / 1024;
        eprintln!("50 MB doc.xml: child peak RSS = {peak_mb} MB ({peak_kb} kB)");

        assert!(
            peak_kb < 80_000,
            "Peak RSS {peak_mb} MB exceeds 80 MB budget for 50 MB document.xml (streaming leak!)"
        );
        eprintln!("Memory budget OK: {peak_mb} MB < 80 MB");
    }

    /// Verify that memory usage stays flat as document.xml grows from 50 MB to 200 MB.
    /// Both tests hitting the same threshold (~10-20 MB) proves O(1) behavior.
    ///
    /// Prerequisite: `cargo build --release --example memtest_child -p docspec-docx-reader`
    #[test]
    #[ignore = "slow memory test, run manually after building memtest_child"]
    fn streaming_doc_xml_scales_to_huge_docs() {
        let bytes = synth_large_docx(200);
        let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
        use std::io::Write;
        tmp.write_all(&bytes).expect("write docx");
        tmp.flush().expect("flush");
        drop(bytes);

        let peak_kb = run_child_and_get_peak_rss(tmp.path());
        let peak_mb = peak_kb / 1024;
        eprintln!("200 MB doc.xml: child peak RSS = {peak_mb} MB ({peak_kb} kB)");

        assert!(
            peak_kb < 80_000,
            "Peak RSS {peak_mb} MB exceeds 80 MB budget for 200 MB document.xml (memory scales with input = not O(1)!)"
        );
        eprintln!(
            "Memory budget OK: {peak_mb} MB < 80 MB — memory is O(1) regardless of document size"
        );
    }
} // mod linux_only
