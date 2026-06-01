//! Integration tests against real DOCX fixture files.
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[cfg(test)]
mod tests {
    use docspec_core::{Event, EventSource as _, TextStyle};
    use docspec_docx_reader::DocxReader;
    use std::io::Cursor;

    fn drain<R: std::io::Read + std::io::Seek>(mut r: DocxReader<R>) -> Vec<Event> {
        let mut out = Vec::new();
        while let Some(e) = r.next_event().expect("event error") {
            out.push(e);
        }
        out
    }

    fn t(content: &str) -> Event {
        Event::Text {
            content: content.to_string(),
            style: TextStyle::default(),
        }
    }

    fn sp() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    fn sd() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    #[test]
    fn plain_text_full_sequence_exact() {
        let bytes = include_bytes!("fixtures/plain-text.docx");
        let r = DocxReader::new(Cursor::new(&bytes[..])).expect("new");
        let events = drain(r);
        assert_eq!(
            events,
            vec![
                sd(),
                sp(),
                t("Here is some code in the Plain Text Style (Microsoft Word for Mac 16.72)"),
                Event::EndParagraph,
                sp(),
                Event::EndParagraph,
                sp(),
                t("<div class=\"foo\">"),
                Event::EndParagraph,
                sp(),
                t("  <p>Paragraph in HTML</p>"),
                Event::EndParagraph,
                sp(),
                t("</div>"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn alternate_document_path_main_part_is_not_word_document_xml() {
        let bytes = include_bytes!("fixtures/alternate_document_path.docx");
        let r = DocxReader::new(Cursor::new(&bytes[..])).expect("new");
        assert_ne!(r.main_part_path(), "word/document.xml");
        assert_eq!(r.main_part_path(), "word/document2.xml");
    }

    #[test]
    fn alternate_document_path_full_sequence_exact() {
        let bytes = include_bytes!("fixtures/alternate_document_path.docx");
        let r = DocxReader::new(Cursor::new(&bytes[..])).expect("new");
        let events = drain(r);
        assert_eq!(
            events,
            vec![
                sd(),
                sp(),
                t("Test"),
                Event::EndParagraph,
                sp(),
                Event::EndParagraph,
                sp(),
                t("This is "),
                t("italic"),
                t(", "),
                t("bold"),
                t(", "),
                t("underlined"),
                t(", "),
                t("italic underlined"),
                t(", "),
                t("bold underlined"),
                t(", "),
                t("bold italic underlined"),
                t("."),
                Event::EndParagraph,
                sp(),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn i18n_blocks_unicode_text_round_trip() {
        let bytes = include_bytes!("fixtures/i18n_blocks.docx");
        let r = DocxReader::new(Cursor::new(&bytes[..])).expect("new");
        let events = drain(r);
        let joined: String = events
            .iter()
            .filter_map(|e| {
                if let Event::Text { content, .. } = e {
                    Some(content.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
        joined,
        "This is Heading 1This is Heading 2This is QuoteThis is Block TextThis is list item 1This is list item 2"
    );
    }

    #[test]
    fn i18n_blocks_full_sequence_exact() {
        let bytes = include_bytes!("fixtures/i18n_blocks.docx");
        let r = DocxReader::new(Cursor::new(&bytes[..])).expect("new");
        let events = drain(r);
        assert_eq!(
            events,
            vec![
                sd(),
                sp(),
                t("This is Heading 1"),
                Event::EndParagraph,
                sp(),
                t("This is Heading 2"),
                Event::EndParagraph,
                sp(),
                t("This is Quote"),
                Event::EndParagraph,
                sp(),
                t("This is Block Text"),
                Event::EndParagraph,
                sp(),
                t("This is list item 1"),
                Event::EndParagraph,
                sp(),
                t("This is list item 2"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn headers_all_emit_paragraphs_not_headings() {
        let bytes = include_bytes!("fixtures/headers.docx");
        let r = DocxReader::new(Cursor::new(&bytes[..])).expect("new");
        let events = drain(r);
        assert!(!events
            .iter()
            .any(|e| matches!(e, Event::StartHeading { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartParagraph { .. })));
    }

    #[test]
    fn headers_full_sequence_exact() {
        let bytes = include_bytes!("fixtures/headers.docx");
        let r = DocxReader::new(Cursor::new(&bytes[..])).expect("new");
        let events = drain(r);
        assert_eq!(
            events,
            vec![
                sd(),
                sp(),
                t("A Test of Headers"),
                Event::EndParagraph,
                sp(),
                t("Second Level"),
                Event::EndParagraph,
                sp(),
                t("Some plain text."),
                Event::EndParagraph,
                sp(),
                t("Third level"),
                Event::EndParagraph,
                sp(),
                t("Some more plain text."),
                Event::EndParagraph,
                sp(),
                t("Fourth level"),
                Event::EndParagraph,
                sp(),
                t("Some more plain text."),
                Event::EndParagraph,
                sp(),
                t("Fifth level"),
                Event::EndParagraph,
                sp(),
                t("Some more plain text."),
                Event::EndParagraph,
                sp(),
                t("Sixth level"),
                Event::EndParagraph,
                sp(),
                t("Some more plain text."),
                Event::EndParagraph,
                sp(),
                t("Seventh level"),
                Event::EndParagraph,
                sp(),
                t("Since no Heading 7 style exists in styles.xml, this gets converted to Span."),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn html_sample_skips_tables_links_images() {
        let bytes = include_bytes!("fixtures/html-sample.docx");
        let r = DocxReader::new(Cursor::new(&bytes[..])).expect("new");
        let events = drain(r);
        assert!(!events.iter().any(|e| matches!(e, Event::StartTable { .. })));
        assert!(!events.iter().any(|e| matches!(e, Event::StartLink { .. })));
        assert!(!events.iter().any(|e| matches!(e, Event::Image { .. })));
    }

    #[test]
    fn html_sample_full_sequence_exact() {
        let bytes = include_bytes!("fixtures/html-sample.docx");
        let r = DocxReader::new(Cursor::new(&bytes[..])).expect("new");
        let events = drain(r);
        assert_eq!(
            events,
            vec![
                sd(),
                sp(),
                t("Here is some code in the HTML Sample Style (Microsoft Word for Mac 16.72)"),
                Event::EndParagraph,
                sp(),
                Event::EndParagraph,
                sp(),
                t("<div class=\"foo\">"),
                Event::EndParagraph,
                sp(),
                t("  <p>Paragraph in HTML</p>"),
                Event::EndParagraph,
                sp(),
                t("</div>"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn codeblock_skips_preformatted() {
        let bytes = include_bytes!("fixtures/codeblock.docx");
        let r = DocxReader::new(Cursor::new(&bytes[..])).expect("new");
        let events = drain(r);
        assert!(!events
            .iter()
            .any(|e| matches!(e, Event::StartPreformatted { .. })));
    }

    #[test]
    fn codeblock_full_sequence_exact() {
        let bytes = include_bytes!("fixtures/codeblock.docx");
        let r = DocxReader::new(Cursor::new(&bytes[..])).expect("new");
        let events = drain(r);
        assert_eq!(
            events,
            vec![
                sd(),
                sp(),
                t("This is some code:"),
                Event::EndParagraph,
                sp(),
                t("readDocx :: ReaderOptions"),
                Event::LineBreak,
                t("         -> B.ByteString"),
                Event::LineBreak,
                t("         -> Pandoc"),
                Event::EndParagraph,
                sp(),
                t("from the beginning of the docx reader."),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }
}
