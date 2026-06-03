//! In-memory DOCX fixture helpers for tests.
//!
//! Builds synthetic DOCX archives from string templates without touching the filesystem.

use std::io::{Cursor, Write as _};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

/// Builds a minimal 2-entry DOCX archive (Deflated) from raw XML strings.
///
/// Entries:
/// - `_rels/.rels` — the relationship file
/// - `word/document.xml` — the main document
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
