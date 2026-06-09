//! Integration tests for `DocxReader`.
#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
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

    use docspec_core::{Event, TextAlignment, TextStyleKind};
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

    mod rpr {
        use super::*;

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
    }

    mod ppr {
        use super::*;

        fn collect_events(content: &str) -> Vec<Event> {
            let document_xml = format!(
                r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{content}</w:body></w:document>"#,
            );
            let mut reader = make_reader(&document_xml);
            drive(&mut reader)
        }

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
            "DocxReader { buf: [], in_ignored_subtree: 0, in_paragraph: false, in_text: false, in_ppr: false, pending_paragraph_alignment: None, paragraph_started_emitted: false, in_rpr: false, pending_run_kinds: [], pending_text: \"\", frozen_run_kinds: [], open_styles: [], phase: \"<phase>\", queue: [], run_content_emitted: false, xml: \"<quick_xml::Reader>\" }"
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
    fn self_closing_ignored_container_emits_no_events() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:hyperlink/></w:body></w:document>"#,
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
    fn wins_subtree_suppressed_inside_paragraph() {
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
    fn paragraph_containing_only_ins_emits_empty_paragraph() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins><w:r><w:t>x</w:t></w:r></w:ins></w:p></w:body></w:document>"#,
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
    fn w_tab_inside_ignored_subtree_is_silently_dropped() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>kept</w:t></w:r><w:ins><w:r><w:tab/></w:r></w:ins></w:p></w:body></w:document>"#,
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
    fn w_br_inside_ignored_subtree_is_silently_dropped() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>kept</w:t></w:r><w:ins><w:r><w:br/></w:r></w:ins></w:p></w:body></w:document>"#,
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
    fn table_inside_ignored_subtree_is_dropped() {
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
                start_para(),
                text("after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn table_row_and_cell_properties_emit_no_events() {
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
    fn hyperlink_inside_cell_is_still_dropped() {
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
    fn rpr_with_unknown_child_is_no_op() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:color w:val="FF0000"/><w:b/></w:rPr><w:t>x</w:t></w:r></w:p></w:body></w:document>"#,
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
}
