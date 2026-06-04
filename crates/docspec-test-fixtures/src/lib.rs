#![allow(clippy::unwrap_used, clippy::expect_used)]
// Reason: docspec-test-fixtures is a workspace-internal test-only crate (publish = false).
// The lifted helpers use expect() on infallible-in-practice zip writes; surfacing Result
// would create awkward .unwrap() chains in every test consumer.

//! In-memory document fixtures for `DocSpec` test suites.
//!
//! Provides helpers to synthesize minimal document archives (DOCX, ODT, etc.) in memory
//! from raw XML strings without touching the filesystem. "Synth" = synthesize.

use std::io::{Cursor, Write as _};
pub use zip::CompressionMethod;
use zip::{write::SimpleFileOptions, ZipWriter};

/// Builds a minimal 2-entry DOCX archive (Deflated) from raw XML strings.
///
/// Entries:
/// - `_rels/.rels` — the relationship file
/// - `word/document.xml` — the main document
///
/// # Panics
///
/// Panics if ZIP write operations fail (infallible in practice for in-memory buffers).
#[must_use]
#[inline]
pub fn synth_docx(rels_xml: &str, document_xml: &str) -> Vec<u8> {
    synth_docx_with_entries(&[
        (
            "_rels/.rels",
            CompressionMethod::Deflated,
            rels_xml.as_bytes(),
        ),
        (
            "word/document.xml",
            CompressionMethod::Deflated,
            document_xml.as_bytes(),
        ),
    ])
}

/// Builds a DOCX archive with arbitrary entries.
///
/// Each entry is a tuple of `(name, compression_method, bytes)`.
///
/// # Panics
///
/// Panics if ZIP write operations fail (infallible in practice for in-memory buffers).
#[must_use]
#[inline]
pub fn synth_docx_with_entries(entries: &[(&str, CompressionMethod, &[u8])]) -> Vec<u8> {
    let buf = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(buf);
    for (name, method, data) in entries {
        let options = SimpleFileOptions::default().compression_method(*method);
        writer
            .start_file(*name, options)
            .expect("start_file failed");
        writer.write_all(data).expect("write_all failed");
    }
    writer.finish().expect("finish failed").into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synth_docx_produces_valid_zip() {
        let rels_xml = r#"<?xml version="1.0"?><Relationships/>"#;
        let document_xml = r#"<?xml version="1.0"?><w:document/>"#;
        let bytes = synth_docx(rels_xml, document_xml);

        // Verify it's a valid ZIP by checking the magic number
        assert!(bytes.len() > 4);
        assert_eq!(
            bytes.get(0..2),
            Some(b"PK".as_slice()),
            "ZIP magic number should be PK"
        );

        // Verify we can read it back
        let cursor = Cursor::new(bytes);
        let zip = zip::ZipArchive::new(cursor).expect("should be valid ZIP");
        assert_eq!(zip.len(), 2, "should have exactly 2 entries");
    }

    #[test]
    fn synth_docx_with_entries_preserves_order() {
        let entries: &[(&str, CompressionMethod, &[u8])] = &[
            ("first.txt", CompressionMethod::Stored, b"first"),
            ("second.txt", CompressionMethod::Stored, b"second"),
            ("third.txt", CompressionMethod::Stored, b"third"),
        ];
        let bytes = synth_docx_with_entries(entries);

        let cursor = Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(cursor).expect("should be valid ZIP");
        assert_eq!(zip.len(), 3, "should have exactly 3 entries");

        // Verify names are in order
        assert_eq!(zip.by_index(0).unwrap().name(), "first.txt");
        assert_eq!(zip.by_index(1).unwrap().name(), "second.txt");
        assert_eq!(zip.by_index(2).unwrap().name(), "third.txt");
    }

    #[test]
    fn synth_docx_with_deflate_compression_round_trips() {
        let test_data = b"The quick brown fox jumps over the lazy dog. ".repeat(10);
        let entries = &[(
            "compressed.bin",
            CompressionMethod::Deflated,
            test_data.as_slice(),
        )];
        let bytes = synth_docx_with_entries(entries);

        let cursor = Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(cursor).expect("should be valid ZIP");
        let mut file = zip.by_index(0).expect("should have first entry");
        let mut decompressed = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut decompressed)
            .expect("should decompress successfully");

        assert_eq!(
            decompressed, test_data,
            "decompressed data should match original"
        );
    }
}
