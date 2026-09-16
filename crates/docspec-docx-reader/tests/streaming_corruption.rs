//! Verifies archive corruption handling in the seekable streaming constructor.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

use std::io::{self, Cursor, Write as _};

use docspec_core::{Error, EventSource as _};
use docspec_docx_reader::DocxReader;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const ROOT_RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const DOCUMENT_XML: &str = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>content</w:t></w:r></w:p></w:body></w:document>"#;

fn docx() -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file("_rels/.rels", stored).unwrap();
    writer.write_all(ROOT_RELS.as_bytes()).unwrap();
    writer.start_file("word/document.xml", deflated).unwrap();
    writer.write_all(DOCUMENT_XML.as_bytes()).unwrap();
    writer.finish().unwrap().into_inner()
}

fn central_document_offset(bytes: &[u8]) -> usize {
    let signature = [0x50, 0x4b, 0x01, 0x02];
    bytes
        .windows(signature.len())
        .enumerate()
        .find_map(|(offset, window)| {
            if window != signature {
                return None;
            }
            let name_start = offset.checked_add(46)?;
            let name_end = name_start.checked_add("word/document.xml".len())?;
            (bytes.get(name_start..name_end)? == b"word/document.xml").then_some(offset)
        })
        .expect("document central-directory entry")
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn consume_error(bytes: Vec<u8>) -> Error {
    let mut reader = DocxReader::from_reader_streaming(Cursor::new(bytes)).unwrap();
    loop {
        match reader.next_event() {
            Ok(Some(_)) => {}
            Ok(None) => panic!("corrupt document was accepted"),
            Err(error) => return error,
        }
    }
}

#[test]
fn truncated_deflate_stream_is_rejected() {
    let mut bytes = docx();
    let central = central_document_offset(&bytes);
    let compressed_offset = central + 20;
    let compressed_size = read_u32(&bytes, compressed_offset);
    write_u32(&mut bytes, compressed_offset, compressed_size - 1);
    match consume_error(bytes) {
        Error::Io { source } => {
            assert_eq!(source.kind(), io::ErrorKind::UnexpectedEof);
            assert_eq!(source.to_string(), "truncated deflate stream");
        }
        other => panic!("expected I/O error, got {other:?}"),
    }
}

#[test]
fn declared_uncompressed_length_is_enforced() {
    let mut bytes = docx();
    let central = central_document_offset(&bytes);
    let size_offset = central + 24;
    let size = read_u32(&bytes, size_offset);
    write_u32(&mut bytes, size_offset, size - 1);
    match consume_error(bytes) {
        Error::Io { source } => {
            assert_eq!(source.kind(), io::ErrorKind::InvalidData);
            assert_eq!(
                source.to_string(),
                format!("ZIP entry exceeds declared length {}", size - 1)
            );
        }
        other => panic!("expected I/O error, got {other:?}"),
    }
}

#[test]
fn invalid_archive_and_missing_package_relationships_fail_at_construction() {
    let invalid = DocxReader::from_reader_streaming(Cursor::new(b"not zip".to_vec()))
        .expect_err("invalid ZIP must fail");
    assert_eq!(invalid.to_string(), "parse error: not a valid ZIP archive");

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("word/document.xml", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(DOCUMENT_XML.as_bytes()).unwrap();
    let missing =
        DocxReader::from_reader_streaming(Cursor::new(writer.finish().unwrap().into_inner()))
            .expect_err("missing relationships must fail");
    assert_eq!(missing.to_string(), "parse error: missing _rels/.rels");
}
