#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use std::io::{Cursor, Write as _};

fn compressed(content: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(content).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn valid_stream_reaches_stable_eof() {
    let encoded = compressed(b"content");
    let mut reader = DeflateReader::new(Cursor::new(encoded));
    let mut decoded = Vec::new();
    reader.read_to_end(&mut decoded).unwrap();
    assert_eq!(decoded, b"content");
    assert_eq!(reader.read(&mut [0; 1]).unwrap(), 0);
}

#[test]
fn corrupt_stream_returns_invalid_data() {
    let mut reader = DeflateReader::new(Cursor::new(vec![0xff; 8]));
    let error = reader
        .read(&mut [0; 8])
        .expect_err("corrupt stream must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(error.to_string(), "corrupt deflate stream");
}

#[test]
fn truncated_stream_with_pending_output_fails_after_returning_that_output() {
    let mut encoded = compressed(b"abcdefghijklmnopqrstuvwxyz");
    encoded.pop().expect("deflate terminator byte");
    let mut reader = DeflateReader::new(Cursor::new(encoded));
    let mut decoded = Vec::new();
    let error = reader
        .read_to_end(&mut decoded)
        .expect_err("truncated stream must fail");
    assert_eq!(decoded, b"abcdefghijklmnopqrstuvwxyz");
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(error.to_string(), "truncated deflate stream");
}

#[test]
fn no_progress_error_is_stable_invalid_data() {
    let error = no_progress_error();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(error.to_string(), "deflate decoder made no progress");
}
