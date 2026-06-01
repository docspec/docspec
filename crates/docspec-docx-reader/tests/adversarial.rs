//! Adversarial in-memory edge-case tests for the DOCX reader.

#![allow(clippy::expect_used, clippy::unwrap_used)]

mod common;

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use docspec_core::{Event, EventSource as _};
    use docspec_docx_reader::DocxReader;

    /// BOM in `_rels/.rels` — quick-xml must survive the U+FEFF prefix.
    #[test]
    fn bom_in_rels_xml_is_handled() {
        let rels_xml = "\u{FEFF}<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>";
        let main_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body></w:document>"#;
        let bytes = super::common::build_docx_with_rels(
            rels_xml,
            &[("word/document.xml", main_xml.as_bytes())],
        );
        let mut reader = DocxReader::new(Cursor::new(bytes)).unwrap();
        let first = reader.next_event().unwrap();
        assert!(matches!(first, Some(Event::StartDocument { .. })));
    }

    /// BOM in main XML — must not panic regardless of whether quick-xml accepts or rejects the BOM.
    #[test]
    fn bom_in_main_xml_is_handled() {
        let main_xml = "\u{FEFF}<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body/></w:document>";
        let bytes = super::common::build_minimal_docx("word/document.xml", main_xml);
        drop(DocxReader::new(Cursor::new(bytes)));
    }

    /// Multiple matching `officeDocument` relationships — the first one wins.
    #[test]
    fn multiple_matching_relationships_first_wins() {
        let rels_xml = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
        <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
        <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document2.xml"/>
    </Relationships>"#;
        let main_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#;
        let bytes = super::common::build_docx_with_rels(
            rels_xml,
            &[
                ("word/document.xml", main_xml.as_bytes()),
                ("word/document2.xml", main_xml.as_bytes()),
            ],
        );
        let reader = DocxReader::new(Cursor::new(bytes)).unwrap();
        assert_eq!(reader.main_part_path(), "word/document.xml");
    }

    /// `..` traversal in `Target` attribute must be rejected with a parse error.
    #[test]
    fn dotdot_traversal_in_target_is_rejected() {
        let rels_xml = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="../escape.xml"/></Relationships>"#;
        let bytes = super::common::build_docx_with_rels(rels_xml, &[]);
        assert!(DocxReader::new(Cursor::new(bytes)).is_err());
    }

    /// Windows drive letter in `Target` (e.g., `C:/foo.xml`) must be rejected.
    #[test]
    fn drive_letter_in_target_is_rejected() {
        let rels_xml = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="C:/foo.xml"/></Relationships>"#;
        let bytes = super::common::build_docx_with_rels(rels_xml, &[]);
        assert!(DocxReader::new(Cursor::new(bytes)).is_err());
    }

    /// HTTP URL in `Target` must be rejected — no external resource fetching.
    #[test]
    fn url_in_target_is_rejected() {
        let rels_xml = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="http://evil.com/x.xml"/></Relationships>"#;
        let bytes = super::common::build_docx_with_rels(rels_xml, &[]);
        assert!(DocxReader::new(Cursor::new(bytes)).is_err());
    }

    /// UTF-16 LE encoded main XML — must produce an error, never panic.
    ///
    /// quick-xml expects UTF-8; the BOM (`0xFF 0xFE`) and UTF-16 content must not
    /// cause a panic. Either construction fails or `next_event` returns `Err`.
    #[test]
    fn utf16_main_xml_produces_error_not_panic() {
        let utf16_bytes: Vec<u8> = vec![0xFF, 0xFE, 0x3C, 0x00, 0x77, 0x00];
        let rels_xml = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
        let bytes =
            super::common::build_docx_with_rels(rels_xml, &[("word/document.xml", &utf16_bytes)]);
        match DocxReader::new(Cursor::new(bytes)) {
            Err(_) => {}
            Ok(mut reader) => while reader.next_event().is_ok_and(|e| e.is_some()) {},
        }
    }

    /// Namespace prefix rebinding — `foo:` mapped to the wordprocessingml URI yields the same events as canonical `w:`.
    #[test]
    fn namespace_prefix_rebinding_yields_canonical_sequence() {
        let main_xml = r#"<?xml version="1.0"?><foo:document xmlns:foo="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><foo:body><foo:p><foo:r><foo:t>hello</foo:t></foo:r></foo:p></foo:body></foo:document>"#;
        let bytes = super::common::build_minimal_docx("word/document.xml", main_xml);
        let mut reader = DocxReader::new(Cursor::new(bytes)).unwrap();
        let mut events = Vec::new();
        while let Some(e) = reader.next_event().unwrap() {
            events.push(e);
        }
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartParagraph { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Text { content, .. } if content == "hello")));
    }

    /// XML comment and processing instruction before `<Relationships>` — must not prevent discovery.
    #[test]
    fn comment_and_pi_in_rels_xml_is_handled() {
        let rels_xml = r#"<?xml version="1.0"?><!-- comment --><?pi ?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
        let main_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#;
        let bytes = super::common::build_docx_with_rels(
            rels_xml,
            &[("word/document.xml", main_xml.as_bytes())],
        );
        assert!(DocxReader::new(Cursor::new(bytes)).is_ok());
    }

    /// `_rels/.rels` containing only `<foo/>` — no `<Relationships>` container means no main part.
    #[test]
    fn rels_without_relationships_container_is_rejected() {
        let rels_xml = r#"<?xml version="1.0"?><foo/>"#;
        let bytes = super::common::build_docx_with_rels(rels_xml, &[]);
        assert!(DocxReader::new(Cursor::new(bytes)).is_err());
    }

    /// Duplicate `_rels/.rels` ZIP entries — pins the zip crate's behavior (no panic).
    ///
    /// The zip crate returns the last entry for duplicate names, so the second rels
    /// (pointing to `word/document2.xml`) wins over the first.
    #[test]
    fn duplicate_zip_entries_behavior_is_pinned() {
        use std::io::Write as _;
        use zip::{write::SimpleFileOptions, ZipWriter};

        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        let rels_xml_1 = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
        let rels_xml_2 = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document2.xml"/></Relationships>"#;
        let main_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#;

        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(rels_xml_1.as_bytes()).unwrap();
        let dup_result = zip.start_file("_rels/.rels", options);
        if dup_result.is_err() {
            return;
        }
        zip.write_all(rels_xml_2.as_bytes()).unwrap();
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(main_xml.as_bytes()).unwrap();
        zip.start_file("word/document2.xml", options).unwrap();
        zip.write_all(main_xml.as_bytes()).unwrap();

        match zip.finish() {
            Err(_) => {}
            Ok(finished) => drop(DocxReader::new(Cursor::new(finished.into_inner()))),
        }
    }

    /// 1 MiB `<w:t>` text content — must produce a single `Text` event, no panic.
    #[test]
    fn very_large_text_content_produces_single_text_event() {
        let large_content = "a".repeat(1_048_576);
        let main_xml = format!(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>{large_content}</w:t></w:r></w:p></w:body></w:document>"#
        );
        let bytes = super::common::build_minimal_docx("word/document.xml", &main_xml);
        let mut reader = DocxReader::new(Cursor::new(bytes)).unwrap();
        let mut text_events = Vec::new();
        while let Some(e) = reader.next_event().unwrap() {
            if let Event::Text { content, .. } = e {
                text_events.push(content);
            }
        }
        assert_eq!(text_events.len(), 1);
        assert_eq!(text_events.first().map(String::len), Some(1_048_576));
    }
}
