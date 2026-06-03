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
