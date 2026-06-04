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

        let mut reader = AnyReader::new(InputFormat::Markdown, "# h");
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

        let mut reader = AnyReader::new(InputFormat::Html, "<p>x</p>");
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
        let mut any_reader = AnyReader::new(InputFormat::Markdown, input);
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
        let mut any_reader = AnyReader::new(InputFormat::Html, input);
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
        check(AnyReader::new(InputFormat::Markdown, ""));
    }

    #[cfg(feature = "html")]
    #[test]
    fn html_assert_is_event_source() {
        fn check<S: EventSource>(_: S) {}
        check(AnyReader::new(InputFormat::Html, ""));
    }
}
