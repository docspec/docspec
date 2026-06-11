//! Integration tests for `MarkdownWriter`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_docs_in_private_items
)]

#[cfg(test)]
mod tests {
    use docspec_core::{Event, EventSink as _, ImageSource, TextStyleKind};
    use docspec_markdown_writer::MarkdownWriter;

    fn run(events: Vec<Event>) -> String {
        let mut buf = Vec::new();
        let mut writer = MarkdownWriter::new(&mut buf);
        for event in events {
            writer.handle_event(event).unwrap();
        }
        writer.finish().unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn empty_writer_finishes_with_no_output() {
        let mut buf = Vec::new();
        let writer = MarkdownWriter::new(&mut buf);
        writer.finish().unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "");
    }

    #[test]
    fn start_document_then_end_document_no_blocks() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::EndDocument,
            ]),
            ""
        );
    }

    #[test]
    fn finish_without_start_is_noop() {
        let mut buf = Vec::new();
        let writer = MarkdownWriter::new(&mut buf);
        writer.finish().unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "");
    }

    #[test]
    fn drops_all_unsupported_events_emitting_nothing() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::Image {
                    id: None,
                    source: ImageSource::Uri { uri: String::new() },
                    title: None,
                    alt: None,
                    decorative: false,
                },
                Event::SoftBreak,
                Event::LineBreak,
                Event::ThematicBreak { id: None },
                Event::EndDocument,
            ]),
            ""
        );
    }

    #[test]
    fn single_paragraph() {
        assert_eq!(
            run(vec![
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
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "Hello\n\n"
        );
    }

    #[test]
    fn two_paragraphs() {
        assert_eq!(
            run(vec![
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
                },
                Event::EndParagraph,
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "World".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "Hello\n\nWorld\n\n"
        );
    }

    #[test]
    fn empty_paragraph_emits_nothing() {
        assert_eq!(
            run(vec![
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
            ]),
            ""
        );
    }

    #[test]
    fn paragraph_then_empty_paragraph_then_paragraph() {
        assert_eq!(
            run(vec![
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
                    content: "A".to_string(),
                },
                Event::EndParagraph,
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "B".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "A\n\nB\n\n"
        );
    }

    #[test]
    fn empty_paragraph_between_headings_emits_nothing() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 1, id: None },
                Event::Text {
                    content: "A".to_string(),
                },
                Event::EndHeading,
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::StartHeading { level: 2, id: None },
                Event::Text {
                    content: "B".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "# A\n\n## B\n\n"
        );
    }

    #[test]
    fn heading_level_1() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 1, id: None },
                Event::Text {
                    content: "Hello".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "# Hello\n\n"
        );
    }

    #[test]
    fn heading_level_2() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 2, id: None },
                Event::Text {
                    content: "Hello".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "## Hello\n\n"
        );
    }

    #[test]
    fn heading_level_6() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 6, id: None },
                Event::Text {
                    content: "Hello".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "###### Hello\n\n"
        );
    }

    #[test]
    fn heading_level_0_clamps_to_1() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 0, id: None },
                Event::Text {
                    content: "Hello".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "# Hello\n\n"
        );
    }

    #[test]
    fn heading_level_7_clamps_to_6() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 7, id: None },
                Event::Text {
                    content: "Hello".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "###### Hello\n\n"
        );
    }

    #[test]
    fn heading_level_255_clamps_to_6() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading {
                    level: 255,
                    id: None,
                },
                Event::Text {
                    content: "Hello".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "###### Hello\n\n"
        );
    }

    #[test]
    fn heading_id_is_dropped() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading {
                    level: 1,
                    id: Some("anchor".to_string()),
                },
                Event::Text {
                    content: "Hello".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "# Hello\n\n"
        );
    }

    #[test]
    fn heading_after_heading() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 1, id: None },
                Event::Text {
                    content: "H1".to_string(),
                },
                Event::EndHeading,
                Event::StartHeading { level: 2, id: None },
                Event::Text {
                    content: "H2".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "# H1\n\n## H2\n\n"
        );
    }

    #[test]
    fn heading_after_paragraph() {
        assert_eq!(
            run(vec![
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
                    content: "Para".to_string(),
                },
                Event::EndParagraph,
                Event::StartHeading { level: 1, id: None },
                Event::Text {
                    content: "H1".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "Para\n\n# H1\n\n"
        );
    }

    #[test]
    fn paragraph_after_heading() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 1, id: None },
                Event::Text {
                    content: "H1".to_string(),
                },
                Event::EndHeading,
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "Para".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "# H1\n\nPara\n\n"
        );
    }

    #[test]
    fn empty_heading_emits_prefix_only() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 2, id: None },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "## \n\n"
        );
    }

    #[test]
    fn empty_heading_between_paragraphs() {
        assert_eq!(
            run(vec![
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
                    content: "A".to_string(),
                },
                Event::EndParagraph,
                Event::StartHeading { level: 3, id: None },
                Event::EndHeading,
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "B".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "A\n\n### \n\nB\n\n"
        );
    }

    #[test]
    fn finish_with_open_paragraph_closes_only_if_text_present() {
        let mut buf = Vec::new();
        let mut writer = MarkdownWriter::new(&mut buf);
        writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .unwrap();
        writer
            .handle_event(Event::StartParagraph {
                alignment: None,
                id: None,
            })
            .unwrap();
        writer
            .handle_event(Event::Text {
                content: "X".to_string(),
            })
            .unwrap();
        writer.finish().unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "X\n\n");
    }

    #[test]
    fn finish_with_open_empty_paragraph_emits_nothing() {
        let mut buf = Vec::new();
        let mut writer = MarkdownWriter::new(&mut buf);
        writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .unwrap();
        writer
            .handle_event(Event::StartParagraph {
                alignment: None,
                id: None,
            })
            .unwrap();
        writer.finish().unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "");
    }

    #[test]
    fn finish_with_open_empty_heading_emits_prefix() {
        let mut buf = Vec::new();
        let mut writer = MarkdownWriter::new(&mut buf);
        writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .unwrap();
        writer
            .handle_event(Event::StartHeading { level: 1, id: None })
            .unwrap();
        writer.finish().unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "# \n\n");
    }

    #[test]
    fn escape_text_with_asterisk() {
        assert_eq!(
            run(vec![
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
                    content: "*bold*".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "\\*bold\\*\n\n"
        );
    }

    #[test]
    fn escape_text_with_backtick() {
        assert_eq!(
            run(vec![
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
                    content: "foo`bar".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "foo\\`bar\n\n"
        );
    }

    #[test]
    fn escape_text_with_open_bracket() {
        assert_eq!(
            run(vec![
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
                    content: "[link]".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "\\[link\\]\n\n"
        );
    }

    #[test]
    fn escape_text_with_lt_amp() {
        assert_eq!(
            run(vec![
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
                    content: "a<b&c".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "a\\<b\\&c\n\n"
        );
    }

    #[test]
    fn escape_text_with_embedded_newline_normalized_to_space() {
        assert_eq!(
            run(vec![
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
                    content: "foo\nbar".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "foo bar\n\n"
        );
    }

    #[test]
    fn escape_text_with_backslash() {
        assert_eq!(
            run(vec![
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
                    content: "a\\b".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "a\\\\b\n\n"
        );
    }

    #[test]
    fn paragraph_starts_with_hash_is_escaped() {
        assert_eq!(
            run(vec![
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
                    content: "# not a heading".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "\\# not a heading\n\n"
        );
    }

    #[test]
    fn paragraph_starts_with_dash_space_is_escaped() {
        assert_eq!(
            run(vec![
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
                    content: "- not a list".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "\\- not a list\n\n"
        );
    }

    #[test]
    fn paragraph_starts_with_one_dot_is_escaped() {
        assert_eq!(
            run(vec![
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
                    content: "1. not a list".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "1\\. not a list\n\n"
        );
    }

    #[test]
    fn paragraph_with_unicode_passes_through() {
        assert_eq!(
            run(vec![
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
                    content: "日本語 🎉".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "日本語 🎉\n\n"
        );
    }

    #[test]
    fn dropped_event_does_not_break_block() {
        assert_eq!(
            run(vec![
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
                    content: "a".to_string(),
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                Event::Text {
                    content: "b".to_string(),
                },
                Event::EndTextStyle,
                Event::Text {
                    content: "c".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "abc\n\n"
        );
    }

    #[test]
    fn softbreak_inside_paragraph_dropped() {
        assert_eq!(
            run(vec![
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
                    content: "a".to_string(),
                },
                Event::SoftBreak,
                Event::Text {
                    content: "b".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "ab\n\n"
        );
    }

    #[test]
    fn thematic_break_between_paragraphs_dropped() {
        assert_eq!(
            run(vec![
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
                    content: "a".to_string(),
                },
                Event::EndParagraph,
                Event::ThematicBreak { id: None },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "b".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "a\n\nb\n\n"
        );
    }

    #[test]
    fn heading_level_4() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 4, id: None },
                Event::Text {
                    content: "Hello".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "#### Hello\n\n"
        );
    }

    #[test]
    fn heading_level_5() {
        assert_eq!(
            run(vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 5, id: None },
                Event::Text {
                    content: "Hello".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "##### Hello\n\n"
        );
    }

    #[test]
    fn finish_with_open_heading_with_text_emits_separator() {
        let mut buf = Vec::new();
        let mut writer = MarkdownWriter::new(&mut buf);
        writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .unwrap();
        writer
            .handle_event(Event::StartHeading { level: 2, id: None })
            .unwrap();
        writer
            .handle_event(Event::Text {
                content: "Title".to_string(),
            })
            .unwrap();
        writer.finish().unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "## Title\n\n");
    }

    #[test]
    fn finish_after_start_document_no_blocks_no_end_document() {
        let mut buf = Vec::new();
        let mut writer = MarkdownWriter::new(&mut buf);
        writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .unwrap();
        writer.finish().unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "");
    }
}
