//! Integration tests for the DOCX reader re-exported through the docspec facade.

#![cfg(feature = "docx")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::tests_outside_test_module,
    clippy::shadow_unrelated,
    clippy::unused_trait_names
)]

#[cfg(test)]
mod tests {
    use std::io::{Cursor, ErrorKind};

    use docspec::readers::DocxReader;
    use docspec::{Error, EventSource};

    #[test]
    fn docx_reader_implements_event_source() {
        fn assert_event_source<S>()
        where
            S: EventSource,
        {
        }
        assert_event_source::<DocxReader>();
    }

    #[test]
    fn docx_reader_from_path_propagates_not_found_io_error() {
        let result = DocxReader::from_path("/nonexistent/path/does/not/exist.docx");
        let err = result.expect_err("missing file must produce an error");
        assert!(
            matches!(&err, Error::Io { source } if source.kind() == ErrorKind::NotFound),
            "expected Error::Io with ErrorKind::NotFound, got: {err:?}"
        );
    }

    #[test]
    fn docx_reader_from_reader_rejects_non_zip_bytes() {
        let bogus = Cursor::new(b"not a zip archive at all".to_vec());
        let result = DocxReader::from_reader(bogus);
        let err = result.expect_err("non-zip input must produce an error");
        assert_eq!(err.to_string(), "parse error: not a valid ZIP archive");
    }
}

use std::io::Cursor;

use docspec_test_utils::synth_docx;

const SIMPLE_RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

#[test]
fn any_reader_from_reader_docx_hello() {
    use docspec::{AnyReader, InputFormat};
    use docspec_core::{Event, EventSource as _};

    let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body></w:document>"#;
    let bytes = synth_docx(SIMPLE_RELS, doc_xml);
    let mut reader = AnyReader::from_reader(InputFormat::Docx, Cursor::new(bytes)).unwrap();

    let events: Vec<_> = core::iter::from_fn(|| reader.next_event().unwrap()).collect();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::StartDocument { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::StartParagraph { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Text { content, .. } if content == "hello")));
    assert!(events.iter().any(|e| matches!(e, Event::EndParagraph)));
    assert!(events.iter().any(|e| matches!(e, Event::EndDocument)));
}

#[test]
fn any_reader_from_reader_docx_invalid_zip() {
    use docspec::{AnyReader, InputFormat};

    let result = AnyReader::from_reader(InputFormat::Docx, Cursor::new(b"not a zip".to_vec()));
    let err = result.err().expect("invalid zip must produce an error");
    assert!(
        err.to_string().to_lowercase().contains("zip")
            || err.to_string().to_lowercase().contains("archive"),
        "error should mention ZIP/archive: {err}"
    );
}

#[cfg(feature = "markdown")]
#[test]
fn any_reader_from_path_markdown() {
    use docspec::{AnyReader, InputFormat};
    use docspec_core::EventSource as _;

    let mut file = tempfile::Builder::new().suffix(".md").tempfile().unwrap();
    std::io::Write::write_all(&mut file, b"# Hello").unwrap();
    let path = file.path().to_path_buf();

    let mut reader = AnyReader::from_path(InputFormat::Markdown, &path).unwrap();
    let first = reader.next_event().unwrap();
    assert!(first.is_some(), "should emit at least one event");
}

#[test]
fn any_reader_from_path_docx() {
    use docspec::{AnyReader, InputFormat};
    use docspec_core::{Event, EventSource as _};

    let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>file</w:t></w:r></w:p></w:body></w:document>"#;
    let bytes = synth_docx(SIMPLE_RELS, doc_xml);

    let mut file = tempfile::Builder::new().suffix(".docx").tempfile().unwrap();
    std::io::Write::write_all(&mut file, &bytes).unwrap();
    let path = file.path().to_path_buf();

    let mut reader = AnyReader::from_path(InputFormat::Docx, &path).unwrap();
    let events: Vec<_> = core::iter::from_fn(|| reader.next_event().unwrap()).collect();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Text { content, .. } if content == "file")));
}

#[test]
fn any_reader_from_path_missing_file() {
    use docspec::{AnyReader, InputFormat};
    use docspec_core::Error;
    use std::io::ErrorKind;

    let result = AnyReader::from_path(InputFormat::Docx, "/nonexistent/path/does/not/exist.docx");
    let err = result.err().expect("missing file must produce an error");
    assert!(
        matches!(&err, Error::Io { source } if source.kind() == ErrorKind::NotFound),
        "expected Error::Io with NotFound, got: {err:?}"
    );
}

#[cfg(feature = "markdown")]
#[test]
fn any_reader_strip_bom_markdown() {
    use docspec::{AnyReader, InputFormat};
    use docspec_core::{Event, EventSource as _};

    let input = "\u{FEFF}# Hi";
    let mut reader = AnyReader::from_str(InputFormat::Markdown, input).unwrap();
    let events: Vec<_> = core::iter::from_fn(|| reader.next_event().unwrap()).collect();
    let has_hi = events
        .iter()
        .any(|e| matches!(e, Event::Text { content, .. } if content == "Hi"));
    assert!(has_hi, "BOM should be stripped; events: {events:?}");
}
