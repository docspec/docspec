//! Integration tests for `HtmlReader`.

#![allow(clippy::expect_used, clippy::panic)]

#[cfg(test)]
mod tests {
    use docspec_core::{Error, Event, Result, TextStyle};
    use docspec_html_reader::{EventSource as _, HtmlReader};

    fn collect_events(input: &str) -> Vec<Event> {
        let mut reader = HtmlReader::new(input);
        let mut events = Vec::new();
        while let Some(ev) = reader.next_event().expect("unexpected parse error") {
            events.push(ev);
        }
        events
    }

    fn collect_events_result(input: &str) -> Result<Vec<Event>> {
        let mut reader = HtmlReader::new(input);
        let mut events = Vec::new();
        loop {
            match reader.next_event()? {
                Some(ev) => events.push(ev),
                None => return Ok(events),
            }
        }
    }

    #[test]
    fn basic_paragraph() {
        let events = collect_events("<p>hello</p>");
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
                    content: "hello".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn empty_paragraph() {
        let events = collect_events("<p></p>");
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
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn multiple_paragraphs() {
        let events = collect_events("<p>one</p><p>two</p>");
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
                    content: "one".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "two".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn whitespace_around_paragraphs() {
        // Leading/trailing whitespace text is outside any paragraph, so it is dropped
        let events = collect_events("  <p>x</p>  ");
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
                    content: "x".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn text_with_unknown_inline_tags() {
        // Text inside <strong> is preserved as separate Text events; formatting is dropped
        let events = collect_events("<p>hello <strong>bold</strong> world</p>");
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
                    content: "hello ".to_string(),
                    style: TextStyle::default(),
                },
                Event::Text {
                    content: "bold".to_string(),
                    style: TextStyle::default(),
                },
                Event::Text {
                    content: " world".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn utf8_multibyte() {
        let events = collect_events("<p>caf\u{e9} \u{65e5}\u{672c}\u{8a9e} \u{f1}</p>");
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
                    content: "caf\u{e9} \u{65e5}\u{672c}\u{8a9e} \u{f1}".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn entity_decoding() {
        // html5gum automatically decodes HTML entities
        let events = collect_events("<p>a &amp; b &lt; c</p>");
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
                    content: "a & b < c".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn unclosed_paragraph_autocloses() {
        // Unclosed <p> must produce EndParagraph before EndDocument
        let events = collect_events("<p>unclosed");
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
                    content: "unclosed".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn orphan_end_tag_ignored() {
        let events = collect_events("</p>");
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn nested_paragraph_inner_ignored() {
        // Inner <p> is ignored (already in_paragraph); inner </p> closes the outer;
        // " tail" after close is dropped (not in_paragraph); outer </p> is orphan
        let events = collect_events("<p>outer <p>inner</p> tail</p>");
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
                    content: "outer ".to_string(),
                    style: TextStyle::default(),
                },
                Event::Text {
                    content: "inner".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn text_outside_paragraph_dropped() {
        let events = collect_events("loose text");
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn comments_inside_paragraph_ignored() {
        // Comments produce no events; text before and after are separate Text events
        let events = collect_events("<p>before<!-- comment -->after</p>");
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
                    content: "before".to_string(),
                    style: TextStyle::default(),
                },
                Event::Text {
                    content: "after".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn doctype_before_paragraph_ignored() {
        let events = collect_events("<!DOCTYPE html><p>hi</p>");
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
                    content: "hi".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn attributes_on_p_ignored() {
        // HTML attributes on <p> are ignored; id comes from None, not from HTML attribute
        let events = collect_events(r#"<p class="foo" id="bar">x</p>"#);
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
                    content: "x".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn empty_input() {
        let events = collect_events("");
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn idempotent_after_eof() {
        let mut reader = HtmlReader::new("");
        // Drain to Ok(None)
        while reader.next_event().expect("no error").is_some() {}
        // Call twice more — must return Ok(None) both times
        assert_eq!(
            reader.next_event().expect("no error after eof"),
            None,
            "first call after eof should return None"
        );
        assert_eq!(
            reader.next_event().expect("no error after eof 2"),
            None,
            "second call after eof should return None"
        );
    }

    #[test]
    fn case_sensitivity_uppercase_p() {
        // html5gum normalizes tag names to lowercase, so <P>hi</P> is treated
        // identically to <p>hi</p> — no case-insensitive comparison needed.
        let events = collect_events("<P>hi</P>");
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
                    content: "hi".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn self_closing_p_slash() {
        // html5gum emits StartTag { self_closing: true } for <p />, so the
        // implementation immediately follows StartParagraph with EndParagraph.
        let events = collect_events("<p />");
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
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn multiple_text_tokens_in_one_paragraph() {
        // Text before and after a comment produces two separate Text events (no coalescing)
        let events = collect_events("<p>first<!-- sep -->second</p>");
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
                    content: "first".to_string(),
                    style: TextStyle::default(),
                },
                Event::Text {
                    content: "second".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn token_error_propagates() {
        // "<" triggers EofBeforeTagName in html5gum, which is reachable with &str input.
        let result = collect_events_result("<");
        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message, "html5gum: EofBeforeTagName",
                    "expected exact html5gum error message"
                );
                assert_eq!(position, None, "expected no position");
            }
            other => panic!("expected Parse error, got: {other:?}"),
        }
    }
}
