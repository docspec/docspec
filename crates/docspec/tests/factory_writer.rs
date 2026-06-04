//! Integration tests for writer factory dispatch.

#![allow(clippy::expect_used, clippy::unwrap_used)]

#[cfg(test)]
mod tests {
    #[cfg(any(
        feature = "blocknote-writer",
        feature = "oxa-writer",
        feature = "html-writer"
    ))]
    use docspec::{AnyWriter, OutputFormat, TextStyle};
    #[cfg(any(
        feature = "blocknote-writer",
        feature = "oxa-writer",
        feature = "html-writer"
    ))]
    use docspec_core::{Event, EventSink};

    #[cfg(any(
        feature = "blocknote-writer",
        feature = "oxa-writer",
        feature = "html-writer"
    ))]
    fn start_document() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    #[cfg(any(
        feature = "blocknote-writer",
        feature = "oxa-writer",
        feature = "html-writer"
    ))]
    fn start_paragraph() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    #[cfg(any(
        feature = "blocknote-writer",
        feature = "oxa-writer",
        feature = "html-writer"
    ))]
    fn text(content: &str) -> Event {
        Event::Text {
            content: content.to_string(),
            style: TextStyle::default(),
        }
    }

    #[cfg(feature = "blocknote-writer")]
    #[test]
    fn blocknote_dispatch_writes_expected_bytes_simple() {
        let mut output = Vec::new();
        let mut writer = AnyWriter::new(OutputFormat::Blocknote, &mut output);
        writer
            .handle_event(start_document())
            .expect("handle_event failed");
        writer
            .handle_event(start_paragraph())
            .expect("handle_event failed");
        writer
            .handle_event(text("hello"))
            .expect("handle_event failed");
        writer
            .handle_event(Event::EndParagraph)
            .expect("handle_event failed");
        writer
            .handle_event(Event::EndDocument)
            .expect("handle_event failed");
        writer.finish().expect("finish failed");
        let json = String::from_utf8(output).expect("not utf8");
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"hello","styles":{}}],"children":[]}]"#
        );
    }

    #[cfg(feature = "blocknote-writer")]
    #[test]
    fn stack_tracking_is_active() {
        let mut output = Vec::new();
        let mut writer = AnyWriter::new(OutputFormat::Blocknote, &mut output);
        writer
            .handle_event(start_document())
            .expect("handle_event failed");
        writer
            .handle_event(text("bare text"))
            .expect("handle_event failed");
        writer
            .handle_event(Event::EndDocument)
            .expect("handle_event failed");
        writer.finish().expect("finish failed");
        let json = String::from_utf8(output).expect("not utf8");
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"bare text","styles":{}}],"children":[]}]"#
        );
    }

    #[cfg(feature = "blocknote-writer")]
    #[test]
    fn assert_is_event_sink() {
        fn check<K: EventSink>(_: K) {}
        let output = Vec::new();
        check(AnyWriter::new(OutputFormat::Blocknote, output));
    }

    #[cfg(feature = "oxa-writer")]
    #[test]
    fn oxa_dispatch_writes_expected_bytes_simple() {
        let mut output = Vec::new();
        let mut writer = AnyWriter::new(OutputFormat::Oxa, &mut output);
        writer
            .handle_event(start_document())
            .expect("handle_event failed");
        writer
            .handle_event(start_paragraph())
            .expect("handle_event failed");
        writer
            .handle_event(text("hello"))
            .expect("handle_event failed");
        writer
            .handle_event(Event::EndParagraph)
            .expect("handle_event failed");
        writer
            .handle_event(Event::EndDocument)
            .expect("handle_event failed");
        writer.finish().expect("finish failed");
        let json = String::from_utf8(output).expect("not utf8");
        assert_eq!(
            json,
            r#"{"type":"Document","children":[{"type":"Paragraph","children":[{"type":"Text","value":"hello"}]}]}"#
        );
    }

    #[cfg(feature = "oxa-writer")]
    #[test]
    fn oxa_stack_tracking_is_active() {
        let mut output = Vec::new();
        let mut writer = AnyWriter::new(OutputFormat::Oxa, &mut output);
        writer
            .handle_event(start_document())
            .expect("handle_event failed");
        writer
            .handle_event(text("bare text"))
            .expect("handle_event failed");
        writer
            .handle_event(Event::EndDocument)
            .expect("handle_event failed");
        writer.finish().expect("finish failed");
        let json = String::from_utf8(output).expect("not utf8");
        assert_eq!(
            json,
            r#"{"type":"Document","children":[{"type":"Paragraph","children":[{"type":"Text","value":"bare text"}]}]}"#
        );
    }

    #[cfg(feature = "oxa-writer")]
    #[test]
    fn oxa_assert_is_event_sink() {
        fn check<K: EventSink>(_: K) {}
        let output = Vec::new();
        check(AnyWriter::new(OutputFormat::Oxa, output));
    }

    #[cfg(feature = "html-writer")]
    #[test]
    fn html_dispatch_writes_expected_bytes_simple() {
        let mut output = Vec::new();
        let mut writer = AnyWriter::new(OutputFormat::Html, &mut output);
        writer
            .handle_event(start_document())
            .expect("handle_event failed");
        writer
            .handle_event(start_paragraph())
            .expect("handle_event failed");
        writer
            .handle_event(text("hello"))
            .expect("handle_event failed");
        writer
            .handle_event(Event::EndParagraph)
            .expect("handle_event failed");
        writer
            .handle_event(Event::EndDocument)
            .expect("handle_event failed");
        writer.finish().expect("finish failed");
        let html = String::from_utf8(output).expect("not utf8");
        assert_eq!(html, "<html><body><p>hello</p></body></html>");
    }

    #[cfg(feature = "html-writer")]
    #[test]
    fn html_stack_tracking_normalizes_bare_text() {
        let mut output = Vec::new();
        let mut writer = AnyWriter::new(OutputFormat::Html, &mut output);
        writer
            .handle_event(start_document())
            .expect("handle_event failed");
        writer
            .handle_event(text("bare text"))
            .expect("handle_event failed");
        writer
            .handle_event(Event::EndDocument)
            .expect("handle_event failed");
        writer.finish().expect("finish failed");
        let html = String::from_utf8(output).expect("not utf8");
        assert_eq!(html, "<html><body><p>bare text</p></body></html>");
    }

    #[cfg(feature = "html-writer")]
    #[test]
    fn html_escapes_special_characters() {
        let mut output = Vec::new();
        let mut writer = AnyWriter::new(OutputFormat::Html, &mut output);
        writer
            .handle_event(start_document())
            .expect("handle_event failed");
        writer
            .handle_event(start_paragraph())
            .expect("handle_event failed");
        writer
            .handle_event(text("a & b < c > d"))
            .expect("handle_event failed");
        writer
            .handle_event(Event::EndParagraph)
            .expect("handle_event failed");
        writer
            .handle_event(Event::EndDocument)
            .expect("handle_event failed");
        writer.finish().expect("finish failed");
        let html = String::from_utf8(output).expect("not utf8");
        assert_eq!(
            html,
            "<html><body><p>a &amp; b &lt; c &gt; d</p></body></html>"
        );
    }

    #[cfg(feature = "html-writer")]
    #[test]
    fn html_dispatch_is_event_sink() {
        fn check<K: EventSink>(_: K) {}
        let output = Vec::new();
        check(AnyWriter::new(OutputFormat::Html, output));
    }
}
