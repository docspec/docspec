//! Integration tests for the enum-dispatch reader factory.

#![allow(clippy::expect_used, clippy::unwrap_used)]

#[cfg(test)]
mod tests {
    #[cfg(any(feature = "markdown", feature = "html"))]
    use docspec::{AnyReader, InputFormat};
    #[cfg(any(feature = "markdown", feature = "html"))]
    use docspec_core::EventSource;

    #[cfg(feature = "markdown")]
    #[test]
    fn markdown_dispatch_emits_first_event() {
        use docspec_markdown_reader::MarkdownReader;

        let mut reader = AnyReader::from_str(InputFormat::Markdown, "# h").unwrap();
        let event = reader.next_event().expect("AnyReader should not fail");
        let expected = MarkdownReader::from_str("# h")
            .next_event()
            .expect("direct reader should not fail");
        assert_eq!(event, expected);
    }

    #[cfg(feature = "html")]
    #[test]
    fn html_dispatch_emits_first_event() {
        use docspec_html_reader::HtmlReader;

        let mut reader = AnyReader::from_str(InputFormat::Html, "<p>x</p>").unwrap();
        let event = reader.next_event().expect("AnyReader should not fail");
        let expected = HtmlReader::from_str("<p>x</p>")
            .next_event()
            .expect("direct reader should not fail");
        assert_eq!(event, expected);
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn roundtrip_full_document_markdown() {
        use docspec_markdown_reader::MarkdownReader;

        let input = "# Hello\n\nWorld";
        let mut any_reader = AnyReader::from_str(InputFormat::Markdown, input).unwrap();
        let mut direct_reader = MarkdownReader::from_str(input);
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
        let mut any_reader = AnyReader::from_str(InputFormat::Html, input).unwrap();
        let mut direct_reader = HtmlReader::from_str(input);
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
        check(AnyReader::from_str(InputFormat::Markdown, "").unwrap());
    }

    #[cfg(feature = "html")]
    #[test]
    fn html_assert_is_event_source() {
        fn check<S: EventSource>(_: S) {}
        check(AnyReader::from_str(InputFormat::Html, "").unwrap());
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn any_reader_from_reader_matches_from_str_markdown() {
        use std::io::Cursor;

        let input = "# Hello\n\nWorld";
        let mut r1 = AnyReader::from_str(InputFormat::Markdown, input).unwrap();
        let mut r2 =
            AnyReader::from_reader(InputFormat::Markdown, Cursor::new(input.as_bytes())).unwrap();
        let events1: Vec<_> = core::iter::from_fn(|| r1.next_event().unwrap()).collect();
        let events2: Vec<_> = core::iter::from_fn(|| r2.next_event().unwrap()).collect();
        assert_eq!(events1, events2);
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn any_reader_from_reader_strips_bom_markdown() {
        use docspec_core::Event;
        use std::io::Cursor;

        let input = b"\xEF\xBB\xBF# Hello".to_vec();
        let mut reader = AnyReader::from_reader(InputFormat::Markdown, Cursor::new(input))
            .expect("from_reader succeeds");

        let events: Vec<_> = core::iter::from_fn(|| reader.next_event().unwrap()).collect();
        let has_heading = events
            .iter()
            .any(|event| matches!(event, Event::StartHeading { .. }));
        assert!(
            has_heading,
            "expected StartHeading event after BOM strip; events: {events:?}"
        );

        let bom_in_text = events.iter().any(|event| match event {
            Event::Text { content, .. } => content.starts_with('\u{FEFF}'),
            _ => false,
        });
        assert!(
            !bom_in_text,
            "BOM should not appear in any Text event content"
        );
    }

    #[cfg(feature = "html")]
    #[test]
    fn any_reader_from_reader_matches_from_str_html() {
        use std::io::Cursor;

        let input = "<p>hello</p>";
        let mut r1 = AnyReader::from_str(InputFormat::Html, input).unwrap();
        let mut r2 =
            AnyReader::from_reader(InputFormat::Html, Cursor::new(input.as_bytes())).unwrap();
        let events1: Vec<_> = core::iter::from_fn(|| r1.next_event().unwrap()).collect();
        let events2: Vec<_> = core::iter::from_fn(|| r2.next_event().unwrap()).collect();
        assert_eq!(events1, events2);
    }
}
