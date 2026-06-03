//! Integration tests for `DocxReader`.
#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
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

    use docspec_core::Error;
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
                assert!(
                    message.starts_with("unsupported compression"),
                    "message was: {message}"
                );
                assert!(message.contains("Bzip2"), "message was: {message}");
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
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

    use docspec_core::{Event, TextStyle};
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

    fn text(content: &str) -> Event {
        Event::Text {
            content: content.to_string(),
            style: TextStyle::default(),
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
    fn table_cells_do_not_emit_paragraphs() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
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
                assert!(
                    message.starts_with("malformed document.xml"),
                    "message was: {message}"
                );
                assert_eq!(position, None);
            }
            other => panic!("expected Error::Parse, got: {other:?}"),
        }
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
    fn w_br_and_w_tab_silently_ignored() {
        let mut reader = make_reader(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t><w:br/><w:t>b</w:t><w:tab/><w:t>c</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let events = drive(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                text("a"),
                text("b"),
                text("c"),
                Event::EndParagraph,
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
}
