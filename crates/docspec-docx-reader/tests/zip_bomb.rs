//! Zip-bomb regression tests for the DOCX reader.
//!
//! A DOCX is a ZIP archive, so a small upload can hide gigabytes of decompressed
//! XML. Two distinct vectors are guarded here, both exercised through the public
//! [`DocxReader`] API:
//!
//! 1. **Small package parts** (`styles.xml`, `numbering.xml`, relationships,
//!    `[Content_Types].xml`) were read fully into memory before parsing. A part
//!    that deflates from a few KB to gigabytes exhausted memory even on the
//!    streaming `from_path` path, which only streams `document.xml`.
//! 2. **A single oversized `document.xml` node** (e.g. one 2 GiB `<w:t>` run) is
//!    read whole by quick-xml, so it inflated the streaming path's scratch buffer
//!    to gigabytes even though a document of many small nodes streams in constant
//!    memory.
//!
//! Both fixtures are synthesized in-process — a 2 GiB→GB decompression bomb must
//! never be committed as a file. The bomb entry is written in chunks so the test
//! never materializes the full decompressed payload while building it.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

use std::io::{Cursor, Write as _};

use docspec_core::{Error, EventSource as _};
use docspec_docx_reader::DocxReader;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Matches `MAX_METADATA_PART_BYTES` and `MAX_XML_NODE_BYTES` in the reader.
/// Kept in sync deliberately: if the reader's cap changes, these tests should be
/// reviewed alongside it.
const CAP: usize = 64 * 1024 * 1024;

/// Writes a stored (uncompressed) ZIP entry.
fn add_stored(writer: &mut ZipWriter<Cursor<Vec<u8>>>, name: &str, content: &[u8]) {
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer.start_file(name, options).unwrap();
    writer.write_all(content).unwrap();
}

/// Writes a deflated ZIP entry: `prefix`, then `fill_len` copies of `fill`, then
/// `suffix`. The fill is streamed in chunks so the builder never holds the full
/// decompressed payload in one allocation.
fn add_deflated_filled(
    writer: &mut ZipWriter<Cursor<Vec<u8>>>,
    name: &str,
    prefix: &[u8],
    fill: u8,
    fill_len: usize,
    suffix: &[u8],
) {
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file(name, options).unwrap();
    writer.write_all(prefix).unwrap();
    let chunk = vec![fill; 1 << 20];
    let mut remaining = fill_len;
    while remaining > 0 {
        let n = remaining.min(chunk.len());
        writer.write_all(&chunk[..n]).unwrap();
        remaining -= n;
    }
    writer.write_all(suffix).unwrap();
}

const ROOT_RELS: &[u8] = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

/// A DOCX whose `word/styles.xml` deflates to just over the metadata cap.
fn styles_bomb_docx() -> Vec<u8> {
    let doc_rels = br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    let document = br#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>hi</w:t></w:r></w:p></w:body></w:document>"#;

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    add_stored(&mut writer, "_rels/.rels", ROOT_RELS);
    add_stored(&mut writer, "word/document.xml", document);
    add_stored(&mut writer, "word/_rels/document.xml.rels", doc_rels);
    add_deflated_filled(&mut writer, "word/styles.xml", b"", b'A', CAP + 1, b"");
    writer.finish().unwrap().into_inner()
}

/// A DOCX whose `word/document.xml` holds one `<w:t>` run well over the node cap.
///
/// The overshoot is 1 MiB rather than a single byte: the per-node window resets
/// between tokens, but `BufReader` pre-buffers up to its capacity (~8 KiB) of the
/// text during the previous token, so the effective cap is `CAP` plus that slop.
/// 1 MiB clears the slop deterministically while staying a real streaming test.
fn document_bigtext_bomb_docx() -> Vec<u8> {
    let prefix = br#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>"#;
    let suffix = b"</w:t></w:r></w:p></w:body></w:document>";

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    add_stored(&mut writer, "_rels/.rels", ROOT_RELS);
    add_deflated_filled(
        &mut writer,
        "word/document.xml",
        prefix,
        b'A',
        CAP + (1 << 20),
        suffix,
    );
    writer.finish().unwrap().into_inner()
}

fn expected_metadata_message(part: &str) -> String {
    format!("package part {part} exceeds the {CAP}-byte limit (possible zip bomb)")
}

const EXPECTED_NODE_MESSAGE: &str = "document.xml node exceeds the size limit (possible zip bomb)";

#[test]
fn styles_bomb_is_rejected_at_construction_via_from_reader() {
    let bomb = styles_bomb_docx();
    match DocxReader::from_reader(Cursor::new(bomb)) {
        Err(Error::Parse { message, position }) => {
            assert_eq!(message, expected_metadata_message("word/styles.xml"));
            assert_eq!(position, None);
        }
        other => panic!("expected Parse error, got {other:?}"),
    }
}

#[test]
fn styles_bomb_is_rejected_at_construction_via_from_path() {
    let bomb = styles_bomb_docx();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bomb).unwrap();
    tmp.flush().unwrap();

    match DocxReader::from_path(tmp.path()) {
        Err(Error::Parse { message, position }) => {
            assert_eq!(message, expected_metadata_message("word/styles.xml"));
            assert_eq!(position, None);
        }
        other => panic!("expected Parse error, got {other:?}"),
    }
}

#[test]
fn document_single_node_bomb_is_rejected_while_streaming_via_from_path() {
    let bomb = document_bigtext_bomb_docx();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bomb).unwrap();
    tmp.flush().unwrap();

    // Construction succeeds — document.xml is streamed lazily on from_path — so
    // the oversized node surfaces when the pump reaches it during iteration.
    let mut reader = DocxReader::from_path(tmp.path()).unwrap();
    let mut saw_node_limit = false;
    loop {
        match reader.next_event() {
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(Error::Io { source }) => {
                assert_eq!(source.to_string(), EXPECTED_NODE_MESSAGE);
                saw_node_limit = true;
                break;
            }
            other => panic!("expected Io node-limit error, got {other:?}"),
        }
    }
    assert!(saw_node_limit, "streaming never hit the node-size limit");
}
