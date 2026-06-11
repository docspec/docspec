//! Integration tests for `DocxReader`.
#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::redundant_test_prefix,
    clippy::std_instead_of_core,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

mod fixture;

#[test]
fn synth_docx_roundtrips_through_zip_archive() {
    use std::io::Cursor;
    use zip::ZipArchive;

    let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;
    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body>
</w:document>"#;

    let bytes = fixture::synth_docx(rels_xml, document_xml);
    let cursor = Cursor::new(bytes);
    let archive = ZipArchive::new(cursor).expect("should be valid ZIP");
    assert_eq!(
        archive.len(),
        2,
        "expected exactly 2 entries: _rels/.rels and word/document.xml"
    );
}

fn collect_events(bytes: Vec<u8>) -> Vec<docspec_core::Event> {
    use docspec_core::EventSource as _;
    let mut reader =
        docspec_docx_reader::DocxReader::from_reader(std::io::Cursor::new(bytes)).unwrap();
    let mut events = Vec::new();
    loop {
        match reader.next_event() {
            Ok(Some(event)) => events.push(event),
            Ok(None) => break,
            Err(err) => panic!("unexpected error: {err:?}"),
        }
    }
    events
}

mod constructor {
    use std::io::Cursor;
    use std::io::{Read, Seek};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use docspec_core::{Error, Event, EventSource as _};
    use docspec_docx_reader::DocxReader;

    use crate::fixture;

    #[test]
    fn from_reader_succeeds_on_minimal_docx() {
        let bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }

    #[test]
    fn from_reader_errors_on_not_a_zip() {
        let result = DocxReader::from_reader(Cursor::new(b"not a zip".to_vec()));
        match result {
            Err(Error::Parse { message, .. }) => {
                assert_eq!(message, "not a valid ZIP archive");
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_passes_through_zip_open_io_error() {
        let result = DocxReader::from_reader(ErrorReader);
        match result {
            Err(Error::Io { source }) => {
                assert_eq!(source.kind(), std::io::ErrorKind::PermissionDenied);
                assert_eq!(source.to_string(), "zip open denied");
            }
            other => panic!("expected Error::Io, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_errors_when_rels_missing() {
        use std::io::Write as _;
        use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

        let buf = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(buf);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer
            .start_file("word/document.xml", options)
            .expect("start_file");
        writer.write_all(b"<doc/>").expect("write_all");
        let bytes = writer.finish().expect("finish").into_inner();
        let result = DocxReader::from_reader(Cursor::new(bytes));
        match result {
            Err(Error::Parse { message, .. }) => {
                assert_eq!(message, "missing _rels/.rels");
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_errors_when_rels_entry_header_is_malformed() {
        let mut bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        bytes[0] = b'X';

        let result = DocxReader::from_reader(Cursor::new(bytes));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "malformed ZIP: invalid Zip archive: Invalid local file header"
                );
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_errors_after_empty_non_matching_rels() {
        let bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://example.com/not-office" Target="word/document.xml"/></Relationships>"#,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "no officeDocument relationship");
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_errors_after_balanced_nested_non_matching_rels() {
        let bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Group><Relationship Id="rId1" Type="http://example.com/not-office" Target="word/document.xml"></Relationship></Group></Relationships>"#,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "no officeDocument relationship");
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_errors_on_unexpected_closing_rels_element() {
        let bytes = fixture::synth_docx(
            "</Relationships>",
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "malformed _rels/.rels");
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_errors_on_rels_xml_parser_error() {
        let bytes = fixture::synth_docx(
            "<Relationships><",
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "malformed _rels/.rels");
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_errors_on_unclosed_rels_xml() {
        let bytes = fixture::synth_docx(
            "<Relationships>",
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "malformed _rels/.rels");
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_errors_on_missing_target_entry() {
        use std::io::Write as _;
        use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

        let buf = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(buf);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let rels = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/missing.xml"/></Relationships>"#;
        writer
            .start_file("_rels/.rels", options)
            .expect("start_file");
        writer.write_all(rels.as_bytes()).expect("write_all");
        let bytes = writer.finish().expect("finish").into_inner();
        let result = DocxReader::from_reader(Cursor::new(bytes));
        match result {
            Err(Error::Parse { message, .. }) => {
                assert_eq!(message, "document target not found: word/missing.xml");
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_errors_on_unsupported_compression() {
        let bytes = fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.as_bytes(),
            ),
            ("word/document.xml", zip::CompressionMethod::Bzip2, b"<doc/>"),
        ]);
        let result = DocxReader::from_reader(Cursor::new(bytes));
        match result {
            Err(Error::Parse { message, .. }) => {
                assert_eq!(message, "unsupported compression: Bzip2");
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn next_event_passes_through_mid_parse_io_error() {
        let bytes = fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Stored,
                b"<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body></w:body></w:document>",
            ),
        ]);

        let fail_at = document_data_start(&bytes);
        let fail_enabled = Arc::new(AtomicBool::new(false));
        let failing_reader = FailingReader::new(bytes, fail_at, Arc::clone(&fail_enabled));
        let mut reader = DocxReader::from_reader(failing_reader).expect("from_reader");
        fail_enabled.store(true, Ordering::SeqCst);

        assert_eq!(
            reader.next_event().expect("start document"),
            Some(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
        );
        match reader.next_event() {
            Err(Error::Io { source }) => {
                assert_eq!(source.kind(), std::io::ErrorKind::Other);
                assert_eq!(source.to_string(), "forced read failure");
            }
            other => panic!("expected Error::Io, got: {other:?}"),
        }
    }

    fn document_data_start(bytes: &[u8]) -> u64 {
        let cursor = Cursor::new(bytes.to_vec());
        let mut archive = zip::ZipArchive::new(cursor).expect("valid ZIP");
        let data_start = archive
            .by_name("word/document.xml")
            .expect("document entry")
            .data_start()
            .expect("data start");
        data_start
    }

    struct FailingReader {
        cursor: Cursor<Vec<u8>>,
        fail_enabled: Arc<AtomicBool>,
        fail_at: u64,
    }

    impl FailingReader {
        fn new(bytes: Vec<u8>, fail_at: u64, fail_enabled: Arc<AtomicBool>) -> Self {
            Self {
                cursor: Cursor::new(bytes),
                fail_enabled,
                fail_at,
            }
        }
    }

    impl Read for FailingReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.fail_enabled.load(Ordering::SeqCst) && self.cursor.position() >= self.fail_at {
                return Err(std::io::Error::other("forced read failure"));
            }
            self.cursor.read(buf)
        }
    }

    impl Seek for FailingReader {
        fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
            self.cursor.seek(pos)
        }
    }

    struct ErrorReader;

    impl Read for ErrorReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "zip open denied",
            ))
        }
    }

    impl Seek for ErrorReader {
        fn seek(&mut self, _pos: std::io::SeekFrom) -> std::io::Result<u64> {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "zip open denied",
            ))
        }
    }

    #[test]
    fn from_reader_handles_stored_compression() {
        let bytes = fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Stored,
                b"<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body></w:body></w:document>",
            ),
        ]);
        let result = DocxReader::from_reader(Cursor::new(bytes));
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }

    #[test]
    fn from_reader_handles_deflated_compression() {
        let bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }

    #[test]
    fn from_reader_handles_absolute_target_path() {
        let bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="/word/document.xml"/></Relationships>"#,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }

    #[test]
    fn from_reader_handles_non_empty_relationship_element() {
        let bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"></Relationship></Relationships>"#,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }

    #[test]
    fn from_reader_errors_on_rels_parent_reference() {
        let bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="../foo/document.xml"/></Relationships>"#,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "rels target contains parent reference: ../foo/document.xml"
                );
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_errors_on_malformed_rels_attribute() {
        let bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target=word/document.xml/></Relationships>"#,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "malformed _rels/.rels: position 120: attribute value must be enclosed in `\"` or `'`"
                );
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_reader_errors_on_rels_attribute_entity() {
        let bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/&bogus;.xml"/></Relationships>"#,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "malformed _rels/.rels: at 6..11: unrecognized entity `bogus`"
                );
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn from_path_errors_on_missing_file() {
        let result = DocxReader::from_path("/tmp/this_file_does_not_exist_docspec_test.docx");
        match result {
            Err(Error::Io { source }) => {
                assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
            }
            other => panic!("expected Error::Io, got: {other:?}"),
        }
    }

    #[test]
    fn from_path_succeeds_on_tempfile() {
        use std::io::Write as _;

        let bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
        tmp.write_all(&bytes).expect("write");
        let result = DocxReader::from_path(tmp.path());
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }

    #[test]
    fn from_reader_does_not_buffer_document_xml() {
        let big_doc = {
            let mut doc = String::from(
                r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
            );
            for _ in 0..1000 {
                doc.push_str("<w:p><w:r><w:t>hello world</w:t></w:r></w:p>");
            }
            doc.push_str("</w:body></w:document>");
            doc
        };
        let bytes = fixture::synth_docx(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            &big_doc,
        );
        let result = DocxReader::from_reader(Cursor::new(bytes));
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }
}

mod events {
    use std::io::Cursor;

    use docspec_core::{Color, Event, TableHeaderScope, TextAlignment, TextStyleKind};
    use docspec_docx_reader::{DocxReader, EventSource as _};

    use crate::fixture;

    const SIMPLE_RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

    fn make_reader(document_xml: &str) -> DocxReader {
        let bytes = fixture::synth_docx(SIMPLE_RELS, document_xml);
        DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader")
    }

    fn drive(reader: &mut DocxReader) -> Vec<Event> {
        let mut events = Vec::new();
        while let Some(event) = reader.next_event().expect("next_event") {
            events.push(event);
        }
        events
    }

    fn collect_events(content: &str) -> Vec<Event> {
        let document_xml = format!(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{content}</w:body></w:document>"#,
        );
        let mut reader = make_reader(&document_xml);
        drive(&mut reader)
    }

    fn expected_events(mut content_events: Vec<Event>) -> Vec<Event> {
        let mut events = vec![start_doc(), start_para()];
        events.append(&mut content_events);
        events.push(Event::EndParagraph);
        events.push(Event::EndDocument);
        events
    }

    fn start_doc() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    fn start_para() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    fn start_para_with_alignment(alignment: TextAlignment) -> Event {
        Event::StartParagraph {
            alignment: Some(alignment),
            id: None,
        }
    }

    fn text(content: &str) -> Event {
        Event::Text {
            content: content.to_string(),
        }
    }

    fn styled_text_events(kinds: &[TextStyleKind], content: &str) -> Vec<Event> {
        let mut events = Vec::new();
        for kind in kinds {
            events.push(Event::StartTextStyle {
                kind: kind.clone(),
                id: None,
            });
        }
        events.push(Event::Text {
            content: content.to_string(),
        });
        for _kind in kinds.iter().rev() {
            events.push(Event::EndTextStyle);
        }
        events
    }

    fn start_link(href: &str, title: Option<&str>) -> Event {
        Event::StartLink {
            href: href.to_string(),
            id: None,
            title: title.map(str::to_string),
        }
    }

    fn relationship_root() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#
    }

    fn document_with_hyperlink(attrs: &str, content: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:hyperlink {attrs}>{content}</w:hyperlink></w:p></w:body>
</w:document>"#
        )
    }

    fn document_hyperlink_rels(relationship_type: &str, target: &str, target_mode: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="{relationship_type}" Target="{target}" TargetMode="{target_mode}"/>
</Relationships>"#
        )
    }

    fn hyperlink_events(document_xml: &str, document_rels: Option<&str>) -> Vec<Event> {
        let mut entries = vec![
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                relationship_root().as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                document_xml.as_bytes(),
            ),
        ];
        if let Some(rels_xml) = document_rels {
            entries.push((
                "word/_rels/document.xml.rels",
                zip::CompressionMethod::Deflated,
                rels_xml.as_bytes(),
            ));
        }
        let docx = fixture::synth_docx_with_entries(&entries);
        let mut reader = DocxReader::from_reader(Cursor::new(docx)).expect("from_reader");
        drive(&mut reader)
    }

    fn styled_hyperlink_events(document_xml: &str, styles_body: &str) -> Vec<Event> {
        let document_rels = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rId1" Type="{TRANSITIONAL_HYPERLINK_TYPE}" Target="https://example.com" TargetMode="External"/>
</Relationships>"#
        );
        let styles_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  {styles_body}
</w:styles>"#
        );
        let entries = [
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                relationship_root().as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                document_xml.as_bytes(),
            ),
            (
                "word/_rels/document.xml.rels",
                zip::CompressionMethod::Deflated,
                document_rels.as_bytes(),
            ),
            (
                "word/styles.xml",
                zip::CompressionMethod::Deflated,
                styles_xml.as_bytes(),
            ),
        ];
        let docx = fixture::synth_docx_with_entries(&entries);
        let mut reader = DocxReader::from_reader(Cursor::new(docx)).expect("from_reader");
        drive(&mut reader)
    }

    fn hyperlink_text_document(attrs: &str) -> String {
        document_with_hyperlink(attrs, "<w:r><w:t>link</w:t></w:r>")
    }

    fn expected_link_events(href: &str, title: Option<&str>) -> Vec<Event> {
        vec![
            start_doc(),
            start_para(),
            start_link(href, title),
            text("link"),
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]
    }

    fn expected_plain_link_text_events() -> Vec<Event> {
        vec![
            start_doc(),
            start_para(),
            text("link"),
            Event::EndParagraph,
            Event::EndDocument,
        ]
    }

    const STRICT_HYPERLINK_TYPE: &str =
        "http://purl.oclc.org/ooxml/officeDocument/relationships/hyperlink";
    const TRANSITIONAL_HYPERLINK_TYPE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

    mod rpr {
        use super::*;

        #[test]
        fn rpr_bold_applied_to_text() {
            let events = collect_events("<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>x</w:t></w:r></w:p>");
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Bold], "x"))
            );
        }

        #[test]
        fn rpr_italic_applied_to_text() {
            let events = collect_events("<w:p><w:r><w:rPr><w:i/></w:rPr><w:t>x</w:t></w:r></w:p>");
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Italic], "x"))
            );
        }

        #[test]
        fn rpr_strike_applied_to_text() {
            let events =
                collect_events("<w:p><w:r><w:rPr><w:strike/></w:rPr><w:t>x</w:t></w:r></w:p>");
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Strikethrough], "x"))
            );
        }

        #[test]
        fn rpr_dstrike_collapses_to_strikethrough() {
            let events =
                collect_events("<w:p><w:r><w:rPr><w:dstrike/></w:rPr><w:t>x</w:t></w:r></w:p>");
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Strikethrough], "x"))
            );
        }

        #[test]
        fn rpr_combined_bold_italic_strike() {
            let events = collect_events(
                "<w:p><w:r><w:rPr><w:b/><w:i/><w:strike/></w:rPr><w:t>x</w:t></w:r></w:p>",
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(
                    &[
                        TextStyleKind::Bold,
                        TextStyleKind::Italic,
                        TextStyleKind::Strikethrough,
                    ],
                    "x",
                ))
            );
        }

        #[test]
        fn rpr_bold_val_false_disables() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:b w:val="false"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn rpr_bold_val_zero_disables() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn rpr_bold_val_on_enables() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:b w:val="on"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Bold], "x"))
            );
        }

        #[test]
        fn rpr_duplicate_last_wins() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:b/><w:b w:val="false"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn rpr_state_resets_between_runs() {
            let events = collect_events(
                "<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>a</w:t></w:r><w:r><w:t>b</w:t></w:r></w:p>",
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para(),
                    Event::StartTextStyle {
                        kind: TextStyleKind::Bold,
                        id: None,
                    },
                    text("a"),
                    Event::EndTextStyle,
                    text("b"),
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }

        #[test]
        fn rpr_absent_uses_default_style() {
            let events = collect_events("<w:p><w:r><w:t>x</w:t></w:r></w:p>");
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn rpr_underline_single_enables() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:u w:val="single"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Underline], "x"))
            );
        }

        #[test]
        fn rpr_underline_double_enables() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:u w:val="double"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Underline], "x"))
            );
        }

        #[test]
        fn rpr_underline_dotted_enables() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:u w:val="dotted"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Underline], "x"))
            );
        }

        #[test]
        fn rpr_underline_val_none_disables() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:u w:val="none"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn rpr_underline_no_val_means_no_underline() {
            let events = collect_events("<w:p><w:r><w:rPr><w:u/></w:rPr><w:t>x</w:t></w:r></w:p>");
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn rpr_vert_align_subscript() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:vertAlign w:val="subscript"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Subscript], "x"))
            );
        }

        #[test]
        fn rpr_vert_align_superscript() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Superscript], "x"))
            );
        }

        #[test]
        fn rpr_vert_align_baseline_resets() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:vertAlign w:val="superscript"/><w:vertAlign w:val="baseline"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn rpr_vert_align_no_val_treated_lenient() {
            let events =
                collect_events("<w:p><w:r><w:rPr><w:vertAlign/></w:rPr><w:t>x</w:t></w:r></w:p>");
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn rpr_underline_bold_combined() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:b/><w:u w:val="single"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(
                    &[TextStyleKind::Bold, TextStyleKind::Underline],
                    "x",
                ))
            );
        }

        #[test]
        fn tab_inside_styled_run_inherits_style() {
            let events = collect_events("<w:p><w:r><w:rPr><w:b/></w:rPr><w:tab/></w:r></w:p>");
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Bold], "\t"))
            );
        }

        #[test]
        fn empty_styled_run_emits_no_style_events() {
            let events = collect_events("<w:p><w:r><w:rPr><w:b/></w:rPr></w:r></w:p>");
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para(),
                    Event::EndParagraph,
                    Event::EndDocument
                ]
            );
        }

        #[test]
        fn multi_wt_run_shares_single_start_end() {
            let events = collect_events(
                "<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>foo</w:t><w:t>bar</w:t></w:r></w:p>",
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para(),
                    Event::StartTextStyle {
                        kind: TextStyleKind::Bold,
                        id: None,
                    },
                    text("foo"),
                    text("bar"),
                    Event::EndTextStyle,
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }

        #[test]
        fn adjacent_styled_and_unstyled_runs() {
            let events = collect_events(
                "<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>a</w:t></w:r><w:r><w:t>b</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>c</w:t></w:r></w:p>",
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para(),
                    Event::StartTextStyle {
                        kind: TextStyleKind::Bold,
                        id: None,
                    },
                    text("a"),
                    Event::EndTextStyle,
                    text("b"),
                    Event::StartTextStyle {
                        kind: TextStyleKind::Bold,
                        id: None,
                    },
                    text("c"),
                    Event::EndTextStyle,
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }

        #[test]
        fn text_color_red_emits_start_text_style_textcolor_red() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:color w:val="FF0000"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(
                    &[TextStyleKind::TextColor(Color::Rgb { r: 255, g: 0, b: 0 })],
                    "x",
                ))
            );
        }

        #[test]
        fn test_w_color_auto_emits_no_event() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:color w:val="auto"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn test_w_color_black_emitted_unchanged() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:color w:val="000000"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(
                    &[TextStyleKind::TextColor(Color::Rgb { r: 0, g: 0, b: 0 })],
                    "x",
                ))
            );
        }

        #[test]
        fn highlight_yellow_emits_mark_yellow() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:highlight w:val="yellow"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(
                    &[TextStyleKind::Mark(Color::Rgb {
                        r: 255,
                        g: 255,
                        b: 0
                    })],
                    "x",
                ))
            );
        }

        #[test]
        fn test_w_highlight_none_emits_no_event() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:highlight w:val="none"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn highlight_unknown_emits_no_event() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:highlight w:val="orangeMaize"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn shd_fill_yellow_emits_mark_yellow() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:shd w:val="clear" w:fill="FFFF00"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(
                    &[TextStyleKind::Mark(Color::Rgb {
                        r: 255,
                        g: 255,
                        b: 0
                    })],
                    "x",
                ))
            );
        }

        #[test]
        fn test_w_shd_fill_auto_emits_no_event() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:shd w:val="clear" w:fill="auto"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn shd_with_no_fill_attribute_emits_no_event() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:shd w:val="clear"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn test_highlight_wins_over_shd() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:highlight w:val="yellow"/><w:shd w:val="clear" w:fill="FF0000"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(
                    &[TextStyleKind::Mark(Color::Rgb {
                        r: 255,
                        g: 255,
                        b: 0
                    })],
                    "x",
                ))
            );
        }

        #[test]
        fn test_shd_used_when_highlight_none() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:highlight w:val="none"/><w:shd w:val="clear" w:fill="FF0000"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(
                    &[TextStyleKind::Mark(Color::Rgb { r: 255, g: 0, b: 0 })],
                    "x",
                ))
            );
        }

        #[test]
        fn test_consecutive_runs_different_text_color() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:color w:val="FF0000"/></w:rPr><w:t>a</w:t></w:r><w:r><w:rPr><w:color w:val="0000FF"/></w:rPr><w:t>b</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para(),
                    Event::StartTextStyle {
                        kind: TextStyleKind::TextColor(Color::Rgb { r: 255, g: 0, b: 0 }),
                        id: None,
                    },
                    text("a"),
                    Event::EndTextStyle,
                    Event::StartTextStyle {
                        kind: TextStyleKind::TextColor(Color::Rgb { r: 0, g: 0, b: 255 }),
                        id: None,
                    },
                    text("b"),
                    Event::EndTextStyle,
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }

        #[test]
        fn bold_plus_text_color_plus_mark_combined() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:b/><w:color w:val="FF0000"/><w:highlight w:val="yellow"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(
                    &[
                        TextStyleKind::Bold,
                        TextStyleKind::TextColor(Color::Rgb { r: 255, g: 0, b: 0 }),
                        TextStyleKind::Mark(Color::Rgb {
                            r: 255,
                            g: 255,
                            b: 0
                        }),
                    ],
                    "x",
                ))
            );
        }

        #[test]
        fn consecutive_runs_with_same_color_emit_close_and_reopen() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:color w:val="FF0000"/></w:rPr><w:t>a</w:t></w:r><w:r><w:rPr><w:color w:val="FF0000"/></w:rPr><w:t>b</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para(),
                    Event::StartTextStyle {
                        kind: TextStyleKind::TextColor(Color::Rgb { r: 255, g: 0, b: 0 }),
                        id: None,
                    },
                    text("a"),
                    Event::EndTextStyle,
                    Event::StartTextStyle {
                        kind: TextStyleKind::TextColor(Color::Rgb { r: 255, g: 0, b: 0 }),
                        id: None,
                    },
                    text("b"),
                    Event::EndTextStyle,
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }

        #[test]
        fn text_color_emits_no_event_when_in_rpr_is_false() {
            let events =
                collect_events(r#"<w:p><w:r><w:color w:val="FF0000"/><w:t>x</w:t></w:r></w:p>"#);
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn task9_smoke_sym_wingdings_skull() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="Wingdings" w:char="F04E"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn task9_smoke_wt_wingdings_pua() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#xF04E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn task9_smoke_wt_wingdings_raw() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn task9_smoke_reset_between_paragraphs() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p><w:p><w:r><w:rPr><w:rFonts w:ascii="Arial"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para(),
                    text("\u{2620}"),
                    Event::EndParagraph,
                    start_para(),
                    text("N"),
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }
    }

    mod sym_element {
        use super::*;

        #[test]
        fn sym_wingdings_skull_via_sym_element() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="Wingdings" w:char="F04E"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn sym_webdings_via_sym_element() {
            let events =
                collect_events(r#"<w:p><w:r><w:sym w:font="Webdings" w:char="0021"/></w:r></w:p>"#);
            assert_eq!(events, expected_events(vec![text("\u{1F577}")]));
        }

        #[test]
        fn sym_pua_codepoint_stripped() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="Wingdings" w:char="F021"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{1F589}")]));
        }

        #[test]
        fn sym_raw_codepoint_below_pua() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="Wingdings" w:char="0021"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{1F589}")]));
        }

        #[test]
        fn sym_unknown_font_drops() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="ComicSans" w:char="F04E"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![]));
        }

        #[test]
        fn sym_missing_font_attr_drops() {
            let events = collect_events(r#"<w:p><w:r><w:sym w:char="F04E"/></w:r></w:p>"#);
            assert_eq!(events, expected_events(vec![]));
        }

        #[test]
        fn sym_missing_char_attr_drops() {
            let events = collect_events(r#"<w:p><w:r><w:sym w:font="Wingdings"/></w:r></w:p>"#);
            assert_eq!(events, expected_events(vec![]));
        }

        #[test]
        fn sym_unmapped_codepoint_drops() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="Wingdings" w:char="0001"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![]));
        }

        #[test]
        fn sym_malformed_char_hex_drops() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="Wingdings" w:char="ZZZZ"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![]));
        }

        #[test]
        fn sym_inside_rpr_ignored() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:sym w:font="Wingdings" w:char="F04E"/></w:rPr><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn sym_outside_paragraph_ignored() {
            let events = collect_events(
                r#"<w:sym w:font="Wingdings" w:char="F04E"/><w:p><w:r><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("x")]));
        }

        #[test]
        fn sym_overrides_run_font() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:sym w:font="Webdings" w:char="0021"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{1F577}")]));
        }

        #[test]
        fn sym_with_run_styling_applies_styles() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:sym w:font="Wingdings" w:char="F04E"/></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Bold], "\u{2620}")),
            );
        }

        #[test]
        fn sym_after_wt_in_same_run() {
            let events = collect_events(
                r#"<w:p><w:r><w:t>OK </w:t><w:sym w:font="Wingdings" w:char="F04E"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("OK "), text("\u{2620}")]));
        }

        #[test]
        fn sym_case_insensitive_font_name() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="WINGDINGS" w:char="F04E"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }
    }

    mod wt_symbol_transform {
        use super::*;

        #[test]
        fn wt_wingdings_pua_codepoint_transforms() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#xF04E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn wt_wingdings_raw_codepoint_transforms() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn wt_wingdings_dual_codepoints_mixed() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#xF04E;&#x4C;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}\u{2639}")]));
        }

        #[test]
        fn wt_unmapped_codepoint_dropped() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#xF001;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![]));
        }

        #[test]
        fn wt_all_unmapped_drops_entire_text_event() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#xF001;&#xF002;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![]));
        }

        #[test]
        fn wt_out_of_range_codepoint_dropped() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#x1F600;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![]));
        }

        #[test]
        fn wt_wingdings_then_arial_only_first_transforms() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r><w:r><w:t>N</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}"), text("N")]));
        }

        #[test]
        fn wt_no_font_set_passes_through() {
            let events = collect_events("<w:p><w:r><w:t>hello</w:t></w:r></w:p>");
            assert_eq!(events, expected_events(vec![text("hello")]));
        }

        #[test]
        fn wt_non_symbol_font_passes_through() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Arial"/></w:rPr><w:t>hello</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("hello")]));
        }

        #[test]
        fn wt_symbol_font_alongside_styling() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/><w:b/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Bold], "\u{2620}"))
            );
        }

        #[test]
        fn wt_multiple_wt_in_one_run_each_transforms() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#xF04E;</w:t><w:t>&#x4C;</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(vec![text("\u{2620}"), text("\u{2639}")])
            );
        }

        #[test]
        fn wt_symbol_text_emits_no_empty_text_event() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t></w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![]));
        }

        #[test]
        fn wt_reset_between_paragraphs() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p><w:p><w:r><w:t>N</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para(),
                    text("\u{2620}"),
                    Event::EndParagraph,
                    start_para(),
                    text("N"),
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }

        #[test]
        fn wt_reset_between_runs() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r><w:r><w:t>N</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}"), text("N")]));
        }
    }

    mod ppr {
        use super::*;

        fn expected_paragraph_events(start: Event) -> Vec<Event> {
            vec![start_doc(), start, Event::EndParagraph, Event::EndDocument]
        }

        #[test]
        fn ppr_jc_center_sets_alignment() {
            let events = collect_events(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr></w:p>"#);
            assert_eq!(
                events,
                expected_paragraph_events(start_para_with_alignment(TextAlignment::Center))
            );
        }

        #[test]
        fn ppr_jc_left_sets_left() {
            let events = collect_events(r#"<w:p><w:pPr><w:jc w:val="left"/></w:pPr></w:p>"#);
            assert_eq!(
                events,
                expected_paragraph_events(start_para_with_alignment(TextAlignment::Left))
            );
        }

        #[test]
        fn ppr_jc_start_maps_to_left() {
            let events = collect_events(r#"<w:p><w:pPr><w:jc w:val="start"/></w:pPr></w:p>"#);
            assert_eq!(
                events,
                expected_paragraph_events(start_para_with_alignment(TextAlignment::Left))
            );
        }

        #[test]
        fn ppr_jc_right_sets_right() {
            let events = collect_events(r#"<w:p><w:pPr><w:jc w:val="right"/></w:pPr></w:p>"#);
            assert_eq!(
                events,
                expected_paragraph_events(start_para_with_alignment(TextAlignment::Right))
            );
        }

        #[test]
        fn ppr_jc_end_maps_to_right() {
            let events = collect_events(r#"<w:p><w:pPr><w:jc w:val="end"/></w:pPr></w:p>"#);
            assert_eq!(
                events,
                expected_paragraph_events(start_para_with_alignment(TextAlignment::Right))
            );
        }

        #[test]
        fn ppr_jc_both_sets_justify() {
            let events = collect_events(r#"<w:p><w:pPr><w:jc w:val="both"/></w:pPr></w:p>"#);
            assert_eq!(
                events,
                expected_paragraph_events(start_para_with_alignment(TextAlignment::Justify))
            );
        }

        #[test]
        fn ppr_jc_distribute_sets_justify() {
            let events = collect_events(r#"<w:p><w:pPr><w:jc w:val="distribute"/></w:pPr></w:p>"#);
            assert_eq!(
                events,
                expected_paragraph_events(start_para_with_alignment(TextAlignment::Justify))
            );
        }

        #[test]
        fn ppr_jc_unmapped_leaves_alignment_none() {
            let events =
                collect_events(r#"<w:p><w:pPr><w:jc w:val="mediumKashida"/></w:pPr></w:p>"#);
            assert_eq!(events, expected_paragraph_events(start_para()));
        }

        #[test]
        fn ppr_jc_no_val_leaves_alignment_none() {
            let events = collect_events("<w:p><w:pPr><w:jc/></w:pPr></w:p>");
            assert_eq!(events, expected_paragraph_events(start_para()));
        }

        #[test]
        fn ppr_absent_emits_start_paragraph_at_first_content() {
            let events = collect_events("<w:p><w:r><w:t>x</w:t></w:r></w:p>");
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para(),
                    text("x"),
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }

        #[test]
        fn ppr_empty_emits_default_alignment() {
            let events = collect_events("<w:p><w:pPr/></w:p>");
            assert_eq!(events, expected_paragraph_events(start_para()));
        }

        #[test]
        fn empty_paragraph_still_emits_start_end() {
            let events = collect_events("<w:p></w:p>");
            assert_eq!(events, expected_paragraph_events(start_para()));
        }

        #[test]
        fn ppr_jc_followed_by_run_emits_in_order() {
            let events = collect_events(
                r#"<w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:t>x</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para_with_alignment(TextAlignment::Right),
                    text("x"),
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }

        #[test]
        fn ppr_rpr_inside_ppr_is_ignored() {
            let events = collect_events(
                "<w:p><w:pPr><w:rPr><w:b/></w:rPr></w:pPr><w:r><w:t>x</w:t></w:r></w:p>",
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para(),
                    text("x"),
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }
    }

    mod rfonts_resolution {
        use super::*;

        #[test]
        fn rfonts_ascii_only_resolves() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn rfonts_h_ansi_only_resolves() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:hAnsi="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn rfonts_cs_only_resolves() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:cs="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn rfonts_ascii_takes_precedence_over_h_ansi() {
            // Wingdings 0x4E → U+2620 (skull); Webdings 0x4E → U+1F441 (eye)
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings" w:hAnsi="Webdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn rfonts_h_ansi_takes_precedence_over_cs() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:hAnsi="Wingdings" w:cs="Webdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn rfonts_east_asia_ignored() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:eastAsia="Wingdings"/></w:rPr><w:t>N</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("N")]));
        }

        #[test]
        fn rfonts_unknown_in_ascii_falls_through_to_h_ansi() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn rfonts_unknown_in_ascii_and_h_ansi_uses_cs() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Helvetica" w:cs="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn rfonts_all_unknown_no_transform() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Helvetica" w:cs="Times"/></w:rPr><w:t>N</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("N")]));
        }

        #[test]
        fn rfonts_case_insensitive_all_caps() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="WINGDINGS"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn rfonts_case_insensitive_lowercase() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn rfonts_case_insensitive_mixed() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="WiNgDiNgS"/></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn rfonts_wingdings_2_with_space() {
            // Wingdings 2 0x21 → U+270A (raised fist ✊)
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings 2"/></w:rPr><w:t>&#x21;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{270a}")]));
        }

        #[test]
        fn rfonts_wingdings2_no_space_does_not_resolve() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings2"/></w:rPr><w:t>&#x21;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("!")]));
        }

        #[test]
        fn rfonts_non_self_closing_form_handled() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"></w:rFonts></w:rPr><w:t>&#x4E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }
    }

    #[test]
    fn single_paragraph_emits_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("hello"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn debug_redacts_xml_reader() {
        let reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );

        assert_eq!(
            format!("{reader:?}"),
            "DocxReader { inner: DocumentReader { buf: [], denied_stack: [], in_paragraph: false, in_text: false, in_ppr: false, pending_paragraph_alignment: None, pending_paragraph_classification: None, current_paragraph_block: Paragraph, paragraph_started_emitted: false, in_rpr: false, pending_run_kinds: [], pending_run_text_color: None, pending_run_mark: None, pending_run_shade: None, pending_text: \"\", frozen_run_kinds: [], frozen_run_text_color: None, frozen_run_mark: None, pending_run_font: None, frozen_run_font: None, open_styles: [], phase: \"<phase>\", queue: [], run_content_emitted: false, data: \"<DocxData>\", hyperlink_map: {}, hyperlink_depth: 0, pending_link: None, list_stack: [], seen_lists: {}, pending_paragraph_list: None, in_numpr: false, pending_num_pr_id: None, pending_num_pr_ilvl: None, xml: \"<quick_xml::Reader>\" } }"
        );
    }

    #[test]
    fn multiple_paragraphs() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>foo</w:t></w:r></w:p><w:p><w:r><w:t>bar</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("foo"),
                Event::EndParagraph,
                start_para(),
                text("bar"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn empty_paragraph_emits_paragraph_pair_only() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument
            ]
        );
    }

    #[test]
    fn empty_paragraph_element_emits_paragraph_pair() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p/></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument
            ]
        );
    }

    #[test]
    fn self_closing_drawing_emits_no_events() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:drawing/></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(events, vec![start_doc(), Event::EndDocument]);
    }

    #[test]
    fn empty_document_body() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(events, vec![start_doc(), Event::EndDocument]);
    }

    #[test]
    fn multiple_runs_in_one_paragraph_emit_separate_text_events() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>foo</w:t></w:r><w:r><w:t>bar</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("foo"),
                text("bar"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn wt_outside_wp_is_silently_dropped() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:t>orphan</w:t></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(events, vec![start_doc(), Event::EndDocument]);
    }

    #[test]
    fn wins_subtree_passes_through_inside_paragraph() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>before</w:t></w:r><w:ins><w:r><w:t>inserted</w:t></w:r></w:ins><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("before"),
                text("inserted"),
                text("after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn wdel_subtree_suppressed() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>before</w:t></w:r><w:del><w:r><w:t>deleted</w:t></w:r></w:del><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("before"),
                text("after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn paragraph_containing_only_ins_emits_inserted_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins><w:r><w:t>x</w:t></w:r></w:ins></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument
            ]
        );
    }

    #[test]
    fn xml_space_preserve_whitespace_is_preserved() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t xml:space="preserve"> hello  world </w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text(" hello  world "),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn xml_entities_unescaped_once() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a &amp; b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a & b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn xml_general_ref_unescaped_once() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a &lt; b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a < b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn xml_general_ref_outside_text_is_ignored() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>&amp;<w:p><w:r><w:t>x</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn unknown_xml_entity_returns_parse_error() {
        let bytes = fixture::synth_docx(
            SIMPLE_RELS,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a &bogus; b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
        assert_eq!(
            reader.next_event().expect("start document"),
            Some(start_doc())
        );
        assert_eq!(
            reader.next_event().expect("start paragraph"),
            Some(start_para())
        );
        match reader.next_event() {
            Err(docspec_core::Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "malformed document.xml: at 1..6: unrecognized entity `bogus`"
                );
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn namespace_prefix_variation_handled() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><ns0:document xmlns:ns0="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><ns0:body><ns0:p><ns0:r><ns0:t>x</ns0:t></ns0:r></ns0:p></ns0:body></ns0:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn next_event_idempotent_after_end_document() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body></w:body></w:document>"#,
        );
        loop {
            if reader.next_event().expect("next_event").is_none() {
                break;
            }
        }
        assert_eq!(reader.next_event().expect("1st extra"), None);
        assert_eq!(reader.next_event().expect("2nd extra"), None);
        assert_eq!(reader.next_event().expect("3rd extra"), None);
    }

    #[test]
    fn malformed_document_xml_returns_error_parse() {
        let bytes = fixture::synth_docx(SIMPLE_RELS, "<w:p");
        let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
        let first = reader.next_event().expect("first call");
        assert_eq!(
            first,
            Some(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
        );
        let second = reader.next_event();
        match second {
            Err(docspec_core::Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "malformed document.xml: syntax error: tag not closed: `>` not found before end of input"
                );
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn xml_decl_doctype_processing_instruction_and_comment_are_ignored() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?>
<!DOCTYPE w:document>
<?docspec before-root?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><!-- body comment --><?docspec inside-body?><w:p><w:r><w:t>visible</w:t></w:r></w:p></w:body>
</w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("visible"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn malformed_utf8_text_returns_parse_error() {
        let bytes = fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                SIMPLE_RELS.as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                b"<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:r><w:t>\xFF</w:t></w:r></w:p></w:body></w:document>",
            ),
        ]);
        let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
        assert_eq!(
            reader.next_event().expect("start document"),
            Some(start_doc())
        );
        assert_eq!(
            reader.next_event().expect("start paragraph"),
            Some(start_para())
        );
        match reader.next_event() {
            Err(docspec_core::Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "malformed document.xml: cannot decode input using UTF-8: invalid utf-8 sequence of 1 bytes from index 0"
                );
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn cdata_inside_text_emits_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t><![CDATA[hello <world>]]></w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("hello <world>"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn cdata_outside_text_is_ignored() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><![CDATA[ignored]]><w:p><w:r><w:t>kept</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("kept"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn malformed_utf8_cdata_returns_parse_error() {
        let bytes = fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                SIMPLE_RELS.as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                b"<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:r><w:t><![CDATA[\xFF]]></w:t></w:r></w:p></w:body></w:document>",
            ),
        ]);
        let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
        assert_eq!(
            reader.next_event().expect("start document"),
            Some(start_doc())
        );
        assert_eq!(
            reader.next_event().expect("start paragraph"),
            Some(start_para())
        );
        match reader.next_event() {
            Err(docspec_core::Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "malformed document.xml: invalid utf-8 sequence of 1 bytes from index 0"
                );
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
    }

    #[test]
    fn eof_mid_text_flushes_text_and_closes_paragraph() {
        let bytes = fixture::synth_docx(
            SIMPLE_RELS,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>partial"#,
        );
        let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("partial"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn eof_mid_paragraph_auto_closes() {
        let bytes = fixture::synth_docx(
            SIMPLE_RELS,
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p>"#,
        );
        let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument
            ]
        );
    }

    #[test]
    fn w_tab_self_closing_emits_text_tab_between_text_runs() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t><w:tab/><w:t>b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a"),
                text("\t"),
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_tab_between_separate_runs_emits_text_tab() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>foo</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>bar</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("foo"),
                text("\t"),
                text("bar"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_tab_at_paragraph_start_emits_text_tab() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:tab/><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("\t"),
                text("after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_tab_at_paragraph_end_emits_text_tab() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>before</w:t><w:tab/></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("before"),
                text("\t"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_tab_only_paragraph_emits_text_tab() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:tab/></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("\t"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_tab_with_end_tag_emits_single_text_tab() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t><w:tab></w:tab><w:t>b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a"),
                text("\t"),
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_tab_outside_paragraph_is_silently_dropped() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tab/><w:p><w:r><w:t>x</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_tab_inside_drawing_is_silently_dropped() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>kept</w:t></w:r><w:drawing><w:r><w:tab/></w:r></w:drawing></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("kept"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_tab_inside_table_cell_emits_text_tab() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t><w:tab/><w:t>b</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartTable { id: None },
                Event::StartTableRow { id: None },
                Event::StartTableCell {
                    colspan: None,
                    id: None,
                    rowspan: None,
                },
                start_para(),
                text("a"),
                text("\t"),
                text("b"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn multiple_w_tab_in_sequence_emit_multiple_text_tabs() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:tab/><w:tab/><w:tab/></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("\t"),
                text("\t"),
                text("\t"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_br_self_closing_emits_line_break_between_text_runs() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t><w:br/><w:t>b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a"),
                Event::LineBreak,
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_br_between_separate_runs_emits_line_break() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>foo</w:t></w:r><w:r><w:br/></w:r><w:r><w:t>bar</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("foo"),
                Event::LineBreak,
                text("bar"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_br_at_paragraph_start_emits_line_break() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:br/><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::LineBreak,
                text("after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_br_at_paragraph_end_emits_line_break() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>before</w:t><w:br/></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("before"),
                Event::LineBreak,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_br_only_paragraph_emits_line_break() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:br/></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::LineBreak,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_br_with_end_tag_emits_single_line_break() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t><w:br></w:br><w:t>b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a"),
                Event::LineBreak,
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_br_with_page_type_emits_line_break() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t><w:br w:type="page"/><w:t>b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a"),
                Event::LineBreak,
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_br_with_column_type_emits_line_break() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t><w:br w:type="column"/><w:t>b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a"),
                Event::LineBreak,
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_br_outside_paragraph_is_silently_dropped() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:br/><w:p><w:r><w:t>x</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_br_inside_drawing_is_silently_dropped() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>kept</w:t></w:r><w:drawing><w:r><w:br/></w:r></w:drawing></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("kept"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn w_br_inside_table_cell_emits_line_break() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t><w:br/><w:t>b</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartTable { id: None },
                Event::StartTableRow { id: None },
                Event::StartTableCell {
                    colspan: None,
                    id: None,
                    rowspan: None,
                },
                start_para(),
                text("a"),
                Event::LineBreak,
                text("b"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn multiple_w_br_in_sequence_emit_multiple_line_breaks() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:br/><w:br/><w:br/></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::LineBreak,
                Event::LineBreak,
                Event::LineBreak,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    fn start_table() -> Event {
        Event::StartTable { id: None }
    }

    fn start_row() -> Event {
        Event::StartTableRow { id: None }
    }

    fn start_cell() -> Event {
        Event::StartTableCell {
            colspan: None,
            id: None,
            rowspan: None,
        }
    }

    #[test]
    fn simple_table_emits_full_structure() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("cell"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn multi_row_multi_cell_table_emits_full_structure() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>c</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("a"),
                Event::EndParagraph,
                Event::EndTableCell,
                start_cell(),
                start_para(),
                text("b"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                start_row(),
                start_cell(),
                start_para(),
                text("c"),
                Event::EndParagraph,
                Event::EndTableCell,
                start_cell(),
                start_para(),
                text("d"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn empty_cell_emits_paragraph_pair() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn multiple_paragraphs_in_cell_emit_multiple_paragraphs() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>first</w:t></w:r></w:p><w:p><w:r><w:t>second</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("first"),
                Event::EndParagraph,
                start_para(),
                text("second"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn nested_table_emits_nested_events() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tbl><w:tr><w:tc><w:p><w:r><w:t>inner</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("inner"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn table_inside_ins_passes_through() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>before</w:t></w:r></w:p><w:ins><w:tbl><w:tr><w:tc><w:p><w:r><w:t>inserted</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:ins><w:p><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("before"),
                Event::EndParagraph,
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("inserted"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                start_para(),
                text("after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn outer_cell_content_after_nested_table_stays_inside_outer_cell() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>before</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>inner</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:r><w:t>after</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("before"),
                Event::EndParagraph,
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("inner"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                start_para(),
                text("after"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn nested_table_inside_outer_header_cell_preserves_outer_header_end() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tbl><w:tr><w:tc><w:p><w:r><w:t>inner</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                Event::StartTableHeader {
                    scope: Some(TableHeaderScope::Column),
                    abbr: None,
                    colspan: None,
                    rowspan: None,
                    id: None,
                },
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("inner"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn nested_table_inside_outer_header_row_preserves_following_header_cell() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tbl><w:tr><w:tc><w:p><w:r><w:t>inner</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc><w:tc><w:p><w:r><w:t>after</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                Event::StartTableHeader {
                    scope: Some(TableHeaderScope::Column),
                    abbr: None,
                    colspan: None,
                    rowspan: None,
                    id: None,
                },
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("inner"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndTableHeader,
                Event::StartTableHeader {
                    scope: Some(TableHeaderScope::Column),
                    abbr: None,
                    colspan: None,
                    rowspan: None,
                    id: None,
                },
                start_para(),
                text("after"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    // gridSpan -> colspan, tblHeader -> StartTableHeader, vMerge ignored (rowspan deferred), tblPr/tcW ignored (table/cell visual props out of scope).
    #[test]
    fn table_properties_now_emit_colspan_header_and_drop_vmerge() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="5000" w:type="pct"/></w:tblPr><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="2500" w:type="pct"/><w:gridSpan w:val="2"/><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                Event::StartTableHeader {
                    scope: Some(TableHeaderScope::Column),
                    abbr: None,
                    colspan: Some(2),
                    rowspan: None,
                    id: None,
                },
                start_para(),
                text("cell"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn table_grid_emits_no_events() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tblGrid><w:gridCol w:w="2880"/><w:gridCol w:w="2880"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("a"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn table_with_tbl_pr_ex_emits_no_extra_events() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tblPrEx><w:tblBorders><w:top w:val="single"/></w:tblBorders></w:tblPrEx><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("cell"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_text_inside_cell_passes_through() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>keep</w:t></w:r><w:hyperlink><w:r><w:t>link</w:t></w:r></w:hyperlink></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("keep"),
                text("link"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn unknown_container_passes_children_through() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:customXml><w:p><w:r><w:t>x</w:t></w:r></w:p></w:customXml></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn rpr_after_text_in_same_run_is_ignored() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>foo</w:t><w:rPr><w:b/></w:rPr></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("foo"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn rpr_after_text_with_more_content_does_not_affect_subsequent_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t><w:rPr><w:b/></w:rPr><w:t>b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a"),
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn ppr_after_run_in_same_paragraph_is_ignored() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>foo</w:t></w:r><w:pPr><w:jc w:val="center"/></w:pPr></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("foo"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn ppr_then_run_then_ppr_only_first_applies() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:t>x</w:t></w:r><w:pPr><w:jc w:val="left"/></w:pPr></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para_with_alignment(TextAlignment::Right),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn out_of_order_rpr_does_not_corrupt_next_run() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t><w:rPr><w:b/></w:rPr></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a"),
                Event::StartTextStyle {
                    kind: TextStyleKind::Italic,
                    id: None,
                },
                text("b"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn empty_rpr_self_closed_emits_default_style() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr/><w:t>x</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn empty_rpr_open_close_emits_default_style() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr></w:rPr><w:t>x</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn empty_ppr_self_closed_emits_default_alignment() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr/><w:r><w:t>x</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    // Uses <w:lang> as a known-but-unhandled rPr child to exercise the default-ignore path.
    fn rpr_with_unknown_child_is_no_op() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:lang w:val="en-US"/><w:b/></w:rPr><w:t>x</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                text("x"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn ppr_with_unknown_child_is_no_op() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:ind w:left="720"/><w:jc w:val="center"/></w:pPr><w:r><w:t>x</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para_with_alignment(TextAlignment::Center),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn rpr_after_line_break_in_same_run_is_ignored() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:br/><w:rPr><w:b/></w:rPr></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::LineBreak,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn multiple_paragraphs_state_resets_between() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>a</w:t></w:r></w:p><w:p><w:r><w:t>b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para_with_alignment(TextAlignment::Right),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                text("a"),
                Event::EndTextStyle,
                Event::EndParagraph,
                start_para(),
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    mod per_font_smoke {
        use super::*;

        #[test]
        fn font_wingdings_skull_via_sym() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="Wingdings" w:char="F04E"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn font_wingdings_skull_via_wt() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#xF04E;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}")]));
        }

        #[test]
        fn font_wingdings2_via_sym() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="Wingdings 2" w:char="0021"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{270a}")]));
        }

        #[test]
        fn font_wingdings2_via_wt() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings 2"/></w:rPr><w:t>&#x21;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{270a}")]));
        }

        #[test]
        fn font_wingdings3_via_sym() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="Wingdings 3" w:char="0021"/></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2B60}")]));
        }

        #[test]
        fn font_wingdings3_via_wt() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings 3"/></w:rPr><w:t>&#x21;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2B60}")]));
        }

        #[test]
        fn font_webdings_via_sym() {
            let events =
                collect_events(r#"<w:p><w:r><w:sym w:font="Webdings" w:char="0021"/></w:r></w:p>"#);
            assert_eq!(events, expected_events(vec![text("\u{1F577}")]));
        }

        #[test]
        fn font_webdings_via_wt() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Webdings"/></w:rPr><w:t>&#x21;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{1F577}")]));
        }

        #[test]
        fn font_symbol_alpha_via_sym() {
            let events =
                collect_events(r#"<w:p><w:r><w:sym w:font="Symbol" w:char="0061"/></w:r></w:p>"#);
            assert_eq!(events, expected_events(vec![text("\u{3b1}")]));
        }

        #[test]
        fn font_symbol_alpha_via_wt() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Symbol"/></w:rPr><w:t>&#x61;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{3b1}")]));
        }
    }

    mod edge_cases {
        use super::*;

        #[test]
        fn edge_queue_length_bounded_under_symbol_heavy_run() {
            let sym_xml = r#"<w:sym w:font="Wingdings" w:char="F04E"/>"#.repeat(50);
            let xml = format!("<w:p><w:r>{sym_xml}</w:r></w:p>");
            let events = collect_events(&xml);
            let expected_texts: Vec<Event> = std::iter::repeat_with(|| text("\u{2620}"))
                .take(50)
                .collect();
            assert_eq!(events, expected_events(expected_texts));
        }

        #[test]
        fn edge_sym_with_paragraph_styling_preserves_alignment() {
            let events = collect_events(
                r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:sym w:font="Wingdings" w:char="F04E"/></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para_with_alignment(TextAlignment::Center),
                    text("\u{2620}"),
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }

        #[test]
        fn edge_consecutive_sym_elements_in_run() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="Wingdings" w:char="F04E"/><w:sym w:font="Wingdings" w:char="F04E"/><w:sym w:font="Wingdings" w:char="F04E"/></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(vec![text("\u{2620}"), text("\u{2620}"), text("\u{2620}")])
            );
        }

        #[test]
        fn edge_sym_and_wt_alternating() {
            let events = collect_events(
                r#"<w:p><w:r><w:sym w:font="Wingdings" w:char="F04E"/><w:t>a</w:t><w:sym w:font="Wingdings" w:char="F04E"/><w:t>b</w:t></w:r></w:p>"#,
            );
            assert_eq!(
                events,
                expected_events(vec![
                    text("\u{2620}"),
                    text("a"),
                    text("\u{2620}"),
                    text("b")
                ])
            );
        }

        #[test]
        fn edge_sym_inside_table_cell() {
            let events = collect_events(
                r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:sym w:font="Wingdings" w:char="F04E"/></w:r></w:p></w:tc></w:tr></w:tbl>"#,
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_table(),
                    start_row(),
                    start_cell(),
                    start_para(),
                    text("\u{2620}"),
                    Event::EndParagraph,
                    Event::EndTableCell,
                    Event::EndTableRow,
                    Event::EndTable,
                    Event::EndDocument,
                ]
            );
        }

        #[test]
        fn edge_unmapped_partial_text_emits_partial() {
            let events = collect_events(
                r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#xF04E;&#xF001;&#xF04C;</w:t></w:r></w:p>"#,
            );
            assert_eq!(events, expected_events(vec![text("\u{2620}\u{2639}")]));
        }

        #[test]
        fn edge_table_existing_test_still_passes() {
            let events = collect_events(
                "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>",
            );
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_table(),
                    start_row(),
                    start_cell(),
                    start_para(),
                    text("cell"),
                    Event::EndParagraph,
                    Event::EndTableCell,
                    Event::EndTableRow,
                    Event::EndTable,
                    Event::EndDocument,
                ]
            );
        }

        #[test]
        fn edge_bold_text_existing_test_still_passes() {
            let events = collect_events("<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>x</w:t></w:r></w:p>");
            assert_eq!(
                events,
                expected_events(styled_text_events(&[TextStyleKind::Bold], "x"))
            );
        }

        #[test]
        fn edge_cross_task_wingdings_wt_webdings_sym_arial_passthrough() {
            let events = collect_events(concat!(
                r#"<w:p>"#,
                r#"<w:r><w:rPr><w:rFonts w:ascii="Wingdings"/></w:rPr><w:t>&#x4E;</w:t></w:r>"#,
                r#"<w:r><w:sym w:font="Webdings" w:char="0021"/></w:r>"#,
                r#"<w:r><w:rPr><w:rFonts w:ascii="Arial"/></w:rPr><w:t>hello</w:t></w:r>"#,
                r#"</w:p>"#,
            ));
            assert_eq!(
                events,
                vec![
                    start_doc(),
                    start_para(),
                    text("\u{2620}"),
                    text("\u{1F577}"),
                    text("hello"),
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }
    }

    fn start_header() -> Event {
        Event::StartTableHeader {
            scope: Some(TableHeaderScope::Column),
            abbr: None,
            colspan: None,
            rowspan: None,
            id: None,
        }
    }

    #[test]
    fn tbl_header_basic_emits_table_header_event() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_header(),
                start_para(),
                text("h"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_multiple_consecutive_header_rows_all_emit_headers() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>h1</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>h2</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>h3</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_header(),
                start_para(),
                text("h1"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                start_row(),
                start_header(),
                start_para(),
                text("h2"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                start_row(),
                start_header(),
                start_para(),
                text("h3"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_val_true_emits_header() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader w:val="true"/></w:trPr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_header(),
                start_para(),
                text("h"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_val_1_emits_header() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader w:val="1"/></w:trPr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_header(),
                start_para(),
                text("h"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_val_on_emits_header() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader w:val="on"/></w:trPr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_header(),
                start_para(),
                text("h"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_val_false_emits_data_cell() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader w:val="false"/></w:trPr><w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("d"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_val_0_emits_data_cell() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader w:val="0"/></w:trPr><w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("d"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_val_off_emits_data_cell() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader w:val="off"/></w:trPr><w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("d"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    // OOXML §17.4.49: once a non-header row appears, subsequent tblHeader markers are ignored
    #[test]
    fn tbl_header_non_contiguous_is_ignored() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_header(),
                start_para(),
                text("h"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                start_row(),
                start_cell(),
                start_para(),
                text("d"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                start_row(),
                start_cell(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_with_gridspan_emits_table_header_with_colspan() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                Event::StartTableHeader {
                    scope: Some(TableHeaderScope::Column),
                    abbr: None,
                    colspan: Some(2),
                    rowspan: None,
                    id: None,
                },
                start_para(),
                text("h"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_in_nested_table_does_not_propagate() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>inner</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("inner"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_with_empty_trpr_emits_data_cell() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr/><w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("d"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_with_other_trpr_children_still_emits_header() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:trHeight w:val="240"/><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_header(),
                start_para(),
                text("h"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_multi_paragraph_header_cell_emits_paragraphs_inside_header() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>p1</w:t></w:r></w:p><w:p><w:r><w:t>p2</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_header(),
                start_para(),
                text("p1"),
                Event::EndParagraph,
                start_para(),
                text("p2"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tbl_header_first_row_has_header_but_second_does_not_keeps_first_as_header() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_header(),
                start_para(),
                text("h"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                start_row(),
                start_cell(),
                start_para(),
                text("d"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn gridspan_two_emits_colspan_some_two() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                Event::StartTableCell {
                    colspan: Some(2),
                    rowspan: None,
                    id: None,
                },
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn gridspan_one_emits_colspan_none() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="1"/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn gridspan_no_val_emits_colspan_none() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn gridspan_zero_emits_colspan_none() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="0"/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn gridspan_non_numeric_emits_colspan_none() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="abc"/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn gridspan_large_value_emits_colspan_some() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="100"/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                Event::StartTableCell {
                    colspan: Some(100),
                    rowspan: None,
                    id: None,
                },
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn gridspan_with_other_tcpr_children_still_emits_colspan() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:tcW w:w="2500" w:type="pct"/><w:gridSpan w:val="2"/><w:shd w:val="clear"/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                Event::StartTableCell {
                    colspan: Some(2),
                    rowspan: None,
                    id: None,
                },
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn gridspan_after_cell_content_started_is_ignored() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>x</w:t></w:r></w:p><w:tcPr><w:gridSpan w:val="3"/></w:tcPr></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn gridspan_in_nested_table_still_works() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_table(),
                start_row(),
                Event::StartTableCell {
                    colspan: Some(2),
                    rowspan: None,
                    id: None,
                },
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn gridspan_in_header_row_emits_table_header_with_colspan() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                Event::StartTableHeader {
                    scope: Some(TableHeaderScope::Column),
                    abbr: None,
                    colspan: Some(2),
                    rowspan: None,
                    id: None,
                },
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_with_text_inside_paragraph_emits_inline_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>see </w:t></w:r><w:hyperlink><w:r><w:t>link</w:t></w:r></w:hyperlink><w:r><w:t> done</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("see "),
                text("link"),
                text(" done"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_styled_run_emits_styled_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:hyperlink><w:r><w:rPr><w:u w:val="single"/></w:rPr><w:t>link</w:t></w:r></w:hyperlink></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Underline,
                    id: None,
                },
                text("link"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn self_closing_hyperlink_emits_nothing() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:hyperlink/></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn orphan_hyperlink_at_body_level_emits_nothing() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:hyperlink><w:r><w:t>x</w:t></w:r></w:hyperlink></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(events, vec![start_doc(), Event::EndDocument,]);
    }

    #[test]
    fn hyperlink_with_valid_rid_emits_start_link_before_text() {
        let document_xml = hyperlink_text_document(r#"r:id="rId1""#);
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(events, expected_link_events("https://example.com", None));
    }

    #[test]
    fn hyperlink_with_strict_uri_type_resolves() {
        let document_xml = hyperlink_text_document(r#"r:id="rId1""#);
        let rels_xml =
            document_hyperlink_rels(STRICT_HYPERLINK_TYPE, "https://example.com", "External");
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(events, expected_link_events("https://example.com", None));
    }

    #[test]
    fn hyperlink_with_transitional_uri_type_resolves() {
        let document_xml = hyperlink_text_document(r#"r:id="rId1""#);
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(events, expected_link_events("https://example.com", None));
    }

    #[test]
    fn hyperlink_anchor_only_emits_hash_href() {
        let document_xml = hyperlink_text_document(r#"w:anchor="section1""#);
        let events = hyperlink_events(&document_xml, None);
        assert_eq!(events, expected_link_events("#section1", None));
    }

    #[test]
    fn hyperlink_rid_supersedes_anchor() {
        let document_xml = hyperlink_text_document(r#"r:id="rId1" w:anchor="section1""#);
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(events, expected_link_events("https://example.com", None));
    }

    #[test]
    fn hyperlink_tooltip_populates_title() {
        let document_xml = hyperlink_text_document(r#"r:id="rId1" w:tooltip="Click here""#);
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            expected_link_events("https://example.com", Some("Click here"))
        );
    }

    #[test]
    fn hyperlink_tooltip_with_xml_entity_is_decoded() {
        let document_xml = hyperlink_text_document(r#"r:id="rId1" w:tooltip="Click &amp; go""#);
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            expected_link_events("https://example.com", Some("Click & go"))
        );
    }

    #[test]
    fn hyperlink_broken_rid_passes_through() {
        let document_xml = hyperlink_text_document(r#"r:id="rId99""#);
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(events, expected_plain_link_text_events());
    }

    #[test]
    fn hyperlink_broken_rid_with_anchor_still_passes_through() {
        let document_xml = hyperlink_text_document(r#"r:id="rId99" w:anchor="section1""#);
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(events, expected_plain_link_text_events());
    }

    #[test]
    fn hyperlink_wrong_type_rid_passes_through() {
        let document_xml = hyperlink_text_document(r#"r:id="rId1""#);
        let rels_xml = document_hyperlink_rels(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles",
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(events, expected_plain_link_text_events());
    }

    #[test]
    fn hyperlink_no_rid_no_anchor_passes_through() {
        let document_xml = hyperlink_text_document("");
        let events = hyperlink_events(&document_xml, None);
        assert_eq!(events, expected_plain_link_text_events());
    }

    #[test]
    fn hyperlink_self_closing_with_valid_rid_emits_nothing() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:hyperlink r:id="rId1"/></w:p></w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument
            ]
        );
    }

    #[test]
    fn hyperlink_no_rels_file_passes_through() {
        let document_xml = hyperlink_text_document(r#"r:id="rId1""#);
        let events = hyperlink_events(&document_xml, None);
        assert_eq!(events, expected_plain_link_text_events());
    }

    #[test]
    fn no_rels_file_with_hyperlinks_passes_through_as_text() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:hyperlink r:id="rId1"><w:r><w:t>fallback</w:t></w:r></w:hyperlink></w:p></w:body>
</w:document>"#;
        let bytes = fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                relationship_root().as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                document_xml.as_bytes(),
            ),
        ]);
        let mut reader = DocxReader::from_reader(Cursor::new(bytes)).expect("from_reader");
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("fallback"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_with_internal_target_mode_emits_target_verbatim() {
        let document_xml = hyperlink_text_document(r#"r:id="rId1""#);
        let rels_xml =
            document_hyperlink_rels(TRANSITIONAL_HYPERLINK_TYPE, "other.docx", "Internal");
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(events, expected_link_events("other.docx", None));
    }

    #[test]
    fn hyperlink_emits_end_link_after_content() {
        let document_xml = document_with_hyperlink(r#"r:id="rId1""#, "<w:r><w:t>x</w:t></w:r>");
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text("x"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_nested_drops_inner_wrapper_emits_content_inline() {
        let document_xml = document_with_hyperlink(
            r#"r:id="rId1""#,
            r#"<w:r><w:t>outer</w:t></w:r><w:hyperlink r:id="rId1"><w:r><w:t>inner</w:t></w:r></w:hyperlink>"#,
        );
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text("outer"),
                text("inner"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_with_styled_run_emits_link_outer_style_inner() {
        let document_xml = document_with_hyperlink(
            r#"r:id="rId1""#,
            "<w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r>",
        );
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                text("bold"),
                Event::EndTextStyle,
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_multi_run_styled_link_outer_styles_inner_each_run() {
        let document_xml = document_with_hyperlink(
            r#"r:id="rId1""#,
            "<w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>italic</w:t></w:r>",
        );
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                text("bold"),
                Event::EndTextStyle,
                Event::StartTextStyle {
                    kind: TextStyleKind::Italic,
                    id: None,
                },
                text("italic"),
                Event::EndTextStyle,
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_empty_emits_nothing_with_valid_rid() {
        let document_xml = document_with_hyperlink(r#"r:id="rId1""#, "");
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument
            ]
        );
    }

    #[test]
    fn hyperlink_with_drawing_inside_still_emits_link_around_text() {
        let document_xml = document_with_hyperlink(
            r#"r:id="rId1""#,
            "<w:r><w:t>before</w:t></w:r><w:drawing><wp:inline/></w:drawing><w:r><w:t>after</w:t></w:r>",
        );
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text("before"),
                text("after"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_paragraph_end_with_open_link_auto_closes() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:hyperlink r:id="rId1"><w:r><w:t>x</w:t></w:r></w:p></w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text("x"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_eof_with_open_link_auto_closes() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:hyperlink r:id="rId1"><w:r><w:t>x</w:t></w:r>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text("x"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_inside_preformatted_paragraph_passes_through() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:pPr><w:pStyle w:val="Source Code"/></w:pPr><w:hyperlink r:id="rId1"><w:r><w:t>code</w:t></w:r></w:hyperlink></w:p></w:body>
</w:document>"#;
        let events = styled_hyperlink_events(
            document_xml,
            r#"<w:style w:type="paragraph" w:styleId="Source Code"><w:name w:val="Source Code"/></w:style>"#,
        );
        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartPreformatted {
                    id: None,
                    syntax: None,
                },
                text("code"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_inside_denied_subtree_emits_nothing() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:drawing><w:hyperlink r:id="rId1"><w:r><w:t>inside-drawing</w:t></w:r></w:hyperlink></w:drawing></w:p></w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument
            ]
        );
    }

    #[test]
    fn hyperlink_inside_heading_emits_link_inside_heading() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:pPr><w:pStyle w:val="Heading 1"/></w:pPr><w:hyperlink r:id="rId1"><w:r><w:t>x</w:t></w:r></w:hyperlink></w:p></w:body>
</w:document>"#;
        let events = styled_hyperlink_events(
            document_xml,
            r#"<w:style w:type="paragraph" w:styleId="Heading 1"><w:name w:val="heading 1"/></w:style>"#,
        );
        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartHeading { level: 1, id: None },
                start_link("https://example.com", None),
                text("x"),
                Event::EndLink,
                Event::EndHeading,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_inside_table_cell_emits_cell_before_link() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:tbl><w:tr><w:tc><w:p><w:hyperlink r:id="rId1"><w:r><w:t>x</w:t></w:r></w:hyperlink></w:p></w:tc></w:tr></w:tbl></w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                start_link("https://example.com", None),
                text("x"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_inside_ins_tracked_insertion_emits_link() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:ins w:id="1" w:author="Author" w:date="2024-01-01T00:00:00Z"><w:hyperlink r:id="rId1"><w:r><w:t>x</w:t></w:r></w:hyperlink></w:ins></w:p></w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text("x"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_inside_sdt_content_emits_link() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:sdt><w:sdtContent><w:hyperlink r:id="rId1"><w:r><w:t>x</w:t></w:r></w:hyperlink></w:sdtContent></w:sdt></w:p></w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text("x"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_with_whitespace_only_content_emits_link_around_whitespace_text() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:hyperlink r:id="rId1"><w:r><w:t xml:space="preserve"> </w:t></w:r></w:hyperlink></w:p></w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text(" "),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_with_tab_content_emits_link_around_tab() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:hyperlink r:id="rId1"><w:r><w:tab/></w:r></w:hyperlink></w:p></w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text("\t"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_with_line_break_content_emits_link_around_break() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:hyperlink r:id="rId1"><w:r><w:br/></w:r></w:hyperlink></w:p></w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                Event::LineBreak,
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_with_multiple_text_fragments_emits_single_link_wrapper() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:hyperlink r:id="rId1"><w:r><w:t>a</w:t><w:t>b</w:t></w:r></w:hyperlink></w:p></w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text("a"),
                text("b"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_with_special_chars_in_anchor_emits_verbatim_fragment() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:hyperlink w:anchor="Section 1.2 §A"><w:r><w:t>x</w:t></w:r></w:hyperlink></w:p></w:body>
</w:document>"#;
        let events = hyperlink_events(document_xml, None);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("#Section 1.2 §A", None),
                text("x"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_with_special_chars_in_rels_target_emits_verbatim() {
        let document_xml = document_with_hyperlink(r#"r:id="rId1""#, "<w:r><w:t>x</w:t></w:r>");
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com?q=1&amp;r=2",
            "External",
        );
        let events = hyperlink_events(&document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com?q=1&r=2", None),
                text("x"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_in_multiple_paragraphs_each_emits_independent_link() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p><w:hyperlink r:id="rId1"><w:r><w:t>a</w:t></w:r></w:hyperlink></w:p>
    <w:p><w:hyperlink r:id="rId1"><w:r><w:t>b</w:t></w:r></w:hyperlink></w:p>
  </w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text("a"),
                Event::EndLink,
                Event::EndParagraph,
                start_para(),
                start_link("https://example.com", None),
                text("b"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn hyperlink_after_unclosed_hyperlink_in_prior_paragraph_emits_correctly() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p><w:hyperlink r:id="rId1"><w:r><w:t>a</w:t></w:r></w:p>
    <w:p><w:hyperlink r:id="rId1"><w:r><w:t>b</w:t></w:r></w:hyperlink></w:p>
  </w:body>
</w:document>"#;
        let rels_xml = document_hyperlink_rels(
            TRANSITIONAL_HYPERLINK_TYPE,
            "https://example.com",
            "External",
        );
        let events = hyperlink_events(document_xml, Some(&rels_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                start_link("https://example.com", None),
                text("a"),
                Event::EndLink,
                Event::EndParagraph,
                start_para(),
                start_link("https://example.com", None),
                text("b"),
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn sdt_with_sdt_content_paragraph_emits_paragraph() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:sdt><w:sdtPr><w:tag w:val="myTag"/><w:id w:val="42"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>SDT content</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("SDT content"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn sdt_without_content_child_emits_nothing() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:sdt><w:sdtPr><w:id w:val="42"/></w:sdtPr></w:sdt></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(events, vec![start_doc(), Event::EndDocument,]);
    }

    #[test]
    fn sdt_end_pr_subtree_dropped() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>text</w:t></w:r><w:sdtEndPr><w:rPr><w:b/></w:rPr></w:sdtEndPr></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn wins_run_level_emits_inserted_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t></w:r><w:ins><w:r><w:t>inserted</w:t></w:r></w:ins><w:r><w:t>b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a"),
                text("inserted"),
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn out_of_order_rpr_inside_wins_is_silently_consumed() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins><w:r><w:t>x</w:t><w:rPr><w:b/></w:rPr></w:r></w:ins></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn block_level_wins_wrapping_paragraph_emits_paragraph() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>existing</w:t></w:r></w:p><w:ins><w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:t>inserted para</w:t></w:r></w:p></w:ins></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("existing"),
                Event::EndParagraph,
                start_para_with_alignment(TextAlignment::Right),
                text("inserted para"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn block_level_wins_wrapping_table_emits_table() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:ins><w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:ins></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("cell"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn move_to_run_level_emits_moved_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t></w:r><w:moveTo><w:r><w:t>moved</w:t></w:r></w:moveTo><w:r><w:t>b</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a"),
                text("moved"),
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn block_level_move_to_wrapping_paragraph_emits_paragraph() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:moveTo><w:p><w:r><w:t>moved</w:t></w:r></w:p></w:moveTo></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("moved"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn sdt_pr_subtree_dropped_inside_paragraph() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>before</w:t></w:r><w:sdtPr><w:tag w:val="x"/><w:id w:val="1"/></w:sdtPr><w:r><w:t>after</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("before"),
                text("after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn drawing_inside_wins_is_dropped() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins><w:r><w:t>before</w:t></w:r><w:drawing><wp:inline w:width="100"/></w:drawing><w:r><w:t>after</w:t></w:r></w:ins></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("before"),
                text("after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn self_closing_drawing_inside_wins_is_dropped() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>kept</w:t></w:r><w:ins><w:drawing/></w:ins></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("kept"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn wins_around_hyperlink_around_run_emits_inner_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins><w:hyperlink><w:r><w:t>x</w:t></w:r></w:hyperlink></w:ins></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn combined_fixture_with_hyperlink_ins_sdt_drawing_emits_expected_events() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>plain </w:t></w:r><w:hyperlink><w:r><w:t>link</w:t></w:r></w:hyperlink><w:r><w:t> </w:t></w:r><w:ins><w:r><w:t>inserted</w:t></w:r></w:ins><w:r><w:t> </w:t></w:r><w:drawing><wp:inline w:width="100"/></w:drawing><w:r><w:t> </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="1"/></w:sdtPr><w:sdtContent><w:r><w:t>sdt</w:t></w:r></w:sdtContent></w:sdt></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("plain "),
                text("link"),
                text(" "),
                text("inserted"),
                text(" "),
                text(" "),
                text("sdt"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn eof_inside_styled_run_auto_closes_styles() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>bold"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                text("bold"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn nested_denied_inside_denied_is_suppressed() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:drawing><w:pict><w:r><w:t>hidden</w:t></w:r></w:pict></w:drawing><w:r><w:t>visible</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("visible"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_self_closing_bold_in_rpr_emits_bold_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:b></w:b></w:rPr><w:t>bold</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                text("bold"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_self_closing_italic_in_rpr_emits_italic_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:i></w:i></w:rPr><w:t>italic</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Italic,
                    id: None,
                },
                text("italic"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_self_closing_strike_in_rpr_emits_strikethrough_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:strike></w:strike></w:rPr><w:t>struck</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Strikethrough,
                    id: None,
                },
                text("struck"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_self_closing_underline_in_rpr_emits_underline_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:u w:val="single"></w:u></w:rPr><w:t>underline</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Underline,
                    id: None,
                },
                text("underline"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_self_closing_vert_align_in_rpr_emits_subscript_text() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:vertAlign w:val="subscript"></w:vertAlign></w:rPr><w:t>sub</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Subscript,
                    id: None,
                },
                text("sub"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_self_closing_jc_in_ppr_sets_alignment() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:jc w:val="center"></w:jc></w:pPr><w:r><w:t>centered</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para_with_alignment(TextAlignment::Center),
                text("centered"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_self_closing_tbl_header_emits_header() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:trPr><w:tblHeader></w:tblHeader></w:trPr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_header(),
                start_para(),
                text("h"),
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_self_closing_gridspan_emits_colspan() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="3"></w:gridSpan></w:tcPr><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                Event::StartTableCell {
                    colspan: Some(3),
                    rowspan: None,
                    id: None,
                },
                start_para(),
                text("cell"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_self_closing_vmerge_is_a_no_op() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:tcPr><w:vMerge w:val="restart"></w:vMerge></w:tcPr><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("cell"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn out_of_order_trpr_after_row_content_is_denied() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc><w:trPr><w:jc w:val="center"/></w:trPr></w:tr></w:tbl></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_para(),
                text("cell"),
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn self_closing_rpr_inside_ppr_is_a_no_op() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:jc w:val="right"/><w:rPr/></w:pPr><w:r><w:t>text</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para_with_alignment(TextAlignment::Right),
                text("text"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }
}

mod happy_path_lists {
    use docspec_core::{Event, ListStyleType};

    use crate::fixture;

    use super::collect_events;

    fn root_rels() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#
    }

    fn doc_rels_with_numbering() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
</Relationships>"#
    }

    fn build_docx(document_xml: &str, numbering_xml: &str) -> Vec<u8> {
        fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                root_rels().as_bytes(),
            ),
            (
                "word/_rels/document.xml.rels",
                zip::CompressionMethod::Deflated,
                doc_rels_with_numbering().as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                document_xml.as_bytes(),
            ),
            (
                "word/numbering.xml",
                zip::CompressionMethod::Deflated,
                numbering_xml.as_bytes(),
            ),
        ])
    }

    fn start_doc() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    fn start_para() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    fn text(content: &str) -> Event {
        Event::Text {
            content: content.to_string(),
        }
    }

    #[test]
    fn flat_ordered_two_items_emits_correct_sequence() {
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>First item</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>Second item</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("First item"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("Second item"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn flat_unordered_two_items_emits_correct_sequence() {
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="2">
    <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="2"><w:abstractNumId w:val="2"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="2"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>First item</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="2"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>Second item</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartUnorderedListItem {
                    id: Some("2".to_string()),
                    level: 0,
                    style_type: ListStyleType::Disc,
                },
                start_para(),
                text("First item"),
                Event::EndParagraph,
                Event::EndUnorderedListItem,
                Event::StartUnorderedListItem {
                    id: Some("2".to_string()),
                    level: 0,
                    style_type: ListStyleType::Disc,
                },
                start_para(),
                text("Second item"),
                Event::EndParagraph,
                Event::EndUnorderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn nested_ordered_parent_child_emits_correct_sequence() {
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>Parent</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Child</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("Parent"),
                Event::EndParagraph,
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 1,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("Child"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn mixed_format_ordered_then_unordered_same_num_id_per_level() {
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="3">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="3"><w:abstractNumId w:val="3"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="3"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>Ordered parent</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="3"/><w:ilvl w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Unordered child</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("3".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("Ordered parent"),
                Event::EndParagraph,
                Event::StartUnorderedListItem {
                    id: Some("3".to_string()),
                    level: 1,
                    style_type: ListStyleType::Disc,
                },
                start_para(),
                text("Unordered child"),
                Event::EndParagraph,
                Event::EndUnorderedListItem,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn lower_letter_emits_lower_alpha_style_type() {
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="5">
    <w:lvl w:ilvl="0"><w:numFmt w:val="lowerLetter"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="5"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>Item</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::LowerAlpha,
                },
                start_para(),
                text("Item"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn lower_roman_emits_lower_roman_style_type() {
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="6">
    <w:lvl w:ilvl="0"><w:numFmt w:val="lowerRoman"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="6"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>Item</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::LowerRoman,
                },
                start_para(),
                text("Item"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn upper_letter_emits_upper_alpha_style_type() {
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="7">
    <w:lvl w:ilvl="0"><w:numFmt w:val="upperLetter"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="7"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>Item</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::UpperAlpha,
                },
                start_para(),
                text("Item"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn upper_roman_emits_upper_roman_style_type() {
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="8">
    <w:lvl w:ilvl="0"><w:numFmt w:val="upperRoman"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="8"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>Item</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::UpperRoman,
                },
                start_para(),
                text("Item"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }
}

mod spec_edge_cases {
    use docspec_core::{Event, ListStyleType};

    use crate::fixture;

    use super::collect_events;

    fn root_rels() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#
    }

    fn doc_rels_with_numbering() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
</Relationships>"#
    }

    fn build_docx(document_xml: &str, numbering_xml: &str) -> Vec<u8> {
        fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                root_rels().as_bytes(),
            ),
            (
                "word/_rels/document.xml.rels",
                zip::CompressionMethod::Deflated,
                doc_rels_with_numbering().as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                document_xml.as_bytes(),
            ),
            (
                "word/numbering.xml",
                zip::CompressionMethod::Deflated,
                numbering_xml.as_bytes(),
            ),
        ])
    }

    fn start_doc() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    fn start_para() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    fn text(content: &str) -> Event {
        Event::Text {
            content: content.to_string(),
        }
    }

    #[test]
    fn num_id_zero_sentinel_emits_plain_paragraph() {
        // §17.9.18: numId=0 sentinel escapes list membership
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="0"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn missing_num_fmt_defaults_to_decimal() {
        // §17.9.17: missing numFmt defaults to decimal
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0">
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn missing_ilvl_defaults_to_zero() {
        // §17.9.3: missing ilvl defaults to 0
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn num_fmt_none_emits_plain_paragraph() {
        // §17.9.17: numFmt=none emits plain paragraph, not a list item
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="none"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn multi_level_type_ignored_for_classification() {
        // §17.9.12: multiLevelType is a UI hint, not authoritative
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:multiLevelType w:val="singleLevel"/>
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>ilvl0</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>ilvl1</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("ilvl0"),
                Event::EndParagraph,
                Event::StartUnorderedListItem {
                    id: Some("1".to_string()),
                    level: 1,
                    style_type: ListStyleType::Disc,
                },
                start_para(),
                text("ilvl1"),
                Event::EndParagraph,
                Event::EndUnorderedListItem,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn numbering_change_in_numpr_ignored() {
        // <w:numberingChange> is a track-changes element recording previous numbering state;
        // it MUST NOT block the numId/ilvl capture
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/><w:numberingChange w:id="1" w:originalNumId="2" w:originalIlvl="0"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, numbering_xml));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn first_list_item_at_deep_level_keeps_start_marker_on_real_item() {
        // Regression: when the first authored list item appears at a non-zero ilvl,
        // reconcile_list_stack synthesizes phantom levels 0..ilvl-1 to keep the event
        // stream well-formed. Phantoms must NOT consume the per-numId `start: Some(1)`
        // marker — it belongs on the user-authored item at the target ilvl.
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="3"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="3"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let events = collect_events(build_docx(document_xml, numbering_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 1,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 2,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 3,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }
}

mod resilience_tests {
    use docspec_core::{Event, ListStyleType};

    use crate::fixture;

    use super::collect_events;

    fn root_rels() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#
    }

    fn doc_rels_with_numbering() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
</Relationships>"#
    }

    fn doc_rels_no_numbering() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#
    }

    fn build_docx(document_xml: &str, numbering_xml: &str) -> Vec<u8> {
        fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                root_rels().as_bytes(),
            ),
            (
                "word/_rels/document.xml.rels",
                zip::CompressionMethod::Deflated,
                doc_rels_with_numbering().as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                document_xml.as_bytes(),
            ),
            (
                "word/numbering.xml",
                zip::CompressionMethod::Deflated,
                numbering_xml.as_bytes(),
            ),
        ])
    }

    fn start_doc() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    fn start_para() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    fn text(content: &str) -> Event {
        Event::Text {
            content: content.to_string(),
        }
    }

    #[test]
    fn missing_numbering_xml_emits_plain_paragraphs() {
        // No word/numbering.xml and no word/_rels/document.xml.rels — paragraphs
        // with <w:numPr> must emit StartParagraph (graceful degradation, no error).
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let bytes = fixture::synth_docx(root_rels(), document_xml);
        let events = collect_events(bytes);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn numbering_rel_missing_target_emits_plain_paragraphs() {
        // word/_rels/document.xml.rels present but contains no numbering relationship.
        // Paragraphs with <w:numPr> must emit StartParagraph (no list events, no error).
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let bytes = fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                root_rels().as_bytes(),
            ),
            (
                "word/_rels/document.xml.rels",
                zip::CompressionMethod::Deflated,
                doc_rels_no_numbering().as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                document_xml.as_bytes(),
            ),
        ]);
        let events = collect_events(bytes);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn empty_numbering_xml_emits_plain_paragraphs() {
        // Empty <w:numbering/> — no abstractNums, no nums defined.
        // Paragraphs with <w:numPr> must emit StartParagraph (no list events, no error).
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let numbering_xml = r#"<?xml version="1.0"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#;
        let events = collect_events(build_docx(document_xml, numbering_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn malformed_numbering_xml_returns_err() {
        // An unclosed XML tag leaves element_depth > 0 at EOF, causing parse_numbering
        // to return Err — which propagates as Err from DocxReader::from_reader.
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let bytes = fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                root_rels().as_bytes(),
            ),
            (
                "word/_rels/document.xml.rels",
                zip::CompressionMethod::Deflated,
                doc_rels_with_numbering().as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                document_xml.as_bytes(),
            ),
            (
                "word/numbering.xml",
                zip::CompressionMethod::Deflated,
                b"<unclosed>",
            ),
        ]);
        let result = docspec_docx_reader::DocxReader::from_reader(std::io::Cursor::new(bytes));
        assert!(result.is_err());
    }

    #[test]
    fn unknown_num_id_emits_plain_paragraph() {
        // Paragraph references numId=999 which is not defined in numbering.xml.
        // MinimalNumbering.resolve() graceful path: emit StartParagraph (no list events).
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="999"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let events = collect_events(build_docx(document_xml, numbering_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn ilvl_overflow_clamps_to_eight() {
        // Paragraph uses <w:ilvl w:val="99"/> — ilvl is clamped to min(99, 8) = 8.
        // reconcile_list_stack fills phantom levels 0-7 with start: None before reaching
        // the target level 8, which receives the `start: Some(1)` per-numId marker.
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="8"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="99"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let events = collect_events(build_docx(document_xml, numbering_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 1,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 2,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 3,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 4,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 5,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 6,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 7,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 8,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_numeric_num_id_emits_plain_paragraph() {
        // <w:numId w:val="abc"/> — parse failure leaves pending_num_pr_id = None.
        // Without a numId, the paragraph emits StartParagraph (no list events).
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="abc"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let events = collect_events(build_docx(document_xml, numbering_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_numeric_ilvl_defaults_to_zero() {
        // <w:ilvl w:val="xyz"/> — parse failure; ilvl defaults to 0 per §17.9.3.
        // numbering.xml defines numId=1 ilvl=0 as decimal, so level 0 is emitted.
        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="xyz"/></w:numPr></w:pPr>
      <w:r><w:t>text</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let events = collect_events(build_docx(document_xml, numbering_xml));
        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                start_para(),
                text("text"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }
}

mod cross_feature_lists {
    use docspec_core::{Event, ListStyleType, TableHeaderScope, TextStyleKind};

    use crate::fixture;

    use super::collect_events;

    fn root_rels() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#
    }

    fn doc_rels_with_numbering() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
</Relationships>"#
    }

    fn doc_rels_with_numbering_and_styles() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#
    }

    fn build_docx(document_xml: &str, numbering_xml: &str) -> Vec<u8> {
        fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                root_rels().as_bytes(),
            ),
            (
                "word/_rels/document.xml.rels",
                zip::CompressionMethod::Deflated,
                doc_rels_with_numbering().as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                document_xml.as_bytes(),
            ),
            (
                "word/numbering.xml",
                zip::CompressionMethod::Deflated,
                numbering_xml.as_bytes(),
            ),
        ])
    }

    fn build_docx_with_styles(
        document_xml: &str,
        numbering_xml: &str,
        styles_xml: &str,
    ) -> Vec<u8> {
        fixture::synth_docx_with_entries(&[
            (
                "_rels/.rels",
                zip::CompressionMethod::Deflated,
                root_rels().as_bytes(),
            ),
            (
                "word/_rels/document.xml.rels",
                zip::CompressionMethod::Deflated,
                doc_rels_with_numbering_and_styles().as_bytes(),
            ),
            (
                "word/document.xml",
                zip::CompressionMethod::Deflated,
                document_xml.as_bytes(),
            ),
            (
                "word/numbering.xml",
                zip::CompressionMethod::Deflated,
                numbering_xml.as_bytes(),
            ),
            (
                "word/styles.xml",
                zip::CompressionMethod::Deflated,
                styles_xml.as_bytes(),
            ),
        ])
    }

    fn decimal_numbering_xml() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#
    }

    fn two_decimal_lists_numbering_xml() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
  <w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#
    }

    fn start_doc() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    fn start_para() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    fn text(content: &str) -> Event {
        Event::Text {
            content: content.to_string(),
        }
    }

    fn start_table() -> Event {
        Event::StartTable { id: None }
    }

    fn start_row() -> Event {
        Event::StartTableRow { id: None }
    }

    fn start_cell() -> Event {
        Event::StartTableCell {
            colspan: None,
            id: None,
            rowspan: None,
        }
    }

    fn start_header() -> Event {
        Event::StartTableHeader {
            scope: Some(TableHeaderScope::Column),
            abbr: None,
            colspan: None,
            rowspan: None,
            id: None,
        }
    }

    fn start_ordered(id: &str, start: Option<u64>) -> Event {
        Event::StartOrderedListItem {
            id: Some(id.to_string()),
            level: 0,
            start,
            style_type: ListStyleType::Decimal,
        }
    }

    #[test]
    fn list_inside_table_cell_nests_correctly() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:tbl><w:tr><w:tc><w:p><w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>list item</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, decimal_numbering_xml()));

        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_ordered("1", Some(1)),
                start_para(),
                text("list item"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn list_inside_table_header_cell_nests_correctly() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>list item</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, decimal_numbering_xml()));

        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_header(),
                start_ordered("1", Some(1)),
                start_para(),
                text("list item"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn styled_text_in_list_item_closes_before_end() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r></w:p></w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, decimal_numbering_xml()));

        assert_eq!(
            events,
            vec![
                start_doc(),
                start_ordered("1", Some(1)),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                text("bold"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn list_paragraph_with_heading_pstyle_emits_heading_not_list() {
        let styles_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
  </w:style>
</w:styles>"#;
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>Heading</w:t></w:r></w:p></w:body>
</w:document>"#;

        let events = collect_events(build_docx_with_styles(
            document_xml,
            decimal_numbering_xml(),
            styles_xml,
        ));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartHeading { level: 1, id: None },
                text("Heading"),
                Event::EndHeading,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn list_open_at_document_end_flushes_correctly() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>item</w:t></w:r></w:p></w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, decimal_numbering_xml()));

        assert_eq!(
            events,
            vec![
                start_doc(),
                start_ordered("1", Some(1)),
                start_para(),
                text("item"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn two_consecutive_lists_each_first_item_emits_start_some_one() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>one</w:t></w:r></w:p>
    <w:p><w:pPr><w:numPr><w:numId w:val="2"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>two</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, two_decimal_lists_numbering_xml()));

        assert_eq!(
            events,
            vec![
                start_doc(),
                start_ordered("1", Some(1)),
                start_para(),
                text("one"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                start_ordered("2", Some(1)),
                start_para(),
                text("two"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn same_num_id_continuation_paragraph_attaches_and_second_item_keeps_start_none() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>one</w:t></w:r></w:p>
    <w:p><w:r><w:t>break</w:t></w:r></w:p>
    <w:p><w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>two</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, decimal_numbering_xml()));

        assert_eq!(
            events,
            vec![
                start_doc(),
                start_ordered("1", Some(1)),
                start_para(),
                text("one"),
                Event::EndParagraph,
                start_para(),
                text("break"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                start_ordered("1", None),
                start_para(),
                text("two"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn list_inside_ins_tracked_change_emits_normally() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:pPr><w:ins w:id="1" w:author="Author" w:date="2024-01-01T00:00:00Z"><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:ins></w:pPr><w:r><w:t>inserted</w:t></w:r></w:p></w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, decimal_numbering_xml()));

        assert_eq!(
            events,
            vec![
                start_doc(),
                start_ordered("1", Some(1)),
                start_para(),
                text("inserted"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn list_inside_block_quote_emits_block_quote_not_list() {
        let styles_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="BlockQuote">
    <w:name w:val="Block Text"/>
  </w:style>
</w:styles>"#;
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:pPr><w:pStyle w:val="BlockQuote"/><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>quote</w:t></w:r></w:p></w:body>
</w:document>"#;

        let events = collect_events(build_docx_with_styles(
            document_xml,
            decimal_numbering_xml(),
            styles_xml,
        ));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartBlockQuote { id: None },
                text("quote"),
                Event::EndBlockQuote,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn list_inside_preformatted_emits_preformatted_not_list() {
        let styles_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="SourceCode">
    <w:name w:val="Source Code"/>
  </w:style>
</w:styles>"#;
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:pPr><w:pStyle w:val="SourceCode"/><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>code</w:t></w:r></w:p></w:body>
</w:document>"#;

        let events = collect_events(build_docx_with_styles(
            document_xml,
            decimal_numbering_xml(),
            styles_xml,
        ));

        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartPreformatted {
                    id: None,
                    syntax: None,
                },
                text("code"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn list_inside_nested_table_emits_no_table_header() {
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:tbl><w:tr><w:tc><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>nested list</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:body>
</w:document>"#;

        let events = collect_events(build_docx(document_xml, decimal_numbering_xml()));

        assert_eq!(
            events,
            vec![
                start_doc(),
                start_table(),
                start_row(),
                start_cell(),
                start_table(),
                start_row(),
                start_cell(),
                start_ordered("1", Some(1)),
                start_para(),
                text("nested list"),
                Event::EndParagraph,
                Event::EndOrderedListItem,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }
}
