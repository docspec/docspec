//! Integration tests for `OdtReader`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

use std::io::Cursor;

use docspec_core::{Error, Event, EventSource as _};
use docspec_odt_reader::OdtReader;

fn empty_document() -> Vec<Event> {
    vec![
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        },
        Event::EndDocument,
    ]
}

#[test]
fn from_reader_emits_exactly_an_empty_document() {
    let mut reader = OdtReader::from_reader(Cursor::new(b"anything")).unwrap();
    assert_eq!(
        docspec_test_utils::collect_events(&mut reader),
        empty_document()
    );
}

#[test]
fn stays_exhausted_after_end() {
    let mut reader = OdtReader::from_reader(Cursor::new(b"")).unwrap();
    let _ = docspec_test_utils::collect_events(&mut reader);
    assert_eq!(reader.next_event().unwrap(), None);
}

#[test]
fn from_path_emits_exactly_an_empty_document() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), b"anything").unwrap();
    let mut reader = OdtReader::from_path(file.path()).unwrap();
    assert_eq!(
        docspec_test_utils::collect_events(&mut reader),
        empty_document()
    );
}

#[test]
fn from_path_missing_file_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let Err(Error::Io { source }) = OdtReader::from_path(dir.path().join("missing.odt")) else {
        panic!("expected an I/O error");
    };
    assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn from_path_directory_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let Err(Error::Io { source }) = OdtReader::from_path(dir.path()) else {
        panic!("expected an I/O error");
    };
    assert_eq!(source.kind(), std::io::ErrorKind::IsADirectory);
}
