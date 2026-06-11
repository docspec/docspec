//! Integration tests for `BlockNoteWriter`.

#![allow(clippy::expect_used, clippy::redundant_test_prefix)]

extern crate alloc;

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use std::collections::HashMap;
    use std::io;
    use std::io::Write;

    use docspec_blocknote_writer::BlockNoteWriter;
    use docspec_core::{
        AssetProvider, Color, Event, EventSink as _, ImageSource, StackTrackingSink, TextAlignment,
        TextStyleKind,
    };
    use docspec_test_utils::builders::text;
    use docspec_test_utils::FailingWriter;

    struct MockAssetProvider {
        assets: HashMap<String, (String, Vec<u8>)>,
        content_type_only: HashMap<String, String>,
        fail_stream: bool,
    }

    impl MockAssetProvider {
        fn new() -> Self {
            Self {
                assets: HashMap::new(),
                content_type_only: HashMap::new(),
                fail_stream: false,
            }
        }

        fn with_asset(mut self, id: &str, content_type: &str, data: &[u8]) -> Self {
            self.assets
                .insert(id.to_string(), (content_type.to_string(), data.to_vec()));
            self
        }

        fn with_content_type_only(mut self, id: &str, content_type: &str) -> Self {
            self.content_type_only
                .insert(id.to_string(), content_type.to_string());
            self
        }

        fn with_failing_stream(mut self) -> Self {
            self.fail_stream = true;
            self
        }
    }

    impl AssetProvider for MockAssetProvider {
        fn content_type(&self, asset_id: &str) -> Option<Cow<'_, str>> {
            self.assets
                .get(asset_id)
                .map(|(ct, _)| Cow::Borrowed(ct.as_str()))
                .or_else(|| {
                    self.content_type_only
                        .get(asset_id)
                        .map(|ct| Cow::Borrowed(ct.as_str()))
                })
        }

        fn stream_to(&self, asset_id: &str, writer: &mut dyn Write) -> Option<io::Result<u64>> {
            if self.fail_stream {
                return Some(Err(io::Error::other("simulated stream failure")));
            }
            self.assets.get(asset_id).map(|(_, data)| {
                writer.write_all(data)?;
                Ok(u64::try_from(data.len()).unwrap_or(0))
            })
        }
    }

    fn run_events_with_assets(events: &[Event], provider: &dyn AssetProvider) -> String {
        let mut buf = Vec::<u8>::new();
        let writer = StackTrackingSink::new(BlockNoteWriter::with_assets(&mut buf, provider));
        docspec_test_utils::drive(writer, events.iter().cloned());
        String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8")
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
    fn image_with_asset_source_without_provider_errors() {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start document should succeed");
        let result = writer.handle_event(Event::Image {
            source: ImageSource::Asset {
                asset_id: "img1".to_string(),
            },
            alt: None,
            title: None,
            decorative: false,
            id: None,
        });
        let err = result.expect_err("Image with asset source requires a provider");
        assert_eq!(err.to_string(), "no AssetProvider configured");
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
        let provider =
            MockAssetProvider::new().with_asset("img1", "image/png", &[0x89, 0x50, 0x4E, 0x47]);
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::with_assets(&mut buf, &provider));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");
        let img_result = writer.handle_event(Event::Image {
            source: ImageSource::Asset {
                asset_id: "img1".to_string(),
            },
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
        let provider = MockAssetProvider::new();
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::with_assets(&mut buf, &provider));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");
        let result = writer.handle_event(Event::Image {
            source: ImageSource::Asset {
                asset_id: "missing".to_string(),
            },
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
        let provider = MockAssetProvider::new()
            .with_asset("img1", "image/png", &[0x89])
            .with_failing_stream();
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::with_assets(&mut buf, &provider));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");
        let result = writer.handle_event(Event::Image {
            source: ImageSource::Asset {
                asset_id: "img1".to_string(),
            },
            alt: None,
            title: None,
            decorative: false,
            id: None,
        });
        let err = result.expect_err("image with failing stream must fail");
        assert_eq!(err.to_string(), "I/O error: simulated stream failure");
    }

    #[test]
    fn asset_image_jpeg() {
        let provider =
            MockAssetProvider::new().with_asset("photo", "image/jpeg", &[0xFF, 0xD8, 0xFF]);
        let json = run_events_with_assets(
            &[
                start_document(),
                Event::Image {
                    source: ImageSource::Asset {
                        asset_id: "photo".to_string(),
                    },
                    alt: None,
                    title: None,
                    decorative: false,
                    id: None,
                },
                Event::EndDocument,
            ],
            &provider,
        );
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"data:image/jpeg;base64,/9j/","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn asset_image_empty_bytes() {
        let provider = MockAssetProvider::new().with_asset("empty", "image/png", &[]);
        let json = run_events_with_assets(
            &[
                start_document(),
                Event::Image {
                    source: ImageSource::Asset {
                        asset_id: "empty".to_string(),
                    },
                    alt: None,
                    title: None,
                    decorative: false,
                    id: None,
                },
                Event::EndDocument,
            ],
            &provider,
        );
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"data:image/png;base64,","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn asset_and_uri_images_mixed() {
        let provider =
            MockAssetProvider::new().with_asset("img1", "image/png", &[0x89, 0x50, 0x4E, 0x47]);
        let json = run_events_with_assets(
            &[
                start_document(),
                Event::Image {
                    source: ImageSource::Asset {
                        asset_id: "img1".to_string(),
                    },
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
            ],
            &provider,
        );
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"data:image/png;base64,iVBORw==","caption":""},"content":null,"children":[]},{"type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn asset_image_same_id_twice() {
        let provider =
            MockAssetProvider::new().with_asset("img1", "image/png", &[0x89, 0x50, 0x4E, 0x47]);
        let json = run_events_with_assets(
            &[
                start_document(),
                Event::Image {
                    source: ImageSource::Asset {
                        asset_id: "img1".to_string(),
                    },
                    alt: None,
                    title: None,
                    decorative: false,
                    id: None,
                },
                Event::Image {
                    source: ImageSource::Asset {
                        asset_id: "img1".to_string(),
                    },
                    alt: None,
                    title: None,
                    decorative: false,
                    id: None,
                },
                Event::EndDocument,
            ],
            &provider,
        );
        assert_eq!(
            json,
            r#"[{"type":"image","props":{"url":"data:image/png;base64,iVBORw==","caption":""},"content":null,"children":[]},{"type":"image","props":{"url":"data:image/png;base64,iVBORw==","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn asset_image_in_paragraph() {
        let provider =
            MockAssetProvider::new().with_asset("img1", "image/png", &[0x89, 0x50, 0x4E, 0x47]);
        let json = run_events_with_assets(
            &[
                start_document(),
                start_paragraph(),
                text("Before"),
                Event::EndParagraph,
                Event::Image {
                    source: ImageSource::Asset {
                        asset_id: "img1".to_string(),
                    },
                    alt: None,
                    title: None,
                    decorative: false,
                    id: None,
                },
                Event::EndDocument,
            ],
            &provider,
        );
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
        let provider = MockAssetProvider::new().with_content_type_only("img1", "image/png");
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::with_assets(&mut buf, &provider));
        let start_result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(start_result.is_ok(), "start should succeed");
        let result = writer.handle_event(Event::Image {
            source: ImageSource::Asset {
                asset_id: "img1".to_string(),
            },
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
    fn image_in_blockquote_emits_sibling() {
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
            r#"[{"type":"quote","content":[],"children":[]},{"type":"image","props":{"url":"https://example.com/logo.png","caption":"logo"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn nested_blockquote_emits_sibling() {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

        // Test actual nesting: send StartBlockQuote while another is open
        // Sibling emission should close outer quote and emit inner as sibling
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
        // Outer was force-closed by sibling emission, so only close inner
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());

        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"outer","styles":{}}],"children":[]},{"type":"quote","content":[{"type":"text","text":"inner","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn heading_in_blockquote_emits_sibling() {
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
            r#"[{"type":"quote","content":[],"children":[]},{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn code_block_in_blockquote_emits_sibling() {
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
            r#"[{"type":"quote","content":[],"children":[]},{"type":"codeBlock","props":{"language":"rust"},"content":[{"type":"text","text":"fn main() {}","styles":{}}],"children":[]}]"#
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
    fn thematic_break_in_blockquote_emits_sibling() {
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
            r#"[{"type":"quote","content":[],"children":[]},{"type":"divider"}]"#
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
    fn outer_table_inside_list_item_drops_table_and_nested_table_alike() {
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
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"bullet","styles":{}}],"children":[]}]"#
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
    fn image_inside_table_cell_is_dropped() {
        // Drives an Image event between StartTableCell / EndTableCell. BlockNote
        // cell content is InlineContent[] — block-level events (including images)
        // are silently dropped per the documented cell-content semantics.
        let json = run_events(&[
            start_document(),
            start_table(),
            start_table_row(),
            start_table_cell(),
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: Some("dropped".to_string()),
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
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]}]"#
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
    fn list_item_with_id_emits_id_field() {
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
            r#"[{"id":"item-1","type":"bulletListItem","content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
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
    fn list_inside_table_cell_is_dropped() {
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_table()).is_ok());
        assert!(writer.handle_event(start_table_row()).is_ok());
        assert!(writer.handle_event(start_table_cell()).is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer.handle_event(text("dropped")).is_ok());
        assert!(writer.handle_event(Event::EndUnorderedListItem).is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn list_inside_blockquote_emits_sibling() {
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
            r#"[{"type":"quote","content":[],"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"quoted bullet","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn nested_table_with_list_in_cell_lifts_table_drops_list() {
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
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"outer","styles":{}}]}]}]},"children":[]},{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]}]"#
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
    fn heading_inside_list_item_is_dropped() {
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
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn image_inside_list_item_is_dropped() {
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
            r#"[{"type":"bulletListItem","content":[],"children":[]}]"#
        );
    }

    #[test]
    fn code_block_inside_list_item_is_dropped() {
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
            r#"[{"type":"bulletListItem","content":[],"children":[]}]"#
        );
    }

    #[test]
    fn nested_blockquote_inside_list_item_is_dropped() {
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
            r#"[{"type":"bulletListItem","content":[],"children":[]}]"#
        );
    }

    #[test]
    fn divider_inside_list_item_is_dropped() {
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
            r#"[{"type":"bulletListItem","content":[],"children":[]}]"#
        );
    }

    #[test]
    fn table_inside_list_item_is_dropped() {
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
            r#"[{"type":"bulletListItem","content":[],"children":[]}]"#
        );
    }

    #[test]
    fn text_after_dropped_block_in_list_item_is_preserved() {
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
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"before","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"after","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn drop_counter_returns_to_zero_after_end() {
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
            Event::EndUnorderedListItem,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("second"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[]},{"type":"bulletListItem","content":[{"type":"text","text":"second","styles":{}}],"children":[]}]"#
        );
    }

    // ============================================================================
    // T14: COVERAGE GAP FILLS
    // ============================================================================

    #[test]
    fn heading_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! returning early in the StartHeading match arm
        // (the in_table_cell=true case). EndHeading with in_text_block=false is also
        // driven (the !in_text_block guard fires). Heading text is flattened into the
        // cell's inline content — only the heading *structure* is dropped.
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
        // Text is flattened into cell inline content (BlockNote cell flattening policy).
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"h","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn blockquote_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! returning early in both the StartBlockQuote and
        // EndBlockQuote match arms. blockquote_depth is never incremented; text is
        // flattened into the cell's inline content.
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
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"q","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn preformatted_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! returning early in both the StartPreformatted and
        // EndPreformatted match arms. Code-block structure is never emitted; text is
        // flattened into the cell's inline content.
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
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[{"type":"text","text":"code","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn line_break_inside_dropped_heading_in_list_is_dropped() {
        // Drives handle_line_break when drop_inside_list_depth > 0.
        // A LineBreak inside a heading that is itself inside a list item is silently
        // discarded — neither the line break nor any surrounding text from the dropped
        // block appears in the output.
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
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"item","styles":{}}],"children":[]}]"#
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
    fn start_heading_inside_table_cell_is_silently_dropped() {
        // Drives return_if_table_cell! return branch in the StartHeading arm (line 843).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_table()).is_ok());
        assert!(writer.handle_event(start_table_row()).is_ok());
        assert!(writer.handle_event(start_table_cell()).is_ok());
        assert!(writer.handle_event(start_heading(1)).is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn start_heading_inside_list_hits_drop_block_macro() {
        // Drives drop_block_in_list_start! return branch in StartHeading arm (line 844)
        // and drop_block_in_list_end! return branch in EndHeading arm (line 849).
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
        assert!(writer.handle_event(start_heading(1)).is_ok());
        assert!(writer.handle_event(Event::EndHeading).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[]}]"#
        );
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
    fn end_preformatted_inside_list_hits_drop_macro() {
        // Drives drop_block_in_list_start! in StartPreformatted arm (line 886) and
        // drop_block_in_list_end! in EndPreformatted arm (line 856).
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
        assert!(writer.handle_event(start_preformatted(None)).is_ok());
        assert!(writer.handle_event(Event::EndPreformatted).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[]}]"#
        );
    }

    #[test]
    fn end_preformatted_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! return branch in EndPreformatted arm (line 857).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_table()).is_ok());
        assert!(writer.handle_event(start_table_row()).is_ok());
        assert!(writer.handle_event(start_table_cell()).is_ok());
        assert!(writer.handle_event(Event::EndPreformatted).is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]}]"#
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
    fn start_blockquote_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! return branch in StartBlockQuote arm (line 866).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_table()).is_ok());
        assert!(writer.handle_event(start_table_row()).is_ok());
        assert!(writer.handle_event(start_table_cell()).is_ok());
        assert!(writer.handle_event(start_blockquote()).is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn start_blockquote_inside_list_hits_drop_block_macro() {
        // Drives drop_block_in_list_start! in StartBlockQuote arm (line 867) and
        // drop_block_in_list_end! in EndBlockQuote arm (line 872).
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
        assert!(writer.handle_event(start_blockquote()).is_ok());
        assert!(writer.handle_event(Event::EndBlockQuote).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[]}]"#
        );
    }

    #[test]
    fn end_blockquote_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! return branch in EndBlockQuote arm (line 873).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_table()).is_ok());
        assert!(writer.handle_event(start_table_row()).is_ok());
        assert!(writer.handle_event(start_table_cell()).is_ok());
        assert!(writer.handle_event(Event::EndBlockQuote).is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn start_preformatted_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! return branch in StartPreformatted arm (line 885).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_table()).is_ok());
        assert!(writer.handle_event(start_table_row()).is_ok());
        assert!(writer.handle_event(start_table_cell()).is_ok());
        assert!(writer.handle_event(start_preformatted(None)).is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn thematic_break_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! return branch in ThematicBreak arm (line 891).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_table()).is_ok());
        assert!(writer.handle_event(start_table_row()).is_ok());
        assert!(writer.handle_event(start_table_cell()).is_ok());
        assert!(writer
            .handle_event(Event::ThematicBreak { id: None })
            .is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"table","content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","content":[]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn end_table_inside_list_hits_drop_macro() {
        // Drives drop_block_in_list_start! in StartTable arm (line 604) and
        // drop_block_in_list_end! in EndTable arm (line 348).
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
        assert!(writer.handle_event(start_table()).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[],"children":[]}]"#
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
    fn with_assets_constructor_accepts_asset_provider() {
        // Drives BlockNoteWriter::with_assets constructor body (line 804).
        let provider = MockAssetProvider::new();
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::with_assets(&mut buf, &provider);
        assert!(writer.handle_event(start_document()).is_ok());
        assert!(writer.handle_event(start_paragraph()).is_ok());
        assert!(writer.handle_event(text("hello")).is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        writer.finish().expect("writer should finish fixture");
        let json = String::from_utf8(buf).expect("BlockNoteWriter output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"paragraph","content":[{"type":"text","text":"hello","styles":{}}],"children":[]}]"#
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
            r#"[{"id":"li-1","type":"numberedListItem","props":{"start":5},"content":[{"type":"text","text":"five","styles":{}}],"children":[]}]"#
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
    fn all_block_types_inside_list_item_are_dropped() {
        // Exercises drop_block_in_list_start! and drop_block_in_list_end! for all block types
        // inside a list item: heading, preformatted, blockquote, table, thematic break
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            text("before"),
            // Heading inside list item → dropped
            start_heading(1),
            text("heading"),
            Event::EndHeading,
            // Preformatted inside list item → dropped
            start_preformatted(None),
            text("code"),
            Event::EndPreformatted,
            // BlockQuote inside list item → dropped
            start_blockquote(),
            text("quote"),
            Event::EndBlockQuote,
            // ThematicBreak inside list item → dropped
            Event::ThematicBreak { id: None },
            text("after"),
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        // Only "before" and "after" should appear; all block content dropped
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"before","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"after","styles":{}}],"children":[]}]}]"#
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
    fn image_after_children_transition_inside_list_item_is_dropped() {
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
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn thematic_break_after_children_transition_inside_list_item_is_dropped() {
        let json = list_item_with_children_transition_then(vec![Event::ThematicBreak { id: None }]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn heading_after_children_transition_inside_list_item_is_dropped() {
        let json = list_item_with_children_transition_then(vec![
            start_heading(2),
            text("leaked-heading"),
            Event::EndHeading,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn blockquote_after_children_transition_inside_list_item_is_dropped() {
        let json = list_item_with_children_transition_then(vec![
            start_blockquote(),
            text("leaked-quote"),
            Event::EndBlockQuote,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn preformatted_after_children_transition_inside_list_item_is_dropped() {
        let json = list_item_with_children_transition_then(vec![
            start_preformatted(None),
            text("leaked-code"),
            Event::EndPreformatted,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn table_after_children_transition_inside_list_item_is_dropped() {
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
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"first","styles":{}}],"children":[{"type":"paragraph","content":[{"type":"text","text":"second","styles":{}}],"children":[]}]}]"#
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
    fn start_list_item_inside_dropped_block_in_list_item_is_silently_dropped() {
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
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"outer","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn paragraph_events_inside_dropped_block_in_list_item_are_fully_absorbed() {
        let json = run_events(&[
            start_document(),
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            start_blockquote(),
            start_paragraph(),
            text("dropped"),
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
            r#"[{"type":"bulletListItem","content":[{"type":"text","text":"real","styles":{}}],"children":[]}]"#
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
    fn dropped_heading_before_first_aligned_list_paragraph_does_not_steal_alignment() {
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
            r#"[{"type":"bulletListItem","props":{"textAlignment":"center"},"content":[{"type":"text","text":"real","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn dropped_link_before_first_aligned_list_paragraph_does_not_steal_alignment() {
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
            r#"[{"type":"bulletListItem","props":{"textAlignment":"right"},"content":[{"type":"text","text":"real","styles":{}}],"children":[]}]"#
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
    fn link_in_dropped_heading_inside_list_emits_no_link() {
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
            r#"[{"type":"bulletListItem","content":[],"children":[]}]"#
        );
    }
}
