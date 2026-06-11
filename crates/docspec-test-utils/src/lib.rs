//! Internal test fixtures shared across the docspec workspace.
//!
//! This crate is not published. It exists solely to share fixture helpers
//! between test modules of multiple workspace crates.

// Reason: Helpers panic on internal ZIP write failure to keep test call sites
// terse. Equivalent to the per-file allows used in workspace test modules.
#![allow(clippy::expect_used)]

pub mod builders;

use std::io::{Cursor, Write as _};

pub use zip::CompressionMethod;
use zip::{write::SimpleFileOptions, ZipWriter};

mod collect;
mod drive;
pub use collect::{collect_events, try_collect_events};
pub use drive::{drive, try_drive};

mod failing_writer;
pub use failing_writer::FailingWriter;

/// Builds a minimal 2-entry DOCX archive (Deflated) from raw XML strings.
///
/// Entries:
/// - `_rels/.rels` — the relationship file
/// - `word/document.xml` — the main document
///
/// # Panics
///
/// Panics if the ZIP writer fails (should never happen for in-memory buffers).
#[inline]
#[must_use]
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
/// Panics if the ZIP writer fails (should never happen for in-memory buffers).
#[inline]
#[must_use]
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
