#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use super::*;
use std::io::{Cursor, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn stored_metadata(compressed_size: u64, uncompressed_size: u64, crc32: u32) -> EntryMetadata {
    EntryMetadata {
        data_start: 0,
        compressed_size,
        uncompressed_size,
        crc32,
        compression: CompressionMethod::Stored,
    }
}

#[test]
fn strict_entry_rejects_boundary_and_length_mismatches() {
    let boundary = StreamingArchive::from_source(
        Box::new(Cursor::new(vec![0])),
        stored_metadata(2, 2, 0),
        true,
    )
    .err()
    .expect("boundary must fail");
    assert_eq!(
        boundary.to_string(),
        "parse error: ZIP entry exceeds archive boundary"
    );

    let mut length = StreamingArchive::from_source(
        Box::new(Cursor::new(b"a".to_vec())),
        stored_metadata(1, 2, crc32fast::hash(b"a")),
        true,
    )
    .unwrap();
    let error = length
        .read_to_end(&mut Vec::new())
        .expect_err("length mismatch must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        error.to_string(),
        "ZIP entry length mismatch: expected 2, read 1"
    );
}

#[test]
fn strict_entry_rejects_trailing_compressed_bytes() {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(b"payload").unwrap();
    let mut compressed = encoder.finish().unwrap();
    compressed.push(0);
    let metadata = EntryMetadata {
        data_start: 0,
        compressed_size: u64::try_from(compressed.len()).unwrap(),
        uncompressed_size: 7,
        crc32: crc32fast::hash(b"payload"),
        compression: CompressionMethod::Deflated,
    };
    let mut reader =
        StreamingArchive::from_source(Box::new(Cursor::new(compressed)), metadata, true).unwrap();
    let error = reader
        .read_to_end(&mut Vec::new())
        .expect_err("trailing compressed byte must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        error.to_string(),
        format!(
            "ZIP compressed length mismatch: expected {}, read {}",
            reader.compressed_size,
            reader.compressed_size - 1
        )
    );
}

#[test]
fn empty_and_post_verification_reads_return_zero() {
    let mut reader = StreamingArchive::from_source(
        Box::new(Cursor::new(b"a".to_vec())),
        stored_metadata(1, 1, crc32fast::hash(b"a")),
        true,
    )
    .unwrap();
    assert_eq!(reader.read(&mut []).unwrap(), 0);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"a");
    assert_eq!(reader.read(&mut [0; 1]).unwrap(), 0);
}

#[test]
fn legacy_deflate_metadata_can_be_finalized() {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(b"legacy").unwrap();
    let compressed = encoder.finish().unwrap();
    let metadata = EntryMetadata {
        data_start: 0,
        compressed_size: u64::try_from(compressed.len()).unwrap(),
        uncompressed_size: 6,
        crc32: crc32fast::hash(b"legacy"),
        compression: CompressionMethod::Deflated,
    };
    let mut reader =
        StreamingArchive::from_source(Box::new(Cursor::new(compressed)), metadata, false).unwrap();
    reader.verify_integrity = true;
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"legacy");
}

#[test]
fn archive_error_mapping_preserves_io_and_parse_kinds() {
    let io_error = map_archive_error(ZipError::Io(io::Error::new(
        io::ErrorKind::BrokenPipe,
        "archive stopped",
    )));
    match io_error {
        Error::Io { source } => {
            assert_eq!(source.kind(), io::ErrorKind::BrokenPipe);
            assert_eq!(source.to_string(), "archive stopped");
        }
        other => panic!("expected I/O error, got {other:?}"),
    }
    assert_eq!(
        map_archive_error(ZipError::InvalidArchive("bad archive".into())).to_string(),
        "parse error: not a valid ZIP archive: invalid Zip archive: bad archive"
    );
}

struct ToggleReader {
    inner: Cursor<Vec<u8>>,
    fail: Arc<AtomicBool>,
}

impl Read for ToggleReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.fail.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "entry stopped"));
        }
        self.inner.read(buf)
    }
}

impl std::io::Seek for ToggleReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

#[test]
fn entry_metadata_maps_missing_entry_and_source_error() {
    let bytes = docspec_test_utils::synth_docx_with_entries(&[(
        "present",
        CompressionMethod::Stored,
        b"data",
    )]);
    let mut missing = ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
    assert_eq!(
        entry_metadata(&mut missing, "absent")
            .err()
            .expect("missing entry")
            .to_string(),
        "parse error: document target not found: specified file not found in archive"
    );

    let fail = Arc::new(AtomicBool::new(false));
    let toggle_reader = ToggleReader {
        inner: Cursor::new(bytes),
        fail: Arc::clone(&fail),
    };
    let mut archive = ZipArchive::new(toggle_reader).unwrap();
    fail.store(true, Ordering::Relaxed);
    match entry_metadata(&mut archive, "present")
        .err()
        .expect("entry read must fail")
    {
        Error::Io { source } => {
            assert_eq!(source.kind(), io::ErrorKind::BrokenPipe);
            assert_eq!(source.to_string(), "entry stopped");
        }
        other => panic!("expected I/O error, got {other:?}"),
    }
}
