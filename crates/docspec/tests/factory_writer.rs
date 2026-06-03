//! Integration tests for writer factory dispatch.

#![allow(clippy::expect_used, clippy::unwrap_used)]

#[cfg(test)]
mod tests {
    #[cfg(feature = "blocknote")]
    use docspec::{AnyWriter, OutputFormat, TextStyle};
    #[cfg(feature = "blocknote")]
    use docspec_core::{Event, EventSink};

    #[cfg(feature = "blocknote")]
    fn start_document() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    #[cfg(feature = "blocknote")]
    fn start_paragraph() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    #[cfg(feature = "blocknote")]
    fn text(content: &str) -> Event {
        Event::Text {
            content: content.to_string(),
            style: TextStyle::default(),
        }
    }

    #[cfg(feature = "blocknote")]
    #[test]
    fn blocknote_dispatch_writes_expected_bytes_simple() {
        let mut output = Vec::new();
        let mut writer = AnyWriter::new(OutputFormat::Blocknote, &mut output);
        writer.handle_event(start_document()).expect("handle_event failed");
        writer.handle_event(start_paragraph()).expect("handle_event failed");
        writer.handle_event(text("hello")).expect("handle_event failed");
        writer.handle_event(Event::EndParagraph).expect("handle_event failed");
        writer.handle_event(Event::EndDocument).expect("handle_event failed");
        writer.finish().expect("finish failed");
        let json = String::from_utf8(output).expect("not utf8");
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"hello","styles":{}}],"children":[]}]"#
        );
    }

    #[cfg(feature = "blocknote")]
    #[test]
    fn stack_tracking_is_active() {
        let mut output = Vec::new();
        let mut writer = AnyWriter::new(OutputFormat::Blocknote, &mut output);
        writer.handle_event(start_document()).expect("handle_event failed");
        writer.handle_event(text("bare text")).expect("handle_event failed");
        writer.handle_event(Event::EndDocument).expect("handle_event failed");
        writer.finish().expect("finish failed");
        let json = String::from_utf8(output).expect("not utf8");
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"bare text","styles":{}}],"children":[]}]"#
        );
    }

    #[cfg(feature = "blocknote")]
    #[test]
    fn assert_is_event_sink() {
        fn check<K: EventSink>(_: K) {}
        let output = Vec::new();
        check(AnyWriter::new(OutputFormat::Blocknote, output));
    }
}
