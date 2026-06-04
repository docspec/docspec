//! Integration tests for the enum-dispatch reader factory.

#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::unwrap_used
)]

#[cfg(test)]
mod tests {
    #[cfg(any(feature = "markdown", feature = "html", feature = "docx"))]
    use std::io::Cursor;

    #[cfg(any(feature = "markdown", feature = "html", feature = "docx"))]
    use docspec::{AnyReader, Event, EventSource, InputFormat, TextStyle};

    #[cfg(any(feature = "markdown", feature = "html", feature = "docx"))]
    fn collect_events(reader: &mut AnyReader) -> docspec::Result<Vec<Event>> {
        let mut events = Vec::new();
        while let Some(event) = reader.next_event()? {
            events.push(event);
        }
        Ok(events)
    }

    #[cfg(any(feature = "markdown", feature = "html", feature = "docx"))]
    fn cursor(input: &str) -> Cursor<Vec<u8>> {
        Cursor::new(input.as_bytes().to_vec())
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn markdown_dispatch_emits_first_event() {
        use docspec_markdown_reader::MarkdownReader;

        let mut reader = AnyReader::from_reader(InputFormat::Markdown, cursor("# h"))
            .expect("AnyReader should construct");
        let event = reader.next_event().expect("AnyReader should not fail");
        let expected = MarkdownReader::new("# h")
            .next_event()
            .expect("direct reader should not fail");
        assert_eq!(event, expected);
    }

    #[cfg(feature = "html")]
    #[test]
    fn html_dispatch_emits_first_event() {
        use docspec_html_reader::HtmlReader;

        let mut reader = AnyReader::from_reader(InputFormat::Html, cursor("<p>x</p>"))
            .expect("AnyReader should construct");
        let event = reader.next_event().expect("AnyReader should not fail");
        let expected = HtmlReader::new("<p>x</p>")
            .next_event()
            .expect("direct reader should not fail");
        assert_eq!(event, expected);
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn roundtrip_full_document_markdown() {
        use docspec_markdown_reader::MarkdownReader;

        let input = "# Hello\n\nWorld";
        let mut any_reader = AnyReader::from_reader(InputFormat::Markdown, cursor(input))
            .expect("AnyReader should construct");
        let mut direct_reader = MarkdownReader::new(input);
        loop {
            let any_event = any_reader.next_event().expect("AnyReader failed");
            let direct_event = direct_reader.next_event().expect("MarkdownReader failed");
            assert_eq!(any_event, direct_event, "event mismatch");
            if any_event.is_none() {
                break;
            }
        }
    }

    #[cfg(feature = "html")]
    #[test]
    fn roundtrip_full_document_html() {
        use docspec_html_reader::HtmlReader;

        let input = "<p>hello</p>";
        let mut any_reader = AnyReader::from_reader(InputFormat::Html, cursor(input))
            .expect("AnyReader should construct");
        let mut direct_reader = HtmlReader::new(input);
        loop {
            let any_event = any_reader.next_event().expect("AnyReader failed");
            let direct_event = direct_reader.next_event().expect("HtmlReader failed");
            assert_eq!(any_event, direct_event, "event mismatch");
            if any_event.is_none() {
                break;
            }
        }
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn assert_is_event_source() {
        fn check<S: EventSource>(_: S) {}
        check(
            AnyReader::from_reader(InputFormat::Markdown, cursor(""))
                .expect("AnyReader should construct"),
        );
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn new_compatibility_constructor_accepts_markdown_text() {
        let mut reader = AnyReader::new(InputFormat::Markdown, "# Hello");
        let events = collect_events(&mut reader).expect("AnyReader should not fail");
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { id: None, level: 1 },
                Event::Text {
                    content: "Hello".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]
        );
    }

    #[cfg(feature = "html")]
    #[test]
    fn new_compatibility_constructor_accepts_html_text() {
        let mut reader = AnyReader::new(InputFormat::Html, "<p>Hello</p>");
        let events = collect_events(&mut reader).expect("AnyReader should not fail");
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "Hello".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[cfg(any(feature = "markdown", feature = "html", feature = "docx"))]
    mod from_reader {
        use super::*;

        #[cfg(feature = "docx")]
        const SIMPLE_RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

        fn start_document() -> Event {
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            }
        }

        fn text(content: &str) -> Event {
            Event::Text {
                content: content.to_string(),
                style: TextStyle::default(),
            }
        }

        #[cfg(feature = "markdown")]
        #[test]
        fn from_reader_markdown_roundtrips_heading() {
            let mut reader =
                AnyReader::from_reader(InputFormat::Markdown, Cursor::new(b"# Hello".to_vec()))
                    .expect("AnyReader should construct");
            let events = collect_events(&mut reader).expect("AnyReader should emit events");
            assert_eq!(
                events,
                vec![
                    start_document(),
                    Event::StartHeading { id: None, level: 1 },
                    text("Hello"),
                    Event::EndHeading,
                    Event::EndDocument,
                ]
            );
        }

        #[cfg(feature = "html")]
        #[test]
        fn from_reader_html_roundtrips_paragraph() {
            let mut reader =
                AnyReader::from_reader(InputFormat::Html, Cursor::new(b"<p>x</p>".to_vec()))
                    .expect("AnyReader should construct");
            let events = collect_events(&mut reader).expect("AnyReader should emit events");
            assert_eq!(
                events,
                vec![
                    start_document(),
                    Event::StartParagraph {
                        alignment: None,
                        id: None,
                    },
                    text("x"),
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }

        #[cfg(feature = "docx")]
        #[test]
        fn from_reader_docx_roundtrips_paragraph() {
            let body = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body></w:document>"#;
            let bytes = docspec_test_fixtures::synth_docx(SIMPLE_RELS, body);
            let mut reader = AnyReader::from_reader(InputFormat::Docx, Cursor::new(bytes))
                .expect("AnyReader should construct");
            let events = collect_events(&mut reader).expect("AnyReader should emit events");
            assert_eq!(
                events,
                vec![
                    start_document(),
                    Event::StartParagraph {
                        alignment: None,
                        id: None,
                    },
                    text("hello"),
                    Event::EndParagraph,
                    Event::EndDocument,
                ]
            );
        }

        #[cfg(feature = "markdown")]
        #[test]
        fn from_reader_invalid_utf8_for_markdown_returns_io_invalid_data() {
            assert!(matches!(
                AnyReader::from_reader(
                    InputFormat::Markdown,
                    Cursor::new(b"\xff\xfe invalid".to_vec()),
                ),
                Err(docspec::Error::Io { source })
                    if source.kind() == std::io::ErrorKind::InvalidData
            ));
        }

        #[cfg(feature = "markdown")]
        #[test]
        fn from_reader_strips_bom_for_markdown() {
            let mut reader = AnyReader::from_reader(
                InputFormat::Markdown,
                Cursor::new("\u{FEFF}# Hello".as_bytes().to_vec()),
            )
            .expect("AnyReader should construct");
            let events = collect_events(&mut reader).expect("AnyReader should emit events");
            assert_eq!(
                events,
                vec![
                    start_document(),
                    Event::StartHeading { id: None, level: 1 },
                    text("Hello"),
                    Event::EndHeading,
                    Event::EndDocument,
                ]
            );
        }

        #[cfg(feature = "markdown")]
        #[test]
        fn from_path_opens_file_and_roundtrips() {
            let dir = tempfile::tempdir().expect("tempdir should be created");
            let path = dir.path().join("input.md");
            std::fs::write(&path, "# Hello").expect("tempfile should be written");
            let mut reader = AnyReader::from_path(InputFormat::Markdown, &path)
                .expect("AnyReader should construct");
            let events = collect_events(&mut reader).expect("AnyReader should emit events");
            assert_eq!(
                events,
                vec![
                    start_document(),
                    Event::StartHeading { id: None, level: 1 },
                    text("Hello"),
                    Event::EndHeading,
                    Event::EndDocument,
                ]
            );
        }
    }

    #[cfg(feature = "html")]
    #[test]
    fn html_assert_is_event_source() {
        fn check<S: EventSource>(_: S) {}
        check(
            AnyReader::from_reader(InputFormat::Html, cursor(""))
                .expect("AnyReader should construct"),
        );
    }
}
