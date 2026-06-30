//! Integration tests for `BlockNoteWriter`.

#![allow(
    clippy::expect_used,
    clippy::redundant_test_prefix,
    clippy::items_after_statements,
    clippy::indexing_slicing
)]

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::Arc;

    use docspec_blocknote_writer::BlockNoteWriter;
    use docspec_core::{
        AssetHandle, Color, Event, EventSink as _, ImageSource, StackTrackingSink, TextAlignment,
        TextStyleKind,
    };
    use docspec_test_utils::builders::text;
    use docspec_test_utils::FailingWriter;

    #[derive(Debug)]
    struct MockAssetHandle {
        asset_id: String,
        content_type: Option<String>,
        bytes: Vec<u8>,
        fail_stream: bool,
    }

    impl MockAssetHandle {
        fn new(asset_id: &str, content_type: &str, bytes: &[u8]) -> Self {
            Self {
                asset_id: asset_id.to_string(),
                content_type: Some(content_type.to_string()),
                bytes: bytes.to_vec(),
                fail_stream: false,
            }
        }

        fn unknown_content_type(asset_id: &str) -> Self {
            Self {
                asset_id: asset_id.to_string(),
                content_type: None,
                bytes: Vec::new(),
                fail_stream: false,
            }
        }

        fn failing(asset_id: &str, content_type: &str) -> Self {
            Self {
                asset_id: asset_id.to_string(),
                content_type: Some(content_type.to_string()),
                bytes: Vec::new(),
                fail_stream: true,
            }
        }
    }

    impl AssetHandle for MockAssetHandle {
        fn content_type(&self) -> Option<std::borrow::Cow<'_, str>> {
            self.content_type.as_deref().map(std::borrow::Cow::Borrowed)
        }

        fn stream_to(&self, writer: &mut dyn Write) -> std::io::Result<u64> {
            if self.fail_stream {
                return Err(std::io::Error::other("mock stream failure"));
            }
            writer.write_all(&self.bytes)?;
            Ok(u64::try_from(self.bytes.len()).unwrap_or(u64::MAX))
        }

        fn asset_id(&self) -> &str {
            &self.asset_id
        }
    }

    fn run_events(events: &[Event]) -> String {
        let mut buf = Vec::<u8>::new();
        let writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
        docspec_test_utils::drive(writer, events.iter().cloned());
        String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8")
    }

    fn run_events_result(events: &[Event]) -> docspec_core::Result<String> {
        let mut buf = Vec::<u8>::new();
        let writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
        docspec_test_utils::try_drive(writer, events.iter().cloned())?;
        String::from_utf8(buf).map_err(|err| docspec_core::Error::Other {
            message: format!("BlockNoteWriter output should be valid UTF-8: {err}"),
        })
    }

    fn run_direct_writer_events(events: &[Event]) -> String {
        let mut buf = Vec::<u8>::new();
        let writer = BlockNoteWriter::new(&mut buf);
        docspec_test_utils::drive(writer, events.iter().cloned());
        String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8")
    }

    fn start_document() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    fn start_paragraph() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    fn start_paragraph_with_alignment(alignment: TextAlignment) -> Event {
        Event::StartParagraph {
            alignment: Some(alignment),
            id: None,
        }
    }

    fn start_text_style(kind: TextStyleKind) -> Event {
        Event::StartTextStyle { kind, id: None }
    }

    fn start_heading(level: u8) -> Event {
        Event::StartHeading { level, id: None }
    }

    fn start_blockquote() -> Event {
        Event::StartBlockQuote { id: None }
    }

    fn start_preformatted(syntax: Option<&str>) -> Event {
        Event::StartPreformatted {
            syntax: syntax.map(str::to_string),
            id: None,
        }
    }

    fn start_table() -> Event {
        Event::StartTable { id: None }
    }

    fn start_table_row() -> Event {
        Event::StartTableRow { id: None }
    }

    fn start_table_cell() -> Event {
        Event::StartTableCell {
            colspan: None,
            id: None,
            rowspan: None,
        }
    }

    #[test]
    fn empty_document() {
        let json = run_events(&[start_document(), Event::EndDocument]);
        assert_eq!(json, "[]");
    }

    #[test]
    fn single_paragraph() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("Hello"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Hello","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn paragraph_with_explicit_left_alignment_omits_default_prop() {
        let json = run_events(&[
            start_document(),
            start_paragraph_with_alignment(TextAlignment::Left),
            text("Hello"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Hello","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn paragraph_with_center_alignment_preserves_non_default_prop() {
        let json = run_events(&[
            start_document(),
            start_paragraph_with_alignment(TextAlignment::Center),
            text("Hello"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"center"},"content":[{"type":"text","text":"Hello","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn paragraph_with_right_alignment_preserves_non_default_prop() {
        let json = run_events(&[
            start_document(),
            start_paragraph_with_alignment(TextAlignment::Right),
            text("Hello"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"right"},"content":[{"type":"text","text":"Hello","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn paragraph_with_justify_alignment_preserves_non_default_prop() {
        let json = run_events(&[
            start_document(),
            start_paragraph_with_alignment(TextAlignment::Justify),
            text("Hello"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"justify"},"content":[{"type":"text","text":"Hello","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn bold_text() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Bold),
            text("Bold"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Bold","styles":{"bold":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_renders_as_newline() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("Line one"),
            Event::SoftBreak,
            text("Line two"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Line one","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"Line two","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_inside_heading() {
        let json = run_events(&[
            start_document(),
            start_heading(2),
            text("Title one"),
            Event::SoftBreak,
            text("Title two"),
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"Title one","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"Title two","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_inside_table_cell() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("Cell line one"),
            Event::SoftBreak,
            text("Cell line two"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Cell line one","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"Cell line two","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_inside_list_item() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("Bullet line one"),
            Event::SoftBreak,
            text("Bullet line two"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"Bullet line one","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"Bullet line two","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_inside_link_display_text() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            Event::StartLink {
                href: "https://example.com".to_string(),
                id: None,
                title: None,
            },
            text("Click line one"),
            Event::SoftBreak,
            text("click line two"),
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"Click line one","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"click line two","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_inside_blockquote() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            start_paragraph(),
            text("Quote line one"),
            Event::SoftBreak,
            text("Quote line two"),
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"Quote line one","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"Quote line two","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_between_styled_spans() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Bold),
            text("Bold line one"),
            Event::SoftBreak,
            text("Bold line two"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        // Three text nodes: bold "Bold line one", bold "\n", bold "Bold line two" because
        // SoftBreak reads the currently open style stack just like any other text event.
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Bold line one","styles":{"bold":true}},{"type":"text","text":"\n","styles":{"bold":true}},{"type":"text","text":"Bold line two","styles":{"bold":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn italic_text() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Italic),
            text("Italic"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Italic","styles":{"italic":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn bold_and_italic_text() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Bold),
            start_text_style(TextStyleKind::Italic),
            text("Both"),
            Event::EndTextStyle,
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Both","styles":{"bold":true,"italic":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn heading_level_1() {
        let json = run_events(&[
            start_document(),
            start_heading(1),
            text("Title"),
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn heading_level_2() {
        let json = run_events(&[
            start_document(),
            start_heading(2),
            text("Subtitle"),
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"Subtitle","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn multiple_paragraphs() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("First"),
            Event::EndParagraph,
            start_paragraph(),
            text("Second"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"First","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"Second","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn image_block() {
        let json = run_events(&[
            start_document(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: Some("Alt text".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"https://example.com/img.png","caption":"Alt text"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn image_without_alt() {
        let json = run_events(&[
            start_document(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn mixed_content() {
        let json = run_events(&[
            start_document(),
            start_heading(1),
            text("Title"),
            Event::EndHeading,
            start_paragraph(),
            text("Body"),
            Event::EndParagraph,
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"Body","styles":{}}],"children":[]},{"type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn ignored_events() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            Event::EndBlockQuote,
            Event::LineBreak,
            Event::ThematicBreak { id: None },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[],"children":[]},{"type":"divider"}]"#
        );
    }

    #[test]
    fn blockquote_with_text() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            start_paragraph(),
            text("test"),
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"test","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn blockquote_with_styled_text() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            start_paragraph(),
            start_text_style(TextStyleKind::Bold),
            text("bold quote"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"bold quote","styles":{"bold":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn blockquote_followed_by_paragraph() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            start_paragraph(),
            text("quoted"),
            Event::EndParagraph,
            Event::EndBlockQuote,
            start_paragraph(),
            text("normal"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"quoted","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"normal","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn blockquote_multiline() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            start_paragraph(),
            text("line1"),
            Event::LineBreak,
            text("line2"),
            Event::LineBreak,
            text("line3"),
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"line1","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"line2","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"line3","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn stack_empty_after_document() {
        let json = run_events(&[
            start_document(),
            start_heading(1),
            text("Title"),
            Event::EndHeading,
            start_paragraph(),
            text("Body"),
            Event::EndParagraph,
            start_blockquote(),
            text("Quote"),
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"Body","styles":{}}],"children":[]},{"type":"quote","content":[{"type":"text","text":"Quote","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_blockquote_auto_closes_open_content() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            start_paragraph(),
            text("Quoted text"),
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"Quoted text","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn multiple_block_types_in_sequence() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("Para"),
            Event::EndParagraph,
            start_blockquote(),
            text("Quote"),
            Event::EndBlockQuote,
            start_heading(2),
            text("Head"),
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Para","styles":{}}],"children":[]},{"type":"quote","content":[{"type":"text","text":"Quote","styles":{}}],"children":[]},{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"Head","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn list_item_tracked_on_stack() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("Item"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn bullet_list_item_with_explicit_left_alignment_omits_default_prop() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_paragraph_with_alignment(TextAlignment::Left),
            text("Item"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn bullet_list_item_preserves_first_paragraph_non_default_alignment() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_paragraph_with_alignment(TextAlignment::Center),
            text("Item"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"textAlignment":"center"},"content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn numbered_list_item_preserves_first_paragraph_alignment_with_start() {
        let json = run_events(&[
            start_document(),
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(3),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            start_paragraph_with_alignment(TextAlignment::Right),
            text("Item"),
            Event::EndParagraph,
            Event::EndOrderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"numberedListItem","props":{"textAlignment":"right","start":3},"content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn single_bullet_item_emits_bullet_list_item_block() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("First bullet"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"First bullet","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn single_numbered_item_emits_numbered_list_item_block_with_start_1() {
        let json = run_events(&[
            start_document(),
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(1),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            text("First item"),
            Event::EndOrderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"numberedListItem","props":{"start":1},"content":[{"type":"text","text":"First item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn two_top_level_bullets_emit_two_sibling_blocks() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("First"),
            Event::EndUnorderedListItem,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("Second"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"First","styles":{}}],"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"Second","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_document_closes_single_open_list_item() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("x"),
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"x","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_document_with_clean_state_unchanged() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("x"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"x","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_document_with_two_consecutive_open_level_0_items_drains_both() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("a"),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("b"),
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"a","styles":{}}],"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"b","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn bullet_then_numbered_then_bullet_at_level_0() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("Bullet one"),
            Event::EndUnorderedListItem,
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(1),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            text("Number one"),
            Event::EndOrderedListItem,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("Bullet two"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"Bullet one","styles":{}}],"children":[]},{"type":"numberedListItem","props":{"start":1},"content":[{"type":"text","text":"Number one","styles":{}}],"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"Bullet two","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn bold_text_inside_bullet_list_item() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_text_style(TextStyleKind::Bold),
            text("Bold bullet"),
            Event::EndTextStyle,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"Bold bullet","styles":{"bold":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn nested_bullet_lists_emit_children_array() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("a"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("b"),
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"b","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn three_level_nesting_emits_correct_structure() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("a"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("b"),
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("c"),
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"b","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"c","styles":{}}],"children":[]}]}]}]"#
        );
    }

    #[test]
    fn nested_numbered_inside_bullet() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("bullet"),
            Event::StartOrderedListItem {
                id: None,
                level: 1,
                start: Some(1),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            text("one"),
            Event::EndOrderedListItem,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"bullet","styles":{}}],"children":[{"type":"numberedListItem","props":{"start":1},"content":[{"type":"text","text":"one","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn multiple_children_at_same_nested_level() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("a"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("b"),
            Event::EndUnorderedListItem,
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("c"),
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"b","styles":{}}],"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"c","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn orphan_end_unordered_list_item_is_silent_ok() {
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        let result = writer.handle_event(Event::EndUnorderedListItem);
        assert!(
            result.is_ok(),
            "orphan EndUnorderedListItem must be silently absorbed"
        );
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        let output = String::from_utf8_lossy(&buf);
        assert_eq!(output, "[]");
    }

    #[test]
    fn orphan_end_blockquote_is_silent_ok() {
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        let result = writer.handle_event(Event::EndBlockQuote);
        assert!(
            result.is_ok(),
            "orphan EndBlockQuote must be silently absorbed"
        );
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        let output = String::from_utf8_lossy(&buf);
        assert_eq!(output, "[]");
    }

    #[test]
    fn text_outside_block_auto_opens_paragraph() {
        let json = run_events(&[start_document(), text("Orphan"), Event::EndDocument]);
        assert_eq!(
            json,
            "[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"Orphan\",\"styles\":{}}],\"children\":[]}]"
        );
    }

    #[test]
    fn multiple_text_in_paragraph() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("Hello "),
            start_text_style(TextStyleKind::Bold),
            text("World"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Hello ","styles":{}},{"type":"text","text":"World","styles":{"bold":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn two_paragraphs_without_ids() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("First"),
            Event::EndParagraph,
            start_paragraph(),
            text("Second"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"First","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"Second","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn json_escaping_quotes() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("He said \"hello\""),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"He said \"hello\"","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn json_escaping_backslash() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("path\\to\\file"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"path\\to\\file","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn json_escaping_newline() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("line1\nline2"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"line1\nline2","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn json_escaping_tab() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("col1\tcol2"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"col1\tcol2","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn empty_paragraph() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(json, r#"[{"type":"paragraph","content":[],"children":[]}]"#);
    }

    #[test]
    fn empty_heading() {
        let json = run_events(&[
            start_document(),
            start_heading(1),
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":1},"content":[],"children":[]}]"#
        );
    }

    #[test]
    fn image_in_paragraph() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("Before"),
            Event::EndParagraph,
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Before","styles":{}}],"children":[]},{"type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn heading_then_paragraph() {
        let json = run_events(&[
            start_document(),
            start_heading(1),
            text("Title"),
            Event::EndHeading,
            start_paragraph(),
            text("Body"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"Body","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn json_escaping_carriage_return() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("line1\rline2"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"line1\rline2","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn image_url_escaping() {
        let json = run_events(&[
            start_document(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img?a=1&b=\"test\"".to_string(),
                },
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"https://example.com/img?a=1&b=\"test\"","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn end_paragraph_after_image_is_noop() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            Event::EndParagraph,
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[],"children":[]},{"type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn error_on_start_document() {
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(FailingWriter::new(0)));
        let result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        let err = result.expect_err("StartDocument must fail when writer fails immediately");
        assert_eq!(err.to_string(), "I/O error: simulated write failure");
    }

    #[test]
    fn error_on_end_document() {
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(FailingWriter::new(1)));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");
        let end_result = writer.handle_event(Event::EndDocument);
        let err =
            end_result.expect_err("EndDocument must fail when writer fails after first write");
        assert_eq!(err.to_string(), "I/O error: simulated write failure");
    }

    #[test]
    fn error_on_heading_begin_object() {
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(FailingWriter::new(1)));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");
        let heading_result = writer.handle_event(start_heading(1));
        let err =
            heading_result.expect_err("StartHeading must fail when writer fails after first write");
        assert_eq!(err.to_string(), "I/O error: simulated write failure");
    }

    #[test]
    fn error_on_paragraph_begin_object() {
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(FailingWriter::new(1)));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");
        let para_result = writer.handle_event(Event::StartParagraph {
            alignment: None,
            id: None,
        });
        let err =
            para_result.expect_err("StartParagraph must fail when writer fails after first write");
        assert_eq!(err.to_string(), "I/O error: simulated write failure");
    }

    #[test]
    fn image_with_asset_provider_success() {
        let handle = Arc::new(MockAssetHandle::new(
            "img1",
            "image/png",
            &[0x89, 0x50, 0x4E, 0x47],
        ));
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");
        let img_result = writer.handle_event(Event::Image {
            source: ImageSource::Asset(handle),
            alt: Some("Test image".to_string()),
            title: None,
            decorative: false,
            id: None,
        });
        assert!(img_result.is_ok(), "image should succeed");
        let end_result = writer.handle_event(Event::EndDocument);
        assert!(end_result.is_ok(), "end should succeed");
        let finish_result = writer.finish();
        assert!(finish_result.is_ok(), "finish should succeed");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"data:image/png;base64,iVBORw==","caption":"Test image"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn image_with_asset_not_found_content_type() {
        let handle = Arc::new(MockAssetHandle::unknown_content_type("missing"));
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");
        let result = writer.handle_event(Event::Image {
            source: ImageSource::Asset(handle),
            alt: None,
            title: None,
            decorative: false,
            id: None,
        });
        let err = result.expect_err("image with missing asset must fail");
        assert_eq!(err.to_string(), "asset not found: missing");
    }

    #[test]
    fn image_with_asset_stream_io_error() {
        let handle = Arc::new(MockAssetHandle::failing("img1", "image/png"));
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");
        let result = writer.handle_event(Event::Image {
            source: ImageSource::Asset(handle),
            alt: None,
            title: None,
            decorative: false,
            id: None,
        });
        let err = result.expect_err("image with failing stream must fail");
        assert_eq!(err.to_string(), "I/O error: mock stream failure");
    }

    #[test]
    fn writer_not_poisoned_after_mid_stream_error() {
        use docspec_core::{Event, EventSink as _, ImageSource};

        let handle = Arc::new(MockAssetHandle::failing("img1", "image/png"));
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");

        let img_result = writer.handle_event(Event::Image {
            source: ImageSource::Asset(handle),
            alt: None,
            title: None,
            decorative: false,
            id: None,
        });
        assert!(img_result.is_err(), "image with failing stream must fail");

        let end_result = writer.handle_event(Event::EndDocument);
        drop(end_result);

        let finish_result = writer.finish();
        drop(finish_result);
    }

    #[test]
    fn asset_image_jpeg() {
        let json = run_events(&[
            start_document(),
            Event::Image {
                source: ImageSource::Asset(Arc::new(MockAssetHandle::new(
                    "photo",
                    "image/jpeg",
                    &[0xFF, 0xD8, 0xFF],
                ))),
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"data:image/jpeg;base64,/9j/","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn asset_image_empty_bytes() {
        let json = run_events(&[
            start_document(),
            Event::Image {
                source: ImageSource::Asset(Arc::new(MockAssetHandle::new(
                    "empty",
                    "image/png",
                    &[],
                ))),
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"data:image/png;base64,","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn asset_and_uri_images_mixed() {
        let json = run_events(&[
            start_document(),
            Event::Image {
                source: ImageSource::Asset(Arc::new(MockAssetHandle::new(
                    "img1",
                    "image/png",
                    &[0x89, 0x50, 0x4E, 0x47],
                ))),
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"data:image/png;base64,iVBORw==","caption":""},"content":null,"children":[]},{"type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn asset_image_same_id_twice() {
        let handle: Arc<dyn AssetHandle> = Arc::new(MockAssetHandle::new(
            "img1",
            "image/png",
            &[0x89, 0x50, 0x4E, 0x47],
        ));
        let json = run_events(&[
            start_document(),
            Event::Image {
                source: ImageSource::Asset(Arc::clone(&handle)),
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::Image {
                source: ImageSource::Asset(Arc::clone(&handle)),
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"data:image/png;base64,iVBORw==","caption":""},"content":null,"children":[]},{"type":"image","props":{"url":"data:image/png;base64,iVBORw==","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn asset_image_in_paragraph() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("Before"),
            Event::EndParagraph,
            Event::Image {
                source: ImageSource::Asset(Arc::new(MockAssetHandle::new(
                    "img1",
                    "image/png",
                    &[0x89, 0x50, 0x4E, 0x47],
                ))),
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Before","styles":{}}],"children":[]},{"type":"image","props":{"url":"data:image/png;base64,iVBORw==","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn failing_writer_flush_is_ok() {
        let mut fw = FailingWriter::new(100);
        let result = fw.flush();
        assert!(result.is_ok(), "flush should succeed");
    }

    #[test]
    fn image_with_asset_stream_not_found() {
        let handle = Arc::new(MockAssetHandle::unknown_content_type("img1"));
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");
        let result = writer.handle_event(Event::Image {
            source: ImageSource::Asset(handle),
            alt: None,
            title: None,
            decorative: false,
            id: None,
        });
        let err = result.expect_err("image with no stream must fail");
        assert_eq!(err.to_string(), "asset not found: img1");
    }

    #[test]
    fn heading_with_explicit_id() {
        let json = run_events(&[
            start_document(),
            Event::StartHeading {
                level: 1,
                id: Some("custom-id".to_string()),
            },
            text("Title"),
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","id":"custom-id","props":{"level":1},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn paragraph_without_id_omits_id_key() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("Body"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"Body","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn code_block_with_language() {
        let json = run_events(&[
            start_document(),
            start_preformatted(Some("rust")),
            text("fn main() {}"),
            Event::EndPreformatted,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"codeBlock","props":{"language":"rust"},"content":[{"type":"text","text":"fn main() {}","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn code_block_without_language() {
        let json = run_events(&[
            start_document(),
            start_preformatted(None),
            text("plain code"),
            Event::EndPreformatted,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"codeBlock","content":[{"type":"text","text":"plain code","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn empty_code_block() {
        let json = run_events(&[
            start_document(),
            start_preformatted(Some("python")),
            Event::EndPreformatted,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"codeBlock","props":{"language":"python"},"content":[],"children":[]}]"#
        );
    }

    #[test]
    fn code_block_with_id() {
        let json = run_events(&[
            start_document(),
            Event::StartPreformatted {
                id: Some("cb-1".to_string()),
                syntax: None,
            },
            Event::EndPreformatted,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"codeBlock","id":"cb-1","content":[],"children":[]}]"#
        );
    }

    #[test]
    fn image_in_blockquote_emits_as_child() {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

        // > ![logo](https://example.com/logo.png)
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_blockquote()).is_ok());
        assert!(writer
            .handle_event(Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/logo.png".to_string(),
                },
                alt: Some("logo".to_string()),
                decorative: false,
                id: None,
                title: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndBlockQuote).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());

        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[],"children":[{"type":"image","props":{"url":"https://example.com/logo.png","caption":"logo"},"content":null,"children":[]}]}]"#
        );
    }

    #[test]
    fn nested_blockquote_emits_as_child() {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_blockquote()).is_ok());
        assert!(writer.handle_event(start_paragraph()).is_ok());
        assert!(writer.handle_event(text("outer")).is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        // DO NOT close outer quote - send nested StartBlockQuote directly
        assert!(writer.handle_event(start_blockquote()).is_ok());
        assert!(writer.handle_event(start_paragraph()).is_ok());
        assert!(writer.handle_event(text("inner")).is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(Event::EndBlockQuote).is_ok());
        // StackTrackingSink auto-closes outer blockquote on EndDocument
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());

        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"outer","styles":{}}],"children":[{"type":"quote","content":[{"type":"text","text":"inner","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn blockquote_with_inline_then_block_child_transitions_state() {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

        // > inline text
        // > ## block heading
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_blockquote()).is_ok());
        assert!(writer.handle_event(start_paragraph()).is_ok());
        assert!(writer.handle_event(text("inline")).is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(start_heading(2)).is_ok());
        assert!(writer.handle_event(text("block")).is_ok());
        assert!(writer.handle_event(Event::EndHeading).is_ok());
        assert!(writer.handle_event(Event::EndBlockQuote).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());

        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"inline","styles":{}}],"children":[{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"block","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn blockquote_with_inline_heading_then_paragraph_keeps_paragraph_child() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            start_paragraph(),
            text("inline"),
            Event::EndParagraph,
            start_heading(2),
            text("heading"),
            Event::EndHeading,
            start_paragraph(),
            text("after"),
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"inline","styles":{}}],"children":[{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"heading","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"after","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn heading_in_blockquote_emits_as_child() {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

        // > # Title
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_blockquote()).is_ok());
        assert!(writer.handle_event(start_heading(1)).is_ok());
        assert!(writer.handle_event(text("Title")).is_ok());
        assert!(writer.handle_event(Event::EndHeading).is_ok());
        assert!(writer.handle_event(Event::EndBlockQuote).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());

        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[],"children":[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn code_block_in_blockquote_emits_as_child() {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

        // > ```code```
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_blockquote()).is_ok());
        assert!(writer
            .handle_event(start_preformatted(Some("rust")))
            .is_ok());
        assert!(writer.handle_event(text("fn main() {}")).is_ok());
        assert!(writer.handle_event(Event::EndPreformatted).is_ok());
        assert!(writer.handle_event(Event::EndBlockQuote).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());

        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[],"children":[{"type":"codeBlock","props":{"language":"rust"},"content":[{"type":"text","text":"fn main() {}","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn image_in_heading_emits_sibling() {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

        // # ![logo](https://example.com/logo.png)
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_heading(1)).is_ok());
        assert!(writer
            .handle_event(Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/logo.png".to_string(),
                },
                alt: Some("logo".to_string()),
                decorative: false,
                id: None,
                title: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndHeading).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());

        assert_eq!(
            String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8"),
            r#"[{"type":"heading","props":{"level":1},"content":[],"children":[]},{"type":"image","props":{"url":"https://example.com/logo.png","caption":"logo"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn thematic_break_in_blockquote_emits_as_child() {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

        // > ---
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_blockquote()).is_ok());
        assert!(writer
            .handle_event(Event::ThematicBreak { id: None })
            .is_ok());
        assert!(writer.handle_event(Event::EndBlockQuote).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());

        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[],"children":[{"type":"divider"}]}]"#
        );
    }

    // ============================================================================
    // CODE/STRIKE/UNDERLINE STYLE TESTS
    // ============================================================================

    #[test]
    fn code_text() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Code),
            text("code"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"code","styles":{"code":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn strikethrough_text() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Strikethrough),
            text("struck"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"struck","styles":{"strike":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn underline_text() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Underline),
            text("underlined"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"underlined","styles":{"underline":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn combined_styles_bold_code_strikethrough() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Bold),
            start_text_style(TextStyleKind::Code),
            start_text_style(TextStyleKind::Strikethrough),
            text("combined"),
            Event::EndTextStyle,
            Event::EndTextStyle,
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"combined","styles":{"bold":true,"code":true,"strike":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn bold_style_flag_text_produces_bold_flag() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Bold),
            text("hello"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"hello","styles":{"bold":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn italic_style_flag_text_produces_italic_flag() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Italic),
            text("hello"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"hello","styles":{"italic":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn code_style_flag_text_produces_code_flag() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Code),
            text("hello"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"hello","styles":{"code":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn strikethrough_style_flag_text_produces_strike_flag() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Strikethrough),
            text("hello"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"hello","styles":{"strike":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn underline_style_flag_text_produces_underline_flag() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Underline),
            text("hello"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"hello","styles":{"underline":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn subscript_dropped_style_text_preserves_content_without_flag() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Subscript),
            text("x"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"x","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn superscript_dropped_style_text_preserves_content_without_flag() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Superscript),
            text("x"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"x","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn mark_style_emits_background_color_palette_snap() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Mark(Color::Rgb {
                r: 251,
                g: 243,
                b: 219,
            })),
            text("x"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"x","styles":{"backgroundColor":"yellow"}}],"children":[]}]"#
        );
    }

    #[test]
    fn test_blocknote_text_color_orange_exact_match() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::TextColor(Color::Rgb {
                r: 217,
                g: 115,
                b: 13,
            })),
            text("x"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"x","styles":{"textColor":"orange"}}],"children":[]}]"#
        );
    }

    #[test]
    fn test_blocknote_saturated_red_snaps_to_background_orange() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Mark(Color::Rgb {
                r: 224,
                g: 62,
                b: 62,
            })),
            text("x"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"x","styles":{"backgroundColor":"orange"}}],"children":[]}]"#
        );
    }

    #[test]
    fn test_blocknote_pure_black_text_color_filtered() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::TextColor(Color::Rgb { r: 0, g: 0, b: 0 })),
            text("x"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"x","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn test_blocknote_pure_black_mark_filtered() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Mark(Color::Rgb { r: 0, g: 0, b: 0 })),
            text("x"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"x","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn bold_plus_text_color_emits_both_keys() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Bold),
            start_text_style(TextStyleKind::TextColor(Color::Rgb {
                r: 224,
                g: 62,
                b: 62,
            })),
            text("x"),
            Event::EndTextStyle,
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"x","styles":{"bold":true,"textColor":"red"}}],"children":[]}]"#
        );
    }

    #[test]
    fn text_color_and_mark_combined_emits_both() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::TextColor(Color::Rgb {
                r: 224,
                g: 62,
                b: 62,
            })),
            start_text_style(TextStyleKind::Mark(Color::Rgb {
                r: 251,
                g: 243,
                b: 219,
            })),
            text("x"),
            Event::EndTextStyle,
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"x","styles":{"textColor":"red","backgroundColor":"yellow"}}],"children":[]}]"#
        );
    }

    #[test]
    fn text_color_arbitrary_rgb_snaps_to_palette() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::TextColor(Color::Rgb {
                r: 200,
                g: 200,
                b: 200,
            })),
            text("x"),
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"x","styles":{"textColor":"gray"}}],"children":[]}]"#
        );
    }

    #[test]
    fn nested_bold_italic_text_styles_produce_both_flags() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            start_text_style(TextStyleKind::Bold),
            start_text_style(TextStyleKind::Italic),
            text("x"),
            Event::EndTextStyle,
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"x","styles":{"bold":true,"italic":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn unbalanced_end_text_style_returns_error() {
        let result = run_events_result(&[
            start_document(),
            start_paragraph(),
            Event::EndTextStyle,
            text("x"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        let err = result.expect_err("unbalanced EndTextStyle must fail");
        assert_eq!(
            err.to_string(),
            "invalid event sequence: expected StartTextStyle, found EndTextStyle: cannot close text style because no text style is open"
        );
    }

    #[test]
    fn empty_table_emits_table_block_with_no_rows() {
        let json = run_events(&[
            start_document(),
            start_table(),
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[]},"children":[]}]"#
        );
    }

    #[test]
    fn simple_table_with_one_data_row_and_two_cells() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("Cell1"),
            Event::EndTableCell,
            start_table_cell(),
            text("Cell2"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Cell1","styles":{}}]},{"type":"tableCell","content":[{"type":"text","text":"Cell2","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn header_only_table() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            Event::StartTableHeader {
                id: None,
                scope: None,
                abbr: None,
                colspan: None,
                rowspan: None,
            },
            text("H1"),
            Event::EndTableHeader,
            Event::StartTableHeader {
                id: None,
                scope: None,
                abbr: None,
                colspan: None,
                rowspan: None,
            },
            text("H2"),
            Event::EndTableHeader,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"H1","styles":{}}]},{"type":"tableCell","content":[{"type":"text","text":"H2","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn table_cell_with_bold_text() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_text_style(TextStyleKind::Bold),
            text("bold"),
            Event::EndTextStyle,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"bold","styles":{"bold":true}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn table_cell_with_colspan_emits_props_with_colspan() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            Event::StartTableCell {
                colspan: Some(3),
                id: None,
                rowspan: None,
            },
            text("merged"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"colspan":3},"content":[{"type":"text","text":"merged","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn table_cell_with_rowspan_emits_props_with_rowspan() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            Event::StartTableCell {
                colspan: None,
                id: None,
                rowspan: Some(2),
            },
            text("merged"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"rowspan":2},"content":[{"type":"text","text":"merged","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn table_cell_with_colspan_and_rowspan_emits_both_props() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            Event::StartTableCell {
                colspan: Some(3),
                id: None,
                rowspan: Some(2),
            },
            text("merged"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"colspan":3,"rowspan":2},"content":[{"type":"text","text":"merged","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn table_header_cell_with_colspan_emits_props_with_colspan() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            Event::StartTableHeader {
                abbr: None,
                colspan: Some(2),
                id: None,
                rowspan: None,
                scope: None,
            },
            text("H"),
            Event::EndTableHeader,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"colspan":2},"content":[{"type":"text","text":"H","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn table_preceded_by_paragraph_closes_paragraph() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("before"),
            Event::EndParagraph,
            start_table(),
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"before","styles":{}}],"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[]},"children":[]}]"#
        );
    }

    #[test]
    fn table_followed_by_paragraph_opens_new_block() {
        let json = run_events(&[
            start_document(),
            start_table(),
            Event::EndTable,
            start_paragraph(),
            text("after"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[]},"children":[]},{"type":"paragraph","content":[{"type":"text","text":"after","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_table_without_start_table_is_noop() {
        // Drives EndTable when table_depth == 0 (orphan close). Covers the
        // defensive guard in handle_end_table that returns early when no
        // table is open. Bypasses StackTrackingSink to drive a hand-crafted
        // sequence the stack tracker would otherwise reject.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        let result = writer.handle_event(Event::EndTable);
        assert!(result.is_ok(), "orphan EndTable must be silently absorbed");
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(json, "[]");
    }

    #[test]
    fn nested_table_lifts_to_top_level_sibling() {
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_table()).is_ok());
        assert!(writer.handle_event(start_table_row()).is_ok());
        assert!(writer.handle_event(start_table_cell()).is_ok());
        assert!(writer.handle_event(text("outer")).is_ok());
        assert!(writer.handle_event(start_table()).is_ok());
        assert!(writer.handle_event(start_table_row()).is_ok());
        assert!(writer.handle_event(start_table_cell()).is_ok());
        assert!(writer.handle_event(text("inner")).is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"outer","styles":{}}]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"inner","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn three_deep_nested_tables_all_lift_in_document_order() {
        let json = run_direct_writer_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("c"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"c","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn multiple_nested_tables_in_same_outer_cell_lift_in_document_order() {
        let json = run_direct_writer_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("b1"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("b2"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"b1","styles":{}}]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"b2","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn text_before_and_after_nested_table_stays_in_outer_cell() {
        let json = run_direct_writer_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("before"),
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("inner"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            text("after"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"before","styles":{}},{"type":"text","text":"after","styles":{}}]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"inner","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn nested_table_inside_header_cell_lifts_to_top_level() {
        let json = run_direct_writer_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            Event::StartTableHeader {
                id: None,
                scope: None,
                abbr: None,
                colspan: None,
                rowspan: None,
            },
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("nested"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndTableHeader,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"nested","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn nested_table_preserves_styled_text_after_lift() {
        let json = run_direct_writer_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_text_style(TextStyleKind::Bold),
            text("b"),
            Event::EndTextStyle,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"b","styles":{"bold":true}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn outer_table_inside_list_item_with_nested_table_lifts_into_outer_list_item_children() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("bullet"),
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("dropped"),
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("also dropped"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"bullet","styles":{}}],"children":[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"dropped","styles":{}}]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"also dropped","styles":{}}]}]}]},"children":[]}]}]"#
        );
    }

    #[test]
    fn list_inside_table_cell_inside_list_item_lifts_into_outer_list_item_children() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("lifted"),
            Event::EndUnorderedListItem,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"lifted","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn lifted_list_under_list_item_does_not_close_outer_item() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("outer"),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("lifted"),
            Event::EndUnorderedListItem,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            start_paragraph(),
            text("after"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"outer","styles":{}}],"children":[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"lifted","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"after","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn lifted_nested_list_rebases_minimum_level_correctly() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("level 0"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("level 1"),
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("level 2"),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(4),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            text("lifted 0"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("lifted 1"),
            Event::EndUnorderedListItem,
            Event::EndOrderedListItem,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"level 0","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"level 1","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"level 2","styles":{}}],"children":[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"numberedListItem","props":{"start":4},"content":[{"type":"text","text":"lifted 0","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"lifted 1","styles":{}}],"children":[]}]}]}]}]}]"#
        );
    }

    #[test]
    fn lifted_list_into_blockquote_no_rebasing() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("outer"),
            start_blockquote(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("quoted lifted"),
            Event::EndUnorderedListItem,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndBlockQuote,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"outer","styles":{}}],"children":[{"type":"quote","content":[],"children":[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"quoted lifted","styles":{}}],"children":[]}]}]}]"#
        );
    }

    #[test]
    fn heading_inside_table_cell_inside_blockquote_lifts_into_blockquote_children() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_heading(2),
            text("lifted heading"),
            Event::EndHeading,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[],"children":[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"lifted heading","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn image_inside_cell_inside_blockquote_inside_list_item_lifts_into_blockquote() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_blockquote(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/in-quote.png".to_string(),
                },
                alt: Some("in quote".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndBlockQuote,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"quote","content":[],"children":[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/in-quote.png","caption":"in quote"},"content":null,"children":[]}]}]}]"#
        );
    }

    #[test]
    fn image_inside_cell_inside_list_item_inside_blockquote_lifts_into_list_item() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/in-list.png".to_string(),
                },
                alt: Some("in list".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndUnorderedListItem,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[],"children":[{"type":"bulletListItem","content":[],"children":[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/in-list.png","caption":"in list"},"content":null,"children":[]}]}]}]"#
        );
    }

    #[test]
    fn blockquote_inside_table_cell_inside_list_item_lifts_into_outer_list_item_children() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_blockquote(),
            text("lifted quote"),
            Event::EndBlockQuote,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"quote","content":[{"type":"text","text":"lifted quote","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn thematic_break_with_id_emits_id_field() {
        let json = run_events(&[
            start_document(),
            Event::ThematicBreak {
                id: Some("hr-1".to_string()),
            },
            Event::EndDocument,
        ]);
        assert_eq!(json, r#"[{"type":"divider","id":"hr-1"}]"#);
    }

    #[test]
    fn image_with_id_emits_id_field() {
        let json = run_events(&[
            start_document(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: None,
                title: None,
                decorative: false,
                id: Some("img-1".to_string()),
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"id":"img-1","type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn end_preformatted_without_open_block_is_noop() {
        // Drives EndPreformatted when in_text_block == false (no open preformatted
        // block). Covers the guard that returns early when the close has no
        // matching open. Bypasses StackTrackingSink since the stack tracker
        // rejects orphan close events.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        let result = writer.handle_event(Event::EndPreformatted);
        assert!(
            result.is_ok(),
            "orphan EndPreformatted must be silently absorbed"
        );
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(json, "[]");
    }

    #[test]
    fn image_inside_table_cell_is_lifted_after_table() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: Some("in cell".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/img.png","caption":"in cell"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn multiple_images_in_same_cell_lift_in_document_order() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/a.png".to_string(),
                },
                alt: Some("A".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/b.png".to_string(),
                },
                alt: Some("B".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/a.png","caption":"A"},"content":null,"children":[]},{"type":"image","props":{"url":"https://example.com/b.png","caption":"B"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn images_in_different_cells_lift_in_document_order() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/cell1.png".to_string(),
                },
                alt: Some("cell1".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableCell,
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/cell2.png".to_string(),
                },
                alt: Some("cell2".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]},{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/cell1.png","caption":"cell1"},"content":null,"children":[]},{"type":"image","props":{"url":"https://example.com/cell2.png","caption":"cell2"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn text_around_image_in_cell_stays_in_cell_while_image_lifts() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("before"),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: Some("middle".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            text("after"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"before","styles":{}},{"type":"text","text":"after","styles":{}}]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/img.png","caption":"middle"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn asset_image_in_cell_lifts_as_data_uri_block() {
        let handle = Arc::new(MockAssetHandle::new(
            "png1",
            "image/png",
            &[0x89, 0x50, 0x4E, 0x47],
        ));
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Asset(handle),
                alt: Some("png".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"data:image/png;base64,iVBORw==","caption":"png"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn image_id_in_cell_survives_lift() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: None,
                title: None,
                decorative: false,
                id: Some("img-99".to_string()),
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"id":"img-99","type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn image_inside_nested_table_cell_lifts_after_nested_table() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/nested.png".to_string(),
                },
                alt: Some("nested".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/nested.png","caption":"nested"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn outer_cell_image_and_nested_cell_image_lift_in_document_order() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/outer.png".to_string(),
                },
                alt: Some("outer".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/nested.png".to_string(),
                },
                alt: Some("nested".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/outer.png","caption":"outer"},"content":null,"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/nested.png","caption":"nested"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn image_inside_header_cell_lifts_after_table() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            Event::StartTableHeader {
                id: None,
                scope: None,
                abbr: None,
                colspan: None,
                rowspan: None,
            },
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/header.png".to_string(),
                },
                alt: Some("header".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableHeader,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/header.png","caption":"header"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn two_separate_tables_each_lift_their_own_in_cell_image() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/t1.png".to_string(),
                },
                alt: Some("t1".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/t2.png".to_string(),
                },
                alt: Some("t2".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/t1.png","caption":"t1"},"content":null,"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/t2.png","caption":"t2"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn image_inside_paragraph_inside_cell_lifts_after_table() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_paragraph(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/wrapped.png".to_string(),
                },
                alt: Some("wrapped".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndParagraph,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"image","props":{"url":"https://example.com/wrapped.png","caption":"wrapped"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn numbered_list_item_with_start_5_emits_start_prop() {
        let json = run_events(&[
            start_document(),
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(5),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            text("Item"),
            Event::EndOrderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"numberedListItem","props":{"start":5},"content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn numbered_list_item_with_no_start_omits_start_prop() {
        let json = run_events(&[
            start_document(),
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: None,
                style_type: docspec_core::ListStyleType::Decimal,
            },
            text("Item"),
            Event::EndOrderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"numberedListItem","content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn unordered_list_item_never_emits_start_prop() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("Item"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn list_item_with_id_drops_id_field() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: Some("item-1".to_string()),
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("Item"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn list_item_without_id_omits_id_key() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("Item"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn list_inside_table_cell_lifts_after_table() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("dropped"),
            Event::EndUnorderedListItem,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"dropped","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn list_inside_blockquote_emits_as_child() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("quoted bullet"),
            Event::EndUnorderedListItem,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"quoted bullet","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn consecutive_same_level_list_items_inside_blockquote_are_siblings() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("first"),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("second"),
            Event::EndUnorderedListItem,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"second","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn nested_table_with_list_in_cell_lifts_table_and_list_after_outer_table() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("outer"),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("inner dropped"),
            Event::EndUnorderedListItem,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"outer","styles":{}}]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"inner dropped","styles":{}}],"children":[]}]"#
        );
    }

    // ============================================================================
    // T10: LEVEL-DOWN TRANSITIONS AND LEVEL-JUMP CLAMPING
    // ============================================================================

    #[test]
    fn level_two_to_zero_drops_three_levels_correctly() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("a"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("b"),
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("c"),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("d"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"b","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"c","styles":{}}],"children":[]}]}]},{"type":"bulletListItem","content":[{"type":"text","text":"d","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn level_two_to_one_drops_one_level() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("a"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("b"),
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("c"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("d"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"b","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"c","styles":{}}],"children":[]}]},{"type":"bulletListItem","content":[{"type":"text","text":"d","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn programmatic_level_jump_0_to_2_clamps_to_1() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("a"),
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("b"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"b","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn explicit_end_then_level_down_works() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("a"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("b"),
            Event::EndUnorderedListItem,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("c"),
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"b","styles":{}}],"children":[]}]},{"type":"bulletListItem","content":[{"type":"text","text":"c","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_document_with_nested_open_items_drains_in_reverse_order() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("a"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("b"),
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("c"),
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"b","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"c","styles":{}}],"children":[]}]}]}]"#
        );
    }

    // ============================================================================
    // T11: PARAGRAPH-INSIDE-LIST-ITEM DISPATCH
    // ============================================================================

    #[test]
    fn single_paragraph_item_inline_content_only() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("Hello"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"Hello","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn multi_paragraph_item_first_inline_rest_as_children() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("Para one"),
            start_paragraph(),
            text("Para two"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"Para one","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"Para two","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn nested_item_inherits_paragraph_dispatch() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("a"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("b1"),
            start_paragraph(),
            text("b2"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"b1","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"b2","styles":{}}],"children":[]}]}]}]"#
        );
    }

    #[test]
    fn three_paragraphs_in_item() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("Para one"),
            start_paragraph(),
            text("Para two"),
            Event::EndParagraph,
            start_paragraph(),
            text("Para three"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"Para one","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"Para two","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"Para three","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn list_immediately_after_blockquote_with_no_intervening_text_emits_at_top_level() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            text("Quote"),
            Event::EndBlockQuote,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("after quote"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"Quote","styles":{}}],"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"after quote","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn heading_inside_list_item_emits_as_child() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_heading(1),
            text("h"),
            Event::EndHeading,
            text("item"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"h","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"item","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn image_inside_list_item_emits_as_child() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]}]"#
        );
    }

    #[test]
    fn code_block_inside_list_item_emits_as_child() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_preformatted(None),
            text("code"),
            Event::EndPreformatted,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"codeBlock","content":[{"type":"text","text":"code","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn nested_blockquote_inside_list_item_emits_as_child() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_blockquote(),
            text("q"),
            Event::EndBlockQuote,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"quote","content":[{"type":"text","text":"q","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn divider_inside_list_item_emits_as_child() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::ThematicBreak { id: None },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"divider"}]}]"#
        );
    }

    #[test]
    fn table_inside_list_item_emits_as_child() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("cell"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"cell","styles":{}}]}]}]},"children":[]}]}]"#
        );
    }

    #[test]
    fn text_after_block_child_in_list_item_appears_after_child() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("before"),
            start_heading(1),
            text("dropped"),
            Event::EndHeading,
            text("after"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"before","styles":{}}],"children":[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"dropped","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"after","styles":{}}],"children":[]}]}]"#
        );
    }

    // ============================================================================
    // T14: COVERAGE GAP FILLS
    // ============================================================================

    #[test]
    fn heading_inside_table_cell_lifts_after_table() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_heading(1),
            text("h"),
            Event::EndHeading,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"h","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn blockquote_inside_table_cell_lifts_after_table() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_blockquote(),
            text("q"),
            Event::EndBlockQuote,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"quote","content":[{"type":"text","text":"q","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn preformatted_inside_table_cell_lifts_after_table() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_preformatted(None),
            text("code"),
            Event::EndPreformatted,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"codeBlock","content":[{"type":"text","text":"code","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn line_break_inside_heading_in_list_item_preserved_in_child() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_heading(1),
            text("head"),
            Event::LineBreak,
            text("more"),
            Event::EndHeading,
            text("item"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"head","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"more","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"item","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn ordered_list_start_value_overflow_returns_error() {
        // Drives the u32::try_from error path in open_list_item_object when the
        // start value exceeds u32::MAX (4,294,967,295). BlockNote's start prop is a u32.
        // Uses BlockNoteWriter directly to bypass StackTrackingSink validation.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        let result = writer.handle_event(Event::StartOrderedListItem {
            id: None,
            level: 0,
            start: Some(u64::from(u32::MAX) + 1),
            style_type: docspec_core::ListStyleType::Decimal,
        });
        let err = result.expect_err("start value exceeding u32::MAX must return an error");
        assert_eq!(
            err.to_string(),
            "ordered list start value out of range: 4294967296: out of range integral type conversion attempted"
        );
    }

    #[test]
    fn paragraph_after_closed_list_item_closes_remaining_stack() {
        // Drives close_open_list_items via handle_paragraph when list_stack is
        // non-empty but the top entry's content_array is closed and
        // first_paragraph_consumed is false (empty item + explicit EndListItem).
        // Verifies that the closed list item is properly finalised and the following
        // paragraph is emitted at the top level.
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::EndUnorderedListItem,
            start_paragraph(),
            text("after"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"after","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn second_paragraph_in_blockquote_emits_separator() {
        // Drives handle_paragraph when blockquote_depth > 0 and blockquote_has_content
        // is true. The separator path emits a "\n\n" text node between the two
        // paragraphs' content so block-quote paragraphs are visually separated.
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            text("first"),
            start_paragraph(),
            text("second"),
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"first","styles":{}},{"type":"text","text":"\n\n","styles":{}},{"type":"text","text":"second","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_heading_without_open_heading_is_noop() {
        // Drives EndHeading when in_text_block is false and drop_inside_list_depth
        // is zero — the !in_text_block guard returns Ok(()) silently.
        // Uses BlockNoteWriter directly because StackTrackingSink rejects orphan
        // End events.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        let result = writer.handle_event(Event::EndHeading);
        assert!(
            result.is_ok(),
            "orphan EndHeading must be silently absorbed"
        );
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        let json = String::from_utf8(buf).expect("output must be valid UTF-8");
        assert_eq!(json, "[]");
    }

    #[test]
    fn text_after_image_inside_paragraph_auto_opens_new_paragraph() {
        // Drives handle_text_event's auto-open paragraph guard (in_text_block=false
        // after Image closes the current paragraph via close_for_block_sibling).
        // StackTrackingSink keeps Paragraph on its own stack but BlockNoteWriter has
        // already closed the paragraph — the next Text call triggers a new implicit
        // paragraph open.
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("before"),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            text("after"),
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"before","styles":{}}],"children":[]},{"type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]},{"type":"paragraph","content":[{"type":"text","text":"after","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn all_catch_all_events_are_silently_ignored_by_writer() {
        // Drives every branch of the wildcard catch-all arm in handle_event.
        // Events with no BlockNote equivalent (captions, definitions, footnotes,
        // links) must all return Ok(()) without emitting any JSON. Uses
        // BlockNoteWriter directly to bypass StackTrackingSink validation of
        // stack-order constraints on orphan End events.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        for event in [
            Event::EndCaption,
            Event::EndDefinitionDetail,
            Event::EndDefinitionList,
            Event::EndDefinitionTerm,
            Event::EndFootnote,
            Event::FootnoteRef { id: 1 },
            Event::StartCaption { id: None },
            Event::StartDefinitionDetail { id: None },
            Event::StartDefinitionList { id: None },
            Event::StartDefinitionTerm { id: None },
            Event::StartFootnote { id: 1 },
        ] {
            assert!(
                writer.handle_event(event).is_ok(),
                "catch-all events must return Ok(())"
            );
        }
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        let json = String::from_utf8(buf).expect("output must be valid UTF-8");
        assert_eq!(json, "[]", "no JSON output from catch-all events");
    }

    #[test]
    fn end_heading_closes_open_heading_block() {
        // Drives close_text_block! in the EndHeading arm (line 853) via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_heading(2)).is_ok());
        assert!(writer.handle_event(text("Heading text")).is_ok());
        assert!(writer.handle_event(Event::EndHeading).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"Heading text","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_preformatted_closes_open_code_block() {
        // Drives close_text_block! in EndPreformatted arm (line 861) via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer
            .handle_event(start_preformatted(Some("rust")))
            .is_ok());
        assert!(writer.handle_event(text("let x = 1;")).is_ok());
        assert!(writer.handle_event(Event::EndPreformatted).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"codeBlock","props":{"language":"rust"},"content":[{"type":"text","text":"let x = 1;","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn thematic_break_inside_table_cell_lifts_after_table() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::ThematicBreak { id: None },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"divider"}]"#
        );
    }

    #[test]
    fn multiple_block_kinds_in_same_cell_all_lift_in_order() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_heading(1),
            text("H"),
            Event::EndHeading,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("item"),
            Event::EndUnorderedListItem,
            start_blockquote(),
            text("Q"),
            Event::EndBlockQuote,
            Event::ThematicBreak { id: None },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"H","styles":{}}],"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"item","styles":{}}],"children":[]},{"type":"quote","content":[{"type":"text","text":"Q","styles":{}}],"children":[]},{"type":"divider"}]"#
        );
    }

    #[test]
    fn cell_with_only_lifted_content_has_empty_content_array() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_heading(2),
            text("title"),
            Event::EndHeading,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"title","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn text_inside_lifted_heading_in_cell_preserved_after_lift() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            start_heading(2),
            text("Inner"),
            Event::EndHeading,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"Inner","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_paragraph_in_normal_context_closes_paragraph_block() {
        // Drives close_text_block! in handle_end_paragraph normal path (line 344)
        // via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_paragraph()).is_ok());
        assert!(writer.handle_event(text("plain text")).is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"plain text","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn level_down_to_nested_parent_breaks_loop() {
        // Drives the break statement (line 592) in the level-down while-loop of
        // handle_start_list_item: level-0 → level-1 → level-2 → back to level-1.
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("a"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("b"),
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("c"),
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("d"),
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"b","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"c","styles":{}}],"children":[]}]},{"type":"bulletListItem","content":[{"type":"text","text":"d","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn multi_paragraph_list_item_raw_covers_second_para_paths() {
        // Drives handle_paragraph second-para path (lines 477, 488) and
        // handle_end_paragraph second-para-close path (line 324) via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer.handle_event(start_paragraph()).is_ok());
        assert!(writer.handle_event(text("first")).is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(start_paragraph()).is_ok());
        assert!(writer.handle_event(text("second")).is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn open_current_list_item_children_state_paths() {
        // Drives open_current_list_item_children with content_array_open=true (line 726)
        // and children_array_open=false (line 737) via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer.handle_event(text("parent")).is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer.handle_event(text("child")).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"parent","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"child","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn open_list_item_object_with_valid_start_value() {
        // Drives u32::try_from success path for the ordered-list `start` prop
        // and ListStackEntry push fields in open_list_item_object.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer
            .handle_event(Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(42),
                style_type: docspec_core::ListStyleType::Decimal,
            })
            .is_ok());
        assert!(writer.handle_event(text("item 42")).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"numberedListItem","props":{"start":42},"content":[{"type":"text","text":"item 42","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn image_event_with_alt_and_id_fields() {
        // Drives Image match arm pattern bindings (lines 900-901) via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer
            .handle_event(Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/photo.jpg".to_string(),
                },
                alt: Some("alt text".to_string()),
                title: None,
                decorative: false,
                id: Some("img-1".to_string()),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"id":"img-1","type":"image","props":{"url":"https://example.com/photo.jpg","caption":"alt text"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn ordered_list_item_with_id_and_start_fields() {
        // Drives StartOrderedListItem match arm pattern bindings (lines 904-905) via
        // raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer
            .handle_event(Event::StartOrderedListItem {
                id: Some("li-1".to_string()),
                level: 0,
                start: Some(5),
                style_type: docspec_core::ListStyleType::Decimal,
            })
            .is_ok());
        assert!(writer.handle_event(text("five")).is_ok());
        assert!(writer.handle_event(Event::EndOrderedListItem).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"numberedListItem","props":{"start":5},"content":[{"type":"text","text":"five","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn each_catch_all_variant_fired_individually() {
        // Drives each OR arm of the wildcard catch-all arm (lines 919-931) in isolation.
        for event in [
            Event::EndCaption,
            Event::EndDefinitionDetail,
            Event::EndDefinitionList,
            Event::EndDefinitionTerm,
            Event::EndFootnote,
            Event::FootnoteRef { id: 99 },
            Event::StartCaption {
                id: Some("c".to_string()),
            },
            Event::StartDefinitionDetail { id: None },
            Event::StartDefinitionList { id: None },
            Event::StartDefinitionTerm { id: None },
            Event::StartFootnote { id: 7 },
        ] {
            let mut buf = Vec::<u8>::new();
            let mut writer = BlockNoteWriter::new(&mut buf);
            assert!(writer.handle_event(start_document()).is_ok());
            assert!(
                writer.handle_event(event).is_ok(),
                "catch-all event must return Ok(())"
            );
            assert!(writer.handle_event(Event::EndDocument).is_ok());
            assert!(writer.finish().is_ok());
            let json = String::from_utf8(buf).expect("output must be valid UTF-8");
            assert_eq!(json, "[]", "catch-all events must not emit JSON");
        }
    }

    #[test]
    fn all_block_types_inside_list_item_emit_as_children() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("before"),
            start_heading(1),
            text("heading"),
            Event::EndHeading,
            start_preformatted(None),
            text("code"),
            Event::EndPreformatted,
            start_blockquote(),
            text("quote"),
            Event::EndBlockQuote,
            Event::ThematicBreak { id: None },
            text("after"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"before","styles":{}}],"children":[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"heading","styles":{}}],"children":[]},{"type":"codeBlock","content":[{"type":"text","text":"code","styles":{}}],"children":[]},{"type":"quote","content":[{"type":"text","text":"quote","styles":{}}],"children":[]},{"type":"divider"},{"type":"paragraph","content":[{"type":"text","text":"after","styles":{}}],"children":[]}]}]"#
        );
    }

    fn list_item_with_children_transition_then(block_events: Vec<Event>) -> String {
        let mut events = vec![
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_paragraph(),
            text("first"),
            Event::EndParagraph,
            start_paragraph(),
            text("second"),
            Event::EndParagraph,
        ];
        events.extend(block_events);
        events.push(Event::EndUnorderedListItem);
        events.push(Event::EndDocument);
        run_events(&events)
    }

    #[test]
    fn image_after_children_transition_inside_list_item_emits_as_child() {
        let json = list_item_with_children_transition_then(vec![Event::Image {
            source: ImageSource::Uri {
                uri: "https://example.com/leaked.png".to_string(),
            },
            alt: Some("leaked".to_string()),
            title: None,
            decorative: false,
            id: None,
        }]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]},{"type":"image","props":{"url":"https://example.com/leaked.png","caption":"leaked"},"content":null,"children":[]}]}]"#
        );
    }

    #[test]
    fn thematic_break_after_children_transition_inside_list_item_emits_as_child() {
        let json = list_item_with_children_transition_then(vec![Event::ThematicBreak { id: None }]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]},{"type":"divider"}]}]"#
        );
    }

    #[test]
    fn heading_after_children_transition_inside_list_item_emits_as_child() {
        let json = list_item_with_children_transition_then(vec![
            start_heading(2),
            text("leaked-heading"),
            Event::EndHeading,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]},{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"leaked-heading","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn blockquote_after_children_transition_inside_list_item_emits_as_child() {
        let json = list_item_with_children_transition_then(vec![
            start_blockquote(),
            text("leaked-quote"),
            Event::EndBlockQuote,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]},{"type":"quote","content":[{"type":"text","text":"leaked-quote","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn preformatted_after_children_transition_inside_list_item_emits_as_child() {
        let json = list_item_with_children_transition_then(vec![
            start_preformatted(None),
            text("leaked-code"),
            Event::EndPreformatted,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]},{"type":"codeBlock","content":[{"type":"text","text":"leaked-code","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn table_after_children_transition_inside_list_item_emits_as_child() {
        let json = list_item_with_children_transition_then(vec![
            start_table(),
            start_table_row(),
            start_table_cell(),
            text("leaked-cell"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"leaked-cell","styles":{}}]}]}]},"children":[]}]}]"#
        );
    }

    #[test]
    fn list_item_with_multiple_block_children_emits_all_in_order() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_heading(2),
            text("a"),
            Event::EndHeading,
            start_paragraph(),
            text("b"),
            Event::EndParagraph,
            start_blockquote(),
            start_paragraph(),
            text("c"),
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"a","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"b","styles":{}}],"children":[]},{"type":"quote","content":[{"type":"text","text":"c","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn ordered_list_item_with_explicit_start_emits_start_prop() {
        // Exercises line 771: start prop write for ordered list items
        let json = run_events(&[
            start_document(),
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(3),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            text("item"),
            Event::EndOrderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"numberedListItem","props":{"start":3},"content":[{"type":"text","text":"item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn multi_paragraph_list_item_second_paragraph_dispatch() {
        // Exercises lines 477, 488, 324, 344 in handle_paragraph and handle_end_paragraph
        // for the second-and-subsequent paragraph case. Direct event emission without
        // StackTrackingSink to isolate the dispatch logic.
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer.handle_event(start_paragraph()).is_ok());
        assert!(writer.handle_event(text("first")).is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(start_paragraph()).is_ok());
        assert!(writer.handle_event(text("second")).is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(Event::EndUnorderedListItem).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn image_after_end_list_item_appears_as_top_level_sibling() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("item"),
            Event::EndUnorderedListItem,
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/foo.png".to_string(),
                },
                alt: Some("Foo".to_string()),
                title: None,
                decorative: false,
                id: None,
            },
            Event::ThematicBreak { id: None },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"item","styles":{}}],"children":[]},{"type":"image","props":{"url":"https://example.com/foo.png","caption":"Foo"},"content":null,"children":[]},{"type":"divider"}]"#
        );
    }

    #[test]
    fn heading_after_end_list_item_appears_as_top_level_sibling() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("item"),
            Event::EndUnorderedListItem,
            start_heading(2),
            text("After list"),
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"item","styles":{}}],"children":[]},{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"After list","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn list_item_inside_blockquote_child_of_list_item_nests_in_blockquote() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("outer"),
            start_blockquote(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("inner"),
            Event::EndUnorderedListItem,
            Event::EndBlockQuote,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"outer","styles":{}}],"children":[{"type":"quote","content":[],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"inner","styles":{}}],"children":[]}]}]}]"#
        );
    }

    #[test]
    fn paragraph_inside_blockquote_child_of_list_item_renders_in_blockquote() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_blockquote(),
            start_paragraph(),
            text("quoted"),
            Event::EndParagraph,
            Event::EndBlockQuote,
            start_paragraph(),
            text("real"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"quote","content":[{"type":"text","text":"quoted","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"real","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn continuation_paragraph_after_nested_list_attaches_to_parent_item() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_paragraph(),
            text("outer"),
            Event::EndParagraph,
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_paragraph(),
            text("nested"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            start_paragraph(),
            text("continuation"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"outer","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"nested","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"continuation","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn parent_paragraph_after_nested_list_without_initial_text_is_preserved_as_child() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("nested"),
            Event::EndUnorderedListItem,
            start_paragraph(),
            text("after"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"nested","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"after","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn heading_child_before_aligned_paragraph_child_in_list_item() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_heading(1),
            text("dropped"),
            Event::EndHeading,
            start_paragraph_with_alignment(TextAlignment::Center),
            text("real"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"dropped","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"center"},"content":[{"type":"text","text":"real","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn heading_child_with_link_before_aligned_paragraph_child_in_list_item() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_heading(1),
            Event::StartLink {
                href: "https://dropped.example".to_string(),
                title: None,
                id: None,
            },
            text("dropped"),
            Event::EndLink,
            Event::EndHeading,
            start_paragraph_with_alignment(TextAlignment::Right),
            text("real"),
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"heading","props":{"level":1},"content":[{"type":"link","href":"https://dropped.example","content":[{"type":"text","text":"dropped","styles":{}}]}],"children":[]},{"type":"paragraph","props":{"textAlignment":"right"},"content":[{"type":"text","text":"real","styles":{}}],"children":[]}]}]"#
        );
    }

    // ============================================================================
    // LINK TESTS
    // ============================================================================

    #[test]
    fn link_simple() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            text("text"),
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"text","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn nested_start_link_is_silently_ignored() {
        let json = run_direct_writer_events(&[
            start_document(),
            start_paragraph(),
            Event::StartLink {
                href: "https://a.example".to_string(),
                title: None,
                id: None,
            },
            Event::StartLink {
                href: "https://b.example".to_string(),
                title: None,
                id: None,
            },
            text("inner"),
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"link","href":"https://a.example","content":[{"type":"text","text":"inner","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn link_left_open_at_paragraph_end_is_defensively_closed() {
        let json = run_direct_writer_events(&[
            start_document(),
            start_paragraph(),
            Event::StartLink {
                href: "https://x.example".to_string(),
                title: None,
                id: None,
            },
            text("label"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"link","href":"https://x.example","content":[{"type":"text","text":"label","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn link_left_open_at_table_cell_end_is_defensively_closed() {
        let json = run_direct_writer_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::StartLink {
                href: "https://cell.example".to_string(),
                title: None,
                id: None,
            },
            text("cell"),
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"link","href":"https://cell.example","content":[{"type":"text","text":"cell","styles":{}}]}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn link_empty_content_emits_empty_styled_text() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn link_drops_title_field() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: Some("a title".to_string()),
                id: None,
            },
            text("text"),
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"text","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn link_with_styled_content_array() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            start_text_style(TextStyleKind::Bold),
            text("bold"),
            Event::EndTextStyle,
            start_text_style(TextStyleKind::Italic),
            text("italic"),
            Event::EndTextStyle,
            text("plain"),
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"bold","styles":{"bold":true}},{"type":"text","text":"italic","styles":{"italic":true}},{"type":"text","text":"plain","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn link_in_paragraph_alongside_other_text() {
        let json = run_events(&[
            start_document(),
            start_paragraph(),
            text("before "),
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            text("link"),
            Event::EndLink,
            text(" after"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"before ","styles":{}},{"type":"link","href":"https://example.com","content":[{"type":"text","text":"link","styles":{}}]},{"type":"text","text":" after","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn link_in_heading() {
        let json = run_events(&[
            start_document(),
            start_heading(1),
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            text("title link"),
            Event::EndLink,
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":1},"content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"title link","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn link_in_list_item() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            text("link"),
            Event::EndLink,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"link","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn link_in_blockquote() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            start_paragraph(),
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            text("link"),
            Event::EndLink,
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"link","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn empty_link_in_blockquote() {
        let json = run_events(&[
            start_document(),
            start_blockquote(),
            start_paragraph(),
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            Event::EndLink,
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn link_in_table_cell() {
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            text("link"),
            Event::EndLink,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"link","styles":{}}]}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn link_in_heading_child_of_list_item_emits_in_heading_content() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_heading(1),
            Event::StartLink {
                href: "https://x".to_string(),
                title: None,
                id: None,
            },
            text("hidden"),
            Event::EndLink,
            Event::EndHeading,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[{"type":"heading","props":{"level":1},"content":[{"type":"link","href":"https://x","content":[{"type":"text","text":"hidden","styles":{}}]}],"children":[]}]}]"#
        );
    }

    #[test]
    fn streaming_writes_never_exceed_8kb() {
        use docspec_core::{Event, EventSink as _, ImageSource};
        use std::io::Write;

        struct CountingWriter {
            inner: Vec<u8>,
            max_single_write: usize,
            total_writes: usize,
        }
        impl Write for CountingWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.max_single_write = self.max_single_write.max(buf.len());
                self.total_writes += 1;
                self.inner.extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let payload: Vec<u8> = (0..(1024 * 1024))
            .map(|i: usize| u8::try_from(i.rem_euclid(256)).unwrap_or(0))
            .collect();
        let payload_len = payload.len();
        let handle = Arc::new(MockAssetHandle::new("big", "image/png", &payload));
        let mut counting = CountingWriter {
            inner: Vec::new(),
            max_single_write: 0,
            total_writes: 0,
        };

        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut counting));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");

        let img_result = writer.handle_event(Event::Image {
            source: ImageSource::Asset(handle),
            alt: Some("Large image".to_string()),
            title: None,
            decorative: false,
            id: None,
        });
        assert!(img_result.is_ok(), "image should succeed");

        let end_result = writer.handle_event(Event::EndDocument);
        assert!(end_result.is_ok(), "end should succeed");

        let finish_result = writer.finish();
        assert!(finish_result.is_ok(), "finish should succeed");

        assert!(
             counting.max_single_write < 16 * 1024,
             "expected streaming (max write < 16KB), got max single write of {} bytes (total payload {})",
             counting.max_single_write, payload_len
         );
        assert!(
            counting.total_writes > 64,
            "expected many small writes for 1MB payload; got {} writes",
            counting.total_writes
        );
    }

    #[test]
    fn sample_docx_lifts_list_from_requirements_cell() {
        let bytes = std::fs::read("tests/fixtures/sample.docx").expect("fixture file");
        let reader = docspec_docx_reader::DocxReader::from_reader(std::io::Cursor::new(bytes))
            .expect("reader creation");
        let mut buf = Vec::<u8>::new();
        let writer = BlockNoteWriter::new(&mut buf);
        let sink = StackTrackingSink::new(writer);
        docspec_core::pipe(reader, sink).expect("pipe");
        let actual_json = String::from_utf8(buf).expect("valid utf-8");
        const EXPECTED_JSON: &str = r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"colspan":2},"content":[{"type":"text","text":"Project properties","styles":{"bold":true}},{"type":"text","text":" ","styles":{"bold":true}}]}]},{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Title","styles":{"bold":true}}]},{"type":"tableCell","content":[{"type":"text","text":"Mapping hybrid governance ","styles":{}},{"type":"text","text":"for sustainable global value chains","styles":{}}]}]},{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Group","styles":{"bold":true}}]},{"type":"tableCell","content":[{"type":"text","text":"PAP","styles":{}}]}]},{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Project type","styles":{"bold":true}}]},{"type":"tableCell","content":[{"type":"text","text":"Master thesis ","styles":{}}]}]},{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Credits","styles":{"bold":true}}]},{"type":"tableCell","content":[{"type":"text","text":"18-24","styles":{}}]}]},{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Supervisor(s)","styles":{"bold":true}}]},{"type":"tableCell","content":[{"type":"text","text":"Dr.","styles":{}},{"type":"text","text":" Otto. Hospes","styles":{}}]}]},{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Examiner(s)","styles":{"bold":true}}]},{"type":"tableCell","content":[{"type":"text","text":"Prof.","styles":{}},{"type":"text","text":" Katrien Termeer and ","styles":{}},{"type":"text","text":"Dr.","styles":{}},{"type":"text","text":" Otto Hospes","styles":{}}]}]},{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Contact info","styles":{"bold":true}}]},{"type":"tableCell","content":[{"type":"text","text":"Dr.","styles":{}},{"type":"text","text":" Otto Hospes","styles":{}}]}]},{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Begin date","styles":{"bold":true}}]},{"type":"tableCell","content":[{"type":"text","text":"asap","styles":{}}]}]},{"cells":[{"type":"tableCell","content":[{"type":"text","text":"End date","styles":{"bold":true}}]},{"type":"tableCell","content":[{"type":"text","text":"November 2016","styles":{}}]}]},{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Description","styles":{"bold":true}}]},{"type":"tableCell","content":[{"type":"text","text":"This master thesis project is part of a larger (PhD) project that examines the potential for developing synergies between public and private governance for sustainable global value chains (GVCs). A central aim of the PhD project is to develop innovative governance arrangements with public authorities in both producing/exporting and importing countries.  ","styles":{}},{"type":"text","text":"Private governance initiatives are considered more effective than state-led initiatives in addressing environmental and social problems in global value chains. However, these private initiatives are increasingly criticised for their limitations in addressing land conflicts and smallholder concerns, their bias towards a single-commodity approach and their lack of area-based governance. These criticisms are paralleled by an increasing role of public actors in developing public sustainability schemes or quasi-accreditation policies for private standards. While currently these public and private initiatives often exist next to each other or even compete, there is great potential for synergies because public and private actors can complement each other’s roles in GVCs. ","styles":{}},{"type":"text","text":"The first objective of the master thesis project is to map different forms of h","styles":{}},{"type":"text","text":"ybrid governance of global value chains. ","styles":{}},{"type":"text","text":"For this purpose the following preliminary classification can be tested and adjusted: a) private certification programmes with government involvement through official recognition of the standard; b) social-private partnerships (roundtables) for specific commodities with limited government involvement through subsidies; c) public-private partnerships with direct involvement of public authorities; d) public-private value chain initiatives with explicit linkages with area-based public policies in the producing country. The second objective is to ","styles":{}},{"type":"text","text":"make an inventory and classification of ","styles":{}},{"type":"text","text":"different scientific concepts that are used ","styles":{}},{"type":"text","text":"by scholars ","styles":{}},{"type":"text","text":"to understand and analyse hybrid governance forms, arrangements and interactions  involving public and private actors.","styles":{}},{"type":"text","text":" ","styles":{}},{"type":"text","text":"The third objective is to conduct a quick scan of the underlying motives and perceived challenges and obstacles for organizing synergies between private and public forms of governance. ","styles":{}},{"type":"text","text":"Data collection and methods consists of three steps: ","styles":{}},{"type":"text","text":"1. ","styles":{}},{"type":"text","text":"systematic literature review","styles":{}},{"type":"text","text":"; 2. ","styles":{}},{"type":"text","text":"analysis of  professional reports of public and private actors involved in certification programmes, social-private partnerships or public-private partnerships; ","styles":{}},{"type":"text","text":"3. ","styles":{}},{"type":"text","text":"interviews with stakeholders that are involved in the larger (PhD) project. ","styles":{}}]}]},{"cells":[{"type":"tableCell","content":[{"type":"text","text":"Requirements","styles":{"bold":true}},{"type":"text","text":" and skills","styles":{"bold":true}}]},{"type":"tableCell","content":[]}]}]},"children":[]},{"type":"bulletListItem","props":{"textAlignment":"justify"},"content":[{"type":"text","text":"Bachelor BIN or BEB; enrolled in master program MID or MME ","styles":{}}],"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"Ambition to develop a master thesis that can serve as a basis for ","styles":{}},{"type":"text","text":"writing and publishing ","styles":{}},{"type":"text","text":"a scientific article","styles":{}}],"children":[]},{"type":"bulletListItem","props":{"textAlignment":"justify"},"content":[{"type":"text","text":"Skills: 1. Good English writing; 2. Experience with organizing Endnote libraries 3. ","styles":{}},{"type":"text","text":"Experience with organizing search queries through Scopus, google advanced search, and other search machines. ","styles":{}}],"children":[]},{"type":"paragraph","content":[],"children":[]},{"type":"paragraph","content":[],"children":[]},{"type":"paragraph","content":[],"children":[]},{"type":"paragraph","content":[],"children":[]}]"#;
        assert_eq!(actual_json, EXPECTED_JSON);
    }

    #[test]
    fn sample_docx_table_cell_with_lift_is_empty() {
        let bytes = std::fs::read("tests/fixtures/sample.docx").expect("fixture file");
        let reader = docspec_docx_reader::DocxReader::from_reader(std::io::Cursor::new(bytes))
            .expect("reader creation");
        let mut buf = Vec::<u8>::new();
        let writer = BlockNoteWriter::new(&mut buf);
        let sink = StackTrackingSink::new(writer);
        docspec_core::pipe(reader, sink).expect("pipe");
        let actual_json = String::from_utf8(buf).expect("valid utf-8");
        let output: serde_json::Value = serde_json::from_str(&actual_json).expect("valid json");
        let table = output
            .as_array()
            .expect("top-level array")
            .iter()
            .find(|b| b["type"] == "table")
            .expect("table block");
        let rows = table["content"]["rows"].as_array().expect("rows array");
        let last_row = rows.last().expect("last row");
        let cells = last_row["cells"].as_array().expect("cells array");
        let lifted_cell = &cells[1];
        assert_eq!(
            lifted_cell["content"],
            serde_json::json!([]),
            "cell that contained the bullet list must be empty after lift"
        );
    }
}
