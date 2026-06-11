//! Unit tests for `OxaWriter` event emission.

#![allow(clippy::expect_used, clippy::unwrap_used)]

#[cfg(test)]
mod tests {
    use docspec_core::{Error, Event, EventSink as _, ImageSource, TextStyleKind};
    use docspec_oxa_writer::OxaWriter;
    use docspec_test_utils::builders::{start_document, start_paragraph, text};
    use serde_json::{json, Value};

    fn run(events: Vec<Event>) -> Value {
        let s = run_raw(events);
        serde_json::from_str(&s).expect("valid JSON")
    }

    fn run_raw(events: Vec<Event>) -> String {
        let mut buf = Vec::new();
        let mut w = OxaWriter::new(&mut buf);
        for e in events {
            w.handle_event(e).expect("handle_event");
        }
        w.finish().expect("finish");
        String::from_utf8(buf).expect("utf-8")
    }

    fn try_run(events: Vec<Event>) -> docspec_core::Result<String> {
        let mut buf = Vec::new();
        let mut w = OxaWriter::new(&mut buf);
        for e in events {
            w.handle_event(e)?;
        }
        w.finish()?;
        String::from_utf8(buf).map_err(|err| Error::Other {
            message: err.to_string(),
        })
    }

    fn end_doc() -> Event {
        Event::EndDocument
    }

    fn end_para() -> Event {
        Event::EndParagraph
    }

    #[test]
    fn document_only() {
        let v = run(vec![start_document(), end_doc()]);
        assert_eq!(v, json!({"type": "Document", "children": []}));
    }

    #[test]
    fn single_paragraph_single_text() {
        let v = run(vec![
            start_document(),
            start_paragraph(),
            text("Hello"),
            end_para(),
            end_doc(),
        ]);
        assert_eq!(
            v,
            json!({
                "type": "Document",
                "children": [{
                    "type": "Paragraph",
                    "children": [{"type": "Text", "value": "Hello"}]
                }]
            })
        );
    }

    #[test]
    fn multi_paragraph_multi_text() {
        let v = run(vec![
            start_document(),
            start_paragraph(),
            text("One"),
            end_para(),
            start_paragraph(),
            text("Two"),
            end_para(),
            end_doc(),
        ]);
        assert_eq!(
            v,
            json!({
                "type": "Document",
                "children": [
                    {
                        "type": "Paragraph",
                        "children": [{"type": "Text", "value": "One"}]
                    },
                    {
                        "type": "Paragraph",
                        "children": [{"type": "Text", "value": "Two"}]
                    }
                ]
            })
        );
    }

    #[test]
    fn empty_paragraph() {
        let v = run(vec![
            start_document(),
            start_paragraph(),
            end_para(),
            end_doc(),
        ]);
        assert_eq!(
            v,
            json!({
                "type": "Document",
                "children": [{"type": "Paragraph", "children": []}]
            })
        );
    }

    #[test]
    fn multiple_text_in_paragraph() {
        let v = run(vec![
            start_document(),
            start_paragraph(),
            text("a"),
            text("b"),
            end_para(),
            end_doc(),
        ]);
        assert_eq!(
            v,
            json!({
                "type": "Document",
                "children": [{
                    "type": "Paragraph",
                    "children": [
                        {"type": "Text", "value": "a"},
                        {"type": "Text", "value": "b"}
                    ]
                }]
            })
        );
    }

    #[test]
    fn unsupported_block_events_at_document_level_dropped() {
        let v = run(vec![
            start_document(),
            Event::StartHeading { level: 1, id: None },
            text("Title"),
            Event::EndHeading,
            Event::Image {
                alt: None,
                decorative: false,
                id: None,
                source: ImageSource::Uri {
                    uri: "x".to_string(),
                },
                title: None,
            },
            Event::ThematicBreak { id: None },
            end_doc(),
        ]);
        assert_eq!(v, json!({"type": "Document", "children": []}));
    }

    #[test]
    fn softbreak_and_linebreak_dropped() {
        let v = run(vec![
            start_document(),
            start_paragraph(),
            text("a"),
            Event::SoftBreak,
            text("b"),
            Event::LineBreak,
            text("c"),
            end_para(),
            end_doc(),
        ]);
        assert_eq!(
            v,
            json!({
                "type": "Document",
                "children": [{
                    "type": "Paragraph",
                    "children": [
                        {"type": "Text", "value": "a"},
                        {"type": "Text", "value": "b"},
                        {"type": "Text", "value": "c"}
                    ]
                }]
            })
        );
    }

    #[test]
    fn text_with_json_special_chars() {
        let json = run_raw(vec![
            start_document(),
            start_paragraph(),
            text("a\nb\t\"c\\d"),
            end_para(),
            end_doc(),
        ]);
        assert_eq!(
            json,
            "{\"type\":\"Document\",\"children\":[{\"type\":\"Paragraph\",\"children\":[{\"type\":\"Text\",\"value\":\"a\\nb\\t\\\"c\\\\d\"}]}]}"
        );
    }

    // # Reason: unicode/emoji literals are intentional content of this test.
    #[allow(clippy::non_ascii_literal)]
    #[test]
    fn text_with_unicode() {
        let json = run_raw(vec![
            start_document(),
            start_paragraph(),
            text("héllo 🎉 日本語"),
            end_para(),
            end_doc(),
        ]);
        assert_eq!(
            json,
            r#"{"type":"Document","children":[{"type":"Paragraph","children":[{"type":"Text","value":"héllo 🎉 日本語"}]}]}"#
        );
    }

    #[test]
    fn empty_string_text() {
        let v = run(vec![
            start_document(),
            start_paragraph(),
            text(""),
            end_para(),
            end_doc(),
        ]);
        assert_eq!(
            v,
            json!({
                "type": "Document",
                "children": [{
                    "type": "Paragraph",
                    "children": [{"type": "Text", "value": ""}]
                }]
            })
        );
    }

    #[test]
    fn orphan_text_dropped() {
        let v = run(vec![
            text("before"),
            start_document(),
            start_paragraph(),
            end_para(),
            text("between"),
            start_paragraph(),
            text("inside"),
            end_para(),
            end_doc(),
        ]);
        assert_eq!(
            v,
            json!({
                "type": "Document",
                "children": [
                    {"type": "Paragraph", "children": []},
                    {
                        "type": "Paragraph",
                        "children": [{"type": "Text", "value": "inside"}]
                    }
                ]
            })
        );
    }

    #[test]
    fn orphan_end_paragraph_no_op() {
        let v = run(vec![start_document(), end_para(), end_doc()]);
        assert_eq!(v, json!({"type": "Document", "children": []}));
    }

    #[test]
    fn double_start_document_no_op() {
        let v = run(vec![
            start_document(),
            start_document(),
            start_paragraph(),
            text("x"),
            end_para(),
            end_doc(),
        ]);
        assert_eq!(
            v,
            json!({
                "type": "Document",
                "children": [{
                    "type": "Paragraph",
                    "children": [{"type": "Text", "value": "x"}]
                }]
            })
        );
    }

    #[test]
    fn end_document_without_start_no_op() {
        let err = try_run(vec![end_doc()]).expect_err("expected Err");
        assert_eq!(
            err.to_string(),
            "JSON error: cannot finish: open containers remain or no root value written"
        );
    }

    #[test]
    fn nested_paragraph_no_op() {
        let v = run(vec![
            start_document(),
            start_paragraph(),
            start_paragraph(),
            text("a"),
            end_para(),
            end_para(),
            end_doc(),
        ]);
        assert_eq!(
            v,
            json!({
                "type": "Document",
                "children": [{
                    "type": "Paragraph",
                    "children": [{"type": "Text", "value": "a"}]
                }]
            })
        );
    }

    #[test]
    fn styled_input_dropped() {
        let styled_events = vec![
            start_document(),
            start_paragraph(),
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
                id: None,
            },
            text("x"),
            Event::EndTextStyle,
            end_para(),
            end_doc(),
        ];
        let unstyled_events = vec![
            start_document(),
            start_paragraph(),
            text("x"),
            end_para(),
            end_doc(),
        ];
        let styled_output = run_raw(styled_events);
        let unstyled_output = run_raw(unstyled_events);
        assert_eq!(styled_output, unstyled_output);
    }
}
