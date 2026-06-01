//! Integration tests for `BlockNoteWriter`.

#![allow(clippy::expect_used)]

extern crate alloc;

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use std::collections::HashMap;
    use std::io;
    use std::io::Write;

    use docspec_blocknote_writer::BlockNoteWriter;
    use docspec_core::{
        AssetProvider, Event, EventSink as _, EventSource as _, ImageSource, StackTrackingSink,
        TextStyle,
    };
    use docspec_markdown_reader::MarkdownReader;

    struct FailingWriter {
        fail_after: usize,
        writes: usize,
    }

    impl FailingWriter {
        fn new(fail_after: usize) -> Self {
            Self {
                fail_after,
                writes: 0,
            }
        }
    }

    impl Write for FailingWriter {
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.writes = self.writes.saturating_add(1);
            if self.writes > self.fail_after {
                return Err(std::io::Error::other("simulated write failure"));
            }
            Ok(buf.len())
        }
    }

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
        let mut writer = StackTrackingSink::new(BlockNoteWriter::with_assets(&mut buf, provider));
        for event in events {
            let handle_result = writer.handle_event(event.clone());
            assert!(
                handle_result.is_ok(),
                "handle_event failed: {handle_result:?}"
            );
        }
        let finish_result = writer.finish();
        assert!(finish_result.is_ok(), "finish failed");
        let string_result = String::from_utf8(buf);
        assert!(string_result.is_ok(), "invalid UTF-8 output");
        string_result.unwrap_or_default()
    }

    fn run_events(events: &[Event]) -> String {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
        for event in events {
            let handle_result = writer.handle_event(event.clone());
            assert!(handle_result.is_ok(), "handle_event failed");
        }
        let finish_result = writer.finish();
        assert!(finish_result.is_ok(), "finish failed");
        let string_result = String::from_utf8(buf);
        assert!(string_result.is_ok(), "invalid UTF-8 output");
        string_result.unwrap_or_default()
    }

    fn run_direct_writer_events(events: &[Event]) -> String {
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        for event in events {
            let handle_result = writer.handle_event(event.clone());
            assert!(handle_result.is_ok(), "handle_event failed");
        }
        let finish_result = writer.finish();
        assert!(finish_result.is_ok(), "finish failed");
        let string_result = String::from_utf8(buf);
        assert!(string_result.is_ok(), "invalid UTF-8 output");
        string_result.unwrap_or_default()
    }

    #[test]
    fn empty_document() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::EndDocument,
        ]);
        assert_eq!(json, "[]");
    }

    #[test]
    fn single_paragraph() {
        let json = run_events(&[
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
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Hello","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn bold_text() {
        let json = run_events(&[
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
                content: "Bold".to_string(),
                style: TextStyle::default().bold(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Bold","styles":{"bold":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_renders_as_newline() {
        let json = run_events(&[
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
                content: "Line one".to_string(),
                style: TextStyle::default(),
            },
            Event::SoftBreak,
            Event::Text {
                content: "Line two".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Line one","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"Line two","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_inside_heading() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartHeading { level: 2, id: None },
            Event::Text {
                content: "Title one".to_string(),
                style: TextStyle::default(),
            },
            Event::SoftBreak,
            Event::Text {
                content: "Title two".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":2,"textAlignment":"left"},"content":[{"type":"text","text":"Title one","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"Title two","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_inside_table_cell() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                colspan: None,
                rowspan: None,
                id: None,
            },
            Event::Text {
                content: "Cell line one".to_string(),
                style: TextStyle::default(),
            },
            Event::SoftBreak,
            Event::Text {
                content: "Cell line two".to_string(),
                style: TextStyle::default(),
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","props":{"textColor":"default"},"content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Cell line one","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"Cell line two","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_inside_list_item() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Bullet line one".to_string(),
                style: TextStyle::default(),
            },
            Event::SoftBreak,
            Event::Text {
                content: "Bullet line two".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Bullet line one","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"Bullet line two","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_inside_link_display_text() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::StartLink {
                href: "https://example.com".to_string(),
                id: None,
                title: None,
            },
            Event::Text {
                content: "Click line one".to_string(),
                style: TextStyle::default(),
            },
            Event::SoftBreak,
            Event::Text {
                content: "click line two".to_string(),
                style: TextStyle::default(),
            },
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"Click line one","styles":{}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"click line two","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn soft_break_inside_blockquote() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Quote line one".to_string(),
                style: TextStyle::default(),
            },
            Event::SoftBreak,
            Event::Text {
                content: "Quote line two".to_string(),
                style: TextStyle::default(),
            },
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
                content: "Bold line one".to_string(),
                style: TextStyle::default().bold(),
            },
            Event::SoftBreak,
            Event::Text {
                content: "Bold line two".to_string(),
                style: TextStyle::default().bold(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        // Three text nodes: bold "Bold line one", default-style "\n", bold "Bold line two"
        // The "\n" node has empty styles because handle_line_break calls handle_text with TextStyle::default()
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Bold line one","styles":{"bold":true}},{"type":"text","text":"\n","styles":{}},{"type":"text","text":"Bold line two","styles":{"bold":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn italic_text() {
        let json = run_events(&[
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
                content: "Italic".to_string(),
                style: TextStyle::default().italic(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Italic","styles":{"italic":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn bold_and_italic_text() {
        let json = run_events(&[
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
                content: "Both".to_string(),
                style: TextStyle::default().bold().italic(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Both","styles":{"bold":true,"italic":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn heading_level_1() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartHeading { level: 1, id: None },
            Event::Text {
                content: "Title".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn heading_level_2() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartHeading { level: 2, id: None },
            Event::Text {
                content: "Subtitle".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":2,"textAlignment":"left"},"content":[{"type":"text","text":"Subtitle","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn multiple_paragraphs() {
        let json = run_events(&[
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
                content: "First".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Second".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"First","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Second","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn image_block() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
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
        assert!(result.is_err());
    }

    #[test]
    fn mixed_content() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartHeading { level: 1, id: None },
            Event::Text {
                content: "Title".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Body".to_string(),
                style: TextStyle::default(),
            },
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
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Body","styles":{}}],"children":[]},{"type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn ignored_events() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "test".to_string(),
                style: TextStyle::default(),
            },
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "bold quote".to_string(),
                style: TextStyle::default().bold(),
            },
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "quoted".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "normal".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"quoted","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"normal","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn blockquote_multiline() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "line1".to_string(),
                style: TextStyle::default(),
            },
            Event::LineBreak,
            Event::Text {
                content: "line2".to_string(),
                style: TextStyle::default(),
            },
            Event::LineBreak,
            Event::Text {
                content: "line3".to_string(),
                style: TextStyle::default(),
            },
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartHeading { level: 1, id: None },
            Event::Text {
                content: "Title".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Body".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::StartBlockQuote { id: None },
            Event::Text {
                content: "Quote".to_string(),
                style: TextStyle::default(),
            },
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Body","styles":{}}],"children":[]},{"type":"quote","content":[{"type":"text","text":"Quote","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_blockquote_auto_closes_open_content() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Quoted text".to_string(),
                style: TextStyle::default(),
            },
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
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::StartBlockQuote { id: None },
            Event::Text {
                content: "Quote".to_string(),
                style: TextStyle::default(),
            },
            Event::EndBlockQuote,
            Event::StartHeading { level: 2, id: None },
            Event::Text {
                content: "Head".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Para","styles":{}}],"children":[]},{"type":"quote","content":[{"type":"text","text":"Quote","styles":{}}],"children":[]},{"type":"heading","props":{"level":2,"textAlignment":"left"},"content":[{"type":"text","text":"Head","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn list_item_tracked_on_stack() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Item".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn single_bullet_item_emits_bullet_list_item_block() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "First bullet".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"First bullet","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn single_numbered_item_emits_numbered_list_item_block_with_start_1() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(1),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            Event::Text {
                content: "First item".to_string(),
                style: TextStyle::default(),
            },
            Event::EndOrderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"numberedListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left","start":1},"content":[{"type":"text","text":"First item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn two_top_level_bullets_emit_two_sibling_blocks() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "First".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Second".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"First","styles":{}}],"children":[]},{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Second","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_document_closes_single_open_list_item() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "x".to_string(),
                style: TextStyle::default(),
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"x","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_document_with_clean_state_unchanged() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "x".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"x","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_document_with_two_consecutive_open_level_0_items_drains_both() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "a".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "b".to_string(),
                style: TextStyle::default(),
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"a","styles":{}}],"children":[]},{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"b","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn bullet_then_numbered_then_bullet_at_level_0() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Bullet one".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(1),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            Event::Text {
                content: "Number one".to_string(),
                style: TextStyle::default(),
            },
            Event::EndOrderedListItem,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Bullet two".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Bullet one","styles":{}}],"children":[]},{"type":"numberedListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left","start":1},"content":[{"type":"text","text":"Number one","styles":{}}],"children":[]},{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Bullet two","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn bold_text_inside_bullet_list_item() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Bold bullet".to_string(),
                style: TextStyle::default().bold(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Bold bullet","styles":{"bold":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn nested_bullet_lists_emit_children_array() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "a".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "b".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"b","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn three_level_nesting_emits_correct_structure() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "a".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "b".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "c".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"b","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"c","styles":{}}],"children":[]}]}]}]"#
        );
    }

    #[test]
    fn nested_numbered_inside_bullet() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "bullet".to_string(),
                style: TextStyle::default(),
            },
            Event::StartOrderedListItem {
                id: None,
                level: 1,
                start: Some(1),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            Event::Text {
                content: "one".to_string(),
                style: TextStyle::default(),
            },
            Event::EndOrderedListItem,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"bullet","styles":{}}],"children":[{"type":"numberedListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left","start":1},"content":[{"type":"text","text":"one","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn multiple_children_at_same_nested_level() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "a".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "b".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "c".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"b","styles":{}}],"children":[]},{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"c","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn orphan_end_unordered_list_item_is_silent_ok() {
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
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
    fn text_outside_block_auto_opens_paragraph() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::Text {
                content: "Orphan".to_string(),
                style: TextStyle::default(),
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            "[{\"type\":\"paragraph\",\"props\":{\"textAlignment\":\"left\"},\"content\":[{\"type\":\"text\",\"text\":\"Orphan\",\"styles\":{}}],\"children\":[]}]"
        );
    }

    #[test]
    fn multiple_text_in_paragraph() {
        let json = run_events(&[
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
                content: "Hello ".to_string(),
                style: TextStyle::default(),
            },
            Event::Text {
                content: "World".to_string(),
                style: TextStyle::default().bold(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Hello ","styles":{}},{"type":"text","text":"World","styles":{"bold":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn two_paragraphs_without_ids() {
        let json = run_events(&[
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
                content: "First".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Second".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"First","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Second","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn json_escaping_quotes() {
        let json = run_events(&[
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
                content: "He said \"hello\"".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"He said \"hello\"","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn json_escaping_backslash() {
        let json = run_events(&[
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
                content: "path\\to\\file".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"path\\to\\file","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn json_escaping_newline() {
        let json = run_events(&[
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
                content: "line1\nline2".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"line1\nline2","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn json_escaping_tab() {
        let json = run_events(&[
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
                content: "col1\tcol2".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"col1\tcol2","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn empty_paragraph() {
        let json = run_events(&[
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
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[],"children":[]}]"#
        );
    }

    #[test]
    fn empty_heading() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartHeading { level: 1, id: None },
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[],"children":[]}]"#
        );
    }

    #[test]
    fn image_in_paragraph() {
        let json = run_events(&[
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
                content: "Before".to_string(),
                style: TextStyle::default(),
            },
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
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Before","styles":{}}],"children":[]},{"type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn heading_then_paragraph() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartHeading { level: 1, id: None },
            Event::Text {
                content: "Title".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Body".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Body","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn json_escaping_carriage_return() {
        let json = run_events(&[
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
                content: "line1\rline2".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"line1\rline2","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn image_url_escaping() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
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
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[],"children":[]},{"type":"image","props":{"url":"https://example.com/img.png","caption":""},"content":null,"children":[]}]"#
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
        assert!(result.is_err());
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
        assert!(end_result.is_err());
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
        let heading_result = writer.handle_event(Event::StartHeading { level: 1, id: None });
        assert!(heading_result.is_err());
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
        assert!(para_result.is_err());
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
        let json = String::from_utf8(buf).unwrap_or_default();
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
        assert!(result.is_err());
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
        assert!(result.is_err());
    }

    #[test]
    fn asset_image_jpeg() {
        let provider =
            MockAssetProvider::new().with_asset("photo", "image/jpeg", &[0xFF, 0xD8, 0xFF]);
        let json = run_events_with_assets(
            &[
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
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
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
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
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
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
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
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
                    content: "Before".to_string(),
                    style: TextStyle::default(),
                },
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
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Before","styles":{}}],"children":[]},{"type":"image","props":{"url":"data:image/png;base64,iVBORw==","caption":""},"content":null,"children":[]}]"#
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
        assert!(result.is_err());
    }

    #[test]
    fn heading_with_explicit_id() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartHeading {
                level: 1,
                id: Some("custom-id".to_string()),
            },
            Event::Text {
                content: "Title".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert!(json.contains("\"id\":\"custom-id\""));
    }

    #[test]
    fn paragraph_without_id_omits_id_key() {
        let json = run_events(&[
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
                content: "Body".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn code_block_with_language() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartPreformatted {
                id: None,
                syntax: Some("rust".to_string()),
            },
            Event::Text {
                content: "fn main() {}".to_string(),
                style: TextStyle::default(),
            },
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartPreformatted {
                id: None,
                syntax: None,
            },
            Event::Text {
                content: "plain code".to_string(),
                style: TextStyle::default(),
            },
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartPreformatted {
                id: None,
                syntax: Some("python".to_string()),
            },
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
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
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartBlockQuote { id: None })
            .is_ok());
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
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartBlockQuote { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartParagraph {
                alignment: None,
                id: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "outer".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        // DO NOT close outer quote - send nested StartBlockQuote directly
        assert!(writer
            .handle_event(Event::StartBlockQuote { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartParagraph {
                alignment: None,
                id: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "inner".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
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
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartBlockQuote { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartHeading { level: 1, id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "Title".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndHeading).is_ok());
        assert!(writer.handle_event(Event::EndBlockQuote).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());

        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[],"children":[]},{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Title","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn code_block_in_blockquote_emits_sibling() {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

        // > ```code```
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartBlockQuote { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartPreformatted {
                syntax: Some("rust".to_string()),
                id: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "fn main() {}".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
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
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartHeading { level: 1, id: None })
            .is_ok());
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

        let string_result = String::from_utf8(buf);
        assert!(string_result.is_ok(), "invalid UTF-8 output");
        assert_eq!(
            string_result.unwrap_or_default(),
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[],"children":[]},{"type":"image","props":{"url":"https://example.com/logo.png","caption":"logo"},"content":null,"children":[]}]"#
        );
    }

    #[test]
    fn thematic_break_in_blockquote_emits_sibling() {
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

        // > ---
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartBlockQuote { id: None })
            .is_ok());
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
                content: "code".to_string(),
                style: TextStyle::default().code(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"code","styles":{"code":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn strikethrough_text() {
        let json = run_events(&[
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
                content: "struck".to_string(),
                style: TextStyle::default().strikethrough(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"struck","styles":{"strike":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn underline_text() {
        let json = run_events(&[
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
                content: "underlined".to_string(),
                style: TextStyle::default().underline(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"underlined","styles":{"underline":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn combined_styles_bold_code_strikethrough() {
        let json = run_events(&[
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
                content: "combined".to_string(),
                style: TextStyle::default().bold().code().strikethrough(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"combined","styles":{"bold":true,"code":true,"strike":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn empty_table_emits_table_block_with_no_rows() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","props":{"textColor":"default"},"content":{"type":"tableContent","columnWidths":[],"rows":[]},"children":[]}]"#
        );
    }

    #[test]
    fn simple_table_with_one_data_row_and_two_cells() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::Text {
                content: "Cell1".to_string(),
                style: TextStyle::default(),
            },
            Event::EndTableCell,
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::Text {
                content: "Cell2".to_string(),
                style: TextStyle::default(),
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","props":{"textColor":"default"},"content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Cell1","styles":{}}]},{"type":"tableCell","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Cell2","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn header_only_table() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableHeader {
                id: None,
                scope: None,
                abbr: None,
                colspan: None,
                rowspan: None,
            },
            Event::Text {
                content: "H1".to_string(),
                style: TextStyle::default(),
            },
            Event::EndTableHeader,
            Event::StartTableHeader {
                id: None,
                scope: None,
                abbr: None,
                colspan: None,
                rowspan: None,
            },
            Event::Text {
                content: "H2".to_string(),
                style: TextStyle::default(),
            },
            Event::EndTableHeader,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","props":{"textColor":"default"},"content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"H1","styles":{}}]},{"type":"tableCell","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"H2","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn table_cell_with_bold_text() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::Text {
                content: "bold".to_string(),
                style: TextStyle::default().bold(),
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","props":{"textColor":"default"},"content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"bold","styles":{"bold":true}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn table_preceded_by_paragraph_closes_paragraph() {
        let json = run_events(&[
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
            Event::EndParagraph,
            Event::StartTable { id: None },
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"before","styles":{}}],"children":[]},{"type":"table","props":{"textColor":"default"},"content":{"type":"tableContent","columnWidths":[],"rows":[]},"children":[]}]"#
        );
    }

    #[test]
    fn table_followed_by_paragraph_opens_new_block() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::EndTable,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "after".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"table","props":{"textColor":"default"},"content":{"type":"tableContent","columnWidths":[],"rows":[]},"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"after","styles":{}}],"children":[]}]"#
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
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        let result = writer.handle_event(Event::EndTable);
        assert!(result.is_ok(), "orphan EndTable must be silently absorbed");
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(json, "[]");
    }

    #[test]
    fn nested_table_inner_structure_is_dropped() {
        // Drives a nested StartTable inside an outer table cell. The writer's
        // depth guards drop every inner table event (start, row, cell, text,
        // and their closes); only the outer table is emitted with the outer
        // cell's text intact. Bypasses StackTrackingSink to drive a hand-
        // crafted sequence that no current reader produces but DOCX/ODT
        // readers may produce in the future.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::StartTable { id: None }).is_ok());
        assert!(writer
            .handle_event(Event::StartTableRow { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "outer".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        // Nested inner table — every event below is silently absorbed by guards
        assert!(writer.handle_event(Event::StartTable { id: None }).is_ok());
        assert!(writer
            .handle_event(Event::StartTableRow { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "inner".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
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
            r#"[{"type":"table","props":{"textColor":"default"},"content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"outer","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn thematic_break_with_id_emits_id_field() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
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
        assert!(json.contains(r#""id":"img-1""#));
    }

    #[test]
    fn end_preformatted_without_open_block_is_noop() {
        // Drives EndPreformatted when in_text_block == false (no open preformatted
        // block). Covers the guard that returns early when the close has no
        // matching open. Bypasses StackTrackingSink since the stack tracker
        // rejects orphan close events.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
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
            r#"[{"type":"table","props":{"textColor":"default"},"content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn numbered_list_item_with_start_5_emits_start_prop() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(5),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            Event::Text {
                content: "Item".to_string(),
                style: TextStyle::default(),
            },
            Event::EndOrderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"numberedListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left","start":5},"content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn numbered_list_item_with_no_start_omits_start_prop() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: None,
                style_type: docspec_core::ListStyleType::Decimal,
            },
            Event::Text {
                content: "Item".to_string(),
                style: TextStyle::default(),
            },
            Event::EndOrderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"numberedListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn unordered_list_item_never_emits_start_prop() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Item".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
        assert!(
            !json.contains("\"start\""),
            "bulletListItem must not emit start"
        );
    }

    #[test]
    fn list_item_with_id_emits_id_field() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: Some("item-1".to_string()),
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Item".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"id":"item-1","type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn list_item_without_id_omits_id_key() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Item".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
        assert!(
            !json.contains("\"id\""),
            "list item without id must not emit id key"
        );
    }

    #[test]
    fn list_inside_table_cell_is_dropped() {
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::StartTable { id: None }).is_ok());
        assert!(writer
            .handle_event(Event::StartTableRow { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "dropped".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndUnorderedListItem).is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        assert!(writer.finish().is_ok());
        let json = String::from_utf8(buf).expect("output should be valid UTF-8");
        assert_eq!(
            json,
            r#"[{"type":"table","props":{"textColor":"default"},"content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[]}]}]},"children":[]}]"#
        );
    }

    #[test]
    fn list_inside_blockquote_emits_sibling() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "quoted bullet".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[],"children":[]},{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"quoted bullet","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn nested_table_with_list_in_cell_drops_list() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::Text {
                content: "outer".to_string(),
                style: TextStyle::default(),
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "inner dropped".to_string(),
                style: TextStyle::default(),
            },
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
            r#"[{"type":"table","props":{"textColor":"default"},"content":{"type":"tableContent","columnWidths":[],"rows":[{"cells":[{"type":"tableCell","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"outer","styles":{}}]}]}]},"children":[]}]"#
        );
    }

    // ============================================================================
    // T10: LEVEL-DOWN TRANSITIONS AND LEVEL-JUMP CLAMPING
    // ============================================================================

    #[test]
    fn level_two_to_zero_drops_three_levels_correctly() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "a".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "b".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "c".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "d".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"b","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"c","styles":{}}],"children":[]}]}]},{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"d","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn level_two_to_one_drops_one_level() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "a".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "b".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "c".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "d".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"b","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"c","styles":{}}],"children":[]}]},{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"d","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn programmatic_level_jump_0_to_2_clamps_to_1() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "a".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "b".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"b","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn explicit_end_then_level_down_works() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "a".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "b".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "c".to_string(),
                style: TextStyle::default(),
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"b","styles":{}}],"children":[]}]},{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"c","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn end_document_with_nested_open_items_drains_in_reverse_order() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "a".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "b".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "c".to_string(),
                style: TextStyle::default(),
            },
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"b","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"c","styles":{}}],"children":[]}]}]}]"#
        );
    }

    // ============================================================================
    // T11: PARAGRAPH-INSIDE-LIST-ITEM DISPATCH
    // ============================================================================

    #[test]
    fn single_paragraph_item_inline_content_only() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Hello".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Hello","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn multi_paragraph_item_first_inline_rest_as_children() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Para one".to_string(),
                style: TextStyle::default(),
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Para two".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Para one","styles":{}}],"children":[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Para two","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn nested_item_inherits_paragraph_dispatch() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "a".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "b1".to_string(),
                style: TextStyle::default(),
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "b2".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"a","styles":{}}],"children":[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"b1","styles":{}}],"children":[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"b2","styles":{}}],"children":[]}]}]}]"#
        );
    }

    #[test]
    fn three_paragraphs_in_item() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "Para one".to_string(),
                style: TextStyle::default(),
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Para two".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Para three".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"Para one","styles":{}}],"children":[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Para two","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Para three","styles":{}}],"children":[]}]}]"#
        );
    }

    #[test]
    fn list_immediately_after_blockquote_with_no_intervening_text_emits_at_top_level() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::Text {
                content: "Quote".to_string(),
                style: TextStyle::default(),
            },
            Event::EndBlockQuote,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "after quote".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"quote","content":[{"type":"text","text":"Quote","styles":{}}],"children":[]},{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"after quote","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn heading_inside_list_item_is_dropped() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartHeading { level: 1, id: None },
            Event::Text {
                content: "h".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::Text {
                content: "item".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(
            !json.contains("\"text\":\"h\""),
            "heading text must be dropped"
        );
        assert!(
            json.contains("\"text\":\"item\""),
            "item text must be preserved"
        );
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"item","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn image_inside_list_item_is_dropped() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
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
        assert!(!json.contains("image"), "image must be dropped");
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[],"children":[]}]"#
        );
    }

    #[test]
    fn code_block_inside_list_item_is_dropped() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartPreformatted {
                id: None,
                syntax: None,
            },
            Event::Text {
                content: "code".to_string(),
                style: TextStyle::default(),
            },
            Event::EndPreformatted,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(
            !json.contains("\"text\":\"code\""),
            "code text must be dropped"
        );
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[],"children":[]}]"#
        );
    }

    #[test]
    fn nested_blockquote_inside_list_item_is_dropped() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartBlockQuote { id: None },
            Event::Text {
                content: "q".to_string(),
                style: TextStyle::default(),
            },
            Event::EndBlockQuote,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(
            !json.contains("\"text\":\"q\""),
            "quote text must be dropped"
        );
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[],"children":[]}]"#
        );
    }

    #[test]
    fn divider_inside_list_item_is_dropped() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::ThematicBreak { id: None },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(!json.contains("divider"), "divider must be dropped");
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[],"children":[]}]"#
        );
    }

    #[test]
    fn table_inside_list_item_is_dropped() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::Text {
                content: "cell".to_string(),
                style: TextStyle::default(),
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(!json.contains("table"), "table must be dropped");
        assert!(
            !json.contains("\"text\":\"cell\""),
            "cell text must be dropped"
        );
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[],"children":[]}]"#
        );
    }

    #[test]
    fn text_after_dropped_block_in_list_item_is_preserved() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "before".to_string(),
                style: TextStyle::default(),
            },
            Event::StartHeading { level: 1, id: None },
            Event::Text {
                content: "dropped".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::Text {
                content: "after".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(
            !json.contains("\"text\":\"dropped\""),
            "dropped text must not appear"
        );
        assert!(
            json.contains("\"text\":\"before\""),
            "before text must be preserved"
        );
        assert!(
            json.contains("\"text\":\"after\""),
            "after text must be preserved"
        );
    }

    #[test]
    fn drop_counter_returns_to_zero_after_end() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartHeading { level: 1, id: None },
            Event::Text {
                content: "dropped".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::EndUnorderedListItem,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "second".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(
            !json.contains("\"text\":\"dropped\""),
            "dropped text must not appear"
        );
        assert!(
            json.contains("\"text\":\"second\""),
            "second item text must be preserved"
        );
        assert_eq!(
            json,
            r#"[{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[],"children":[]},{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"second","styles":{}}],"children":[]}]"#
        );
    }

    fn load_fixture(name: &str) -> serde_json::Value {
        let path = format!(
            "{}/../../tests/fixtures/blocknote/{}",
            env!("CARGO_MANIFEST_DIR"),
            name
        );
        let read_result = std::fs::read_to_string(&path);
        assert!(read_result.is_ok(), "fixture {name} not readable");
        let parse_result: Result<serde_json::Value, _> =
            serde_json::from_str(&read_result.unwrap_or_default());
        assert!(parse_result.is_ok(), "fixture {name} not valid JSON");
        parse_result.unwrap_or_default()
    }

    fn run_markdown(input: &str) -> String {
        let mut reader = MarkdownReader::new(input);
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
        loop {
            let next = reader.next_event();
            assert!(next.is_ok(), "markdown reader failed");
            match next.unwrap_or_default() {
                Some(event) => {
                    let handle_result = writer.handle_event(event);
                    assert!(handle_result.is_ok(), "handle_event failed");
                }
                None => break,
            }
        }
        let finish_result = writer.finish();
        assert!(finish_result.is_ok(), "pipeline finish failed");
        let string_result = String::from_utf8(buf);
        assert!(string_result.is_ok(), "invalid UTF-8 output");
        string_result.unwrap_or_default()
    }

    #[test]
    fn integration_simple_bullet_list_matches_fixture() {
        let json = run_markdown("- a\n- b\n- c");
        let actual_result: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(actual_result.is_ok(), "actual output not valid JSON");
        assert_eq!(
            actual_result.unwrap_or_default(),
            load_fixture("lists_simple_bullet.json")
        );
    }

    #[test]
    fn integration_simple_numbered_list_matches_fixture() {
        let json = run_markdown("1. one\n2. two\n3. three");
        let actual_result: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(actual_result.is_ok(), "actual output not valid JSON");
        assert_eq!(
            actual_result.unwrap_or_default(),
            load_fixture("lists_simple_numbered.json"),
        );
    }

    #[test]
    fn integration_nested_bullets_matches_fixture() {
        let json = run_markdown("- a\n  - b\n  - c\n- d");
        let actual_result: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(actual_result.is_ok(), "actual output not valid JSON");
        assert_eq!(
            actual_result.unwrap_or_default(),
            load_fixture("lists_nested_bullets.json"),
        );
    }

    #[test]
    fn integration_mixed_types_matches_fixture() {
        let json = run_markdown("- bullet\n1. numbered\n- another bullet");
        let actual_result: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(actual_result.is_ok(), "actual output not valid JSON");
        assert_eq!(
            actual_result.unwrap_or_default(),
            load_fixture("lists_mixed_types.json"),
        );
    }

    #[test]
    fn integration_multi_paragraph_item_matches_fixture() {
        let json = run_markdown("- first para\n\n  second para\n- next item");
        let actual_result: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(actual_result.is_ok(), "actual output not valid JSON");
        assert_eq!(
            actual_result.unwrap_or_default(),
            load_fixture("lists_multi_paragraph_item.json"),
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::StartHeading { level: 1, id: None },
            Event::Text {
                content: "h".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert!(
            !json.contains("\"type\":\"heading\""),
            "heading structure must not appear in table cell"
        );
        // Text is flattened into cell inline content (BlockNote cell flattening policy).
        assert!(
            json.contains("\"text\":\"h\""),
            "heading text is flattened into cell inline content"
        );
    }

    #[test]
    fn blockquote_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! returning early in both the StartBlockQuote and
        // EndBlockQuote match arms. blockquote_depth is never incremented; text is
        // flattened into the cell's inline content.
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::StartBlockQuote { id: None },
            Event::Text {
                content: "q".to_string(),
                style: TextStyle::default(),
            },
            Event::EndBlockQuote,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert!(
            !json.contains("\"type\":\"quote\""),
            "blockquote structure must not appear in table cell"
        );
        assert!(
            json.contains("\"text\":\"q\""),
            "blockquote text is flattened into cell inline content"
        );
    }

    #[test]
    fn preformatted_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! returning early in both the StartPreformatted and
        // EndPreformatted match arms. Code-block structure is never emitted; text is
        // flattened into the cell's inline content.
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::StartPreformatted {
                id: None,
                syntax: None,
            },
            Event::Text {
                content: "code".to_string(),
                style: TextStyle::default(),
            },
            Event::EndPreformatted,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert!(
            !json.contains("\"type\":\"codeBlock\""),
            "preformatted structure must not appear in table cell"
        );
        assert!(
            json.contains("\"text\":\"code\""),
            "preformatted text is flattened into cell inline content"
        );
    }

    #[test]
    fn line_break_inside_dropped_heading_in_list_is_dropped() {
        // Drives handle_line_break when drop_inside_list_depth > 0.
        // A LineBreak inside a heading that is itself inside a list item is silently
        // discarded — neither the line break nor any surrounding text from the dropped
        // block appears in the output.
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartHeading { level: 1, id: None },
            Event::Text {
                content: "head".to_string(),
                style: TextStyle::default(),
            },
            Event::LineBreak,
            Event::Text {
                content: "more".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::Text {
                content: "item".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(
            !json.contains("\"text\":\"head\""),
            "heading text must be dropped"
        );
        assert!(
            !json.contains("\"text\":\"\\n\""),
            "line break inside dropped block must be dropped"
        );
        assert!(
            json.contains("\"text\":\"item\""),
            "item text must be preserved"
        );
    }

    #[test]
    fn ordered_list_start_value_overflow_returns_error() {
        // Drives the u32::try_from error path in open_list_item_object when the
        // start value exceeds u32::MAX (4,294,967,295). BlockNote's start prop is a u32.
        // Uses BlockNoteWriter directly to bypass StackTrackingSink validation.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        let result = writer.handle_event(Event::StartOrderedListItem {
            id: None,
            level: 0,
            start: Some(u64::from(u32::MAX) + 1),
            style_type: docspec_core::ListStyleType::Decimal,
        });
        assert!(
            result.is_err(),
            "start value exceeding u32::MAX must return an error"
        );
        let err_msg = format!("{result:?}");
        assert!(
            err_msg.contains("out of range"),
            "error message must describe the range overflow"
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::EndUnorderedListItem,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "after".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert!(
            json.contains("\"type\":\"bulletListItem\""),
            "empty list item must be emitted"
        );
        assert!(
            json.contains("\"text\":\"after\""),
            "paragraph after list must be emitted"
        );
        assert!(
            json.contains("\"type\":\"paragraph\""),
            "paragraph block must appear at top level"
        );
    }

    #[test]
    fn second_paragraph_in_blockquote_emits_separator() {
        // Drives handle_paragraph when blockquote_depth > 0 and blockquote_has_content
        // is true. The separator path emits a "\n\n" text node between the two
        // paragraphs' content so block-quote paragraphs are visually separated.
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::Text {
                content: "first".to_string(),
                style: TextStyle::default(),
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "second".to_string(),
                style: TextStyle::default(),
            },
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert!(
            json.contains("\"text\":\"first\""),
            "first paragraph content must appear"
        );
        assert!(
            json.contains("\"text\":\"second\""),
            "second paragraph content must appear"
        );
        assert!(
            json.contains("\"text\":\"\\n\\n\""),
            "paragraph separator \\n\\n must be emitted between paragraphs"
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
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
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
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/img.png".to_string(),
                },
                alt: None,
                title: None,
                decorative: false,
                id: None,
            },
            Event::Text {
                content: "after".to_string(),
                style: TextStyle::default(),
            },
            Event::EndDocument,
        ]);
        assert!(
            json.contains("\"text\":\"before\""),
            "text before image must appear"
        );
        assert!(
            json.contains("\"text\":\"after\""),
            "text after image must appear in auto-opened paragraph"
        );
        assert!(
            json.contains("\"type\":\"image\""),
            "image block must appear"
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
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
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
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::StartTable { id: None }).is_ok());
        assert!(writer
            .handle_event(Event::StartTableRow { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartHeading { level: 1, id: None })
            .is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            !json.contains("\"type\":\"heading\""),
            "no heading must appear inside table cell output"
        );
    }

    #[test]
    fn start_heading_inside_list_hits_drop_block_macro() {
        // Drives drop_block_in_list_start! return branch in StartHeading arm (line 844)
        // and drop_block_in_list_end! return branch in EndHeading arm (line 849).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartHeading { level: 1, id: None })
            .is_ok());
        assert!(writer.handle_event(Event::EndHeading).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            !json.contains("\"type\":\"heading\""),
            "heading inside list must be dropped"
        );
        assert!(
            json.contains("\"type\":\"bulletListItem\""),
            "list item must still be emitted"
        );
    }

    #[test]
    fn end_heading_closes_open_heading_block() {
        // Drives close_text_block! in the EndHeading arm (line 853) via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartHeading { level: 2, id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "Heading text".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndHeading).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"type\":\"heading\""),
            "heading block must be in output"
        );
        assert!(
            json.contains("\"text\":\"Heading text\""),
            "heading text must be in output"
        );
        assert!(
            json.contains("\"children\":[]"),
            "heading must be properly closed with children:[]"
        );
    }

    #[test]
    fn end_preformatted_inside_list_hits_drop_macro() {
        // Drives drop_block_in_list_start! in StartPreformatted arm (line 886) and
        // drop_block_in_list_end! in EndPreformatted arm (line 856).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartPreformatted {
                id: None,
                syntax: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndPreformatted).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            !json.contains("\"type\":\"codeBlock\""),
            "preformatted block must be dropped inside list"
        );
    }

    #[test]
    fn end_preformatted_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! return branch in EndPreformatted arm (line 857).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::StartTable { id: None }).is_ok());
        assert!(writer
            .handle_event(Event::StartTableRow { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndPreformatted).is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(json.contains("\"type\":\"table\""), "table must appear");
    }

    #[test]
    fn end_preformatted_closes_open_code_block() {
        // Drives close_text_block! in EndPreformatted arm (line 861) via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartPreformatted {
                id: None,
                syntax: Some("rust".to_string()),
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "let x = 1;".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndPreformatted).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"type\":\"codeBlock\""),
            "code block must appear"
        );
        assert!(
            json.contains("\"children\":[]"),
            "code block must be properly closed"
        );
    }

    #[test]
    fn start_blockquote_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! return branch in StartBlockQuote arm (line 866).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::StartTable { id: None }).is_ok());
        assert!(writer
            .handle_event(Event::StartTableRow { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartBlockQuote { id: None })
            .is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            !json.contains("\"type\":\"quote\""),
            "blockquote inside table cell must be dropped"
        );
    }

    #[test]
    fn start_blockquote_inside_list_hits_drop_block_macro() {
        // Drives drop_block_in_list_start! in StartBlockQuote arm (line 867) and
        // drop_block_in_list_end! in EndBlockQuote arm (line 872).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartBlockQuote { id: None })
            .is_ok());
        assert!(writer.handle_event(Event::EndBlockQuote).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            !json.contains("\"type\":\"quote\""),
            "blockquote inside list must be dropped"
        );
        assert!(
            json.contains("\"type\":\"bulletListItem\""),
            "list item must still be emitted"
        );
    }

    #[test]
    fn end_blockquote_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! return branch in EndBlockQuote arm (line 873).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::StartTable { id: None }).is_ok());
        assert!(writer
            .handle_event(Event::StartTableRow { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndBlockQuote).is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(json.contains("\"type\":\"table\""), "table must appear");
    }

    #[test]
    fn start_preformatted_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! return branch in StartPreformatted arm (line 885).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::StartTable { id: None }).is_ok());
        assert!(writer
            .handle_event(Event::StartTableRow { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartPreformatted {
                id: None,
                syntax: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            !json.contains("\"type\":\"codeBlock\""),
            "preformatted inside table cell must be dropped"
        );
    }

    #[test]
    fn thematic_break_inside_table_cell_is_dropped() {
        // Drives return_if_table_cell! return branch in ThematicBreak arm (line 891).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer.handle_event(Event::StartTable { id: None }).is_ok());
        assert!(writer
            .handle_event(Event::StartTableRow { id: None })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::ThematicBreak { id: None })
            .is_ok());
        assert!(writer.handle_event(Event::EndTableCell).is_ok());
        assert!(writer.handle_event(Event::EndTableRow).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            !json.contains("\"type\":\"divider\""),
            "thematic break inside table cell must be dropped"
        );
    }

    #[test]
    fn end_table_inside_list_hits_drop_macro() {
        // Drives drop_block_in_list_start! in StartTable arm (line 604) and
        // drop_block_in_list_end! in EndTable arm (line 348).
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer.handle_event(Event::StartTable { id: None }).is_ok());
        assert!(writer.handle_event(Event::EndTable).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            !json.contains("\"type\":\"table\""),
            "table inside list must be dropped"
        );
        assert!(
            json.contains("\"type\":\"bulletListItem\""),
            "list item must still be emitted"
        );
    }

    #[test]
    fn end_paragraph_in_normal_context_closes_paragraph_block() {
        // Drives close_text_block! in handle_end_paragraph normal path (line 344)
        // via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartParagraph {
                alignment: None,
                id: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "plain text".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"type\":\"paragraph\""),
            "paragraph block must be in output"
        );
        assert!(
            json.contains("\"text\":\"plain text\""),
            "paragraph text must be in output"
        );
        assert!(
            json.contains("\"children\":[]"),
            "paragraph must be properly closed with children:[]"
        );
    }

    #[test]
    fn with_assets_constructor_accepts_asset_provider() {
        // Drives BlockNoteWriter::with_assets constructor body (line 804).
        let provider = MockAssetProvider::new();
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::with_assets(&mut buf, &provider);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartParagraph {
                alignment: None,
                id: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "hello".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"text\":\"hello\""),
            "paragraph text must appear when using with_assets writer"
        );
    }

    #[test]
    fn level_down_to_nested_parent_breaks_loop() {
        // Drives the break statement (line 592) in the level-down while-loop of
        // handle_start_list_item: level-0 → level-1 → level-2 → back to level-1.
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "a".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "b".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 2,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "c".to_string(),
                style: TextStyle::default(),
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "d".to_string(),
                style: TextStyle::default(),
            },
            Event::EndDocument,
        ]);
        assert!(
            json.contains("\"text\":\"a\""),
            "root item text must appear"
        );
        assert!(
            json.contains("\"text\":\"d\""),
            "sibling item at level 1 must appear"
        );
    }

    #[test]
    fn multi_paragraph_list_item_raw_covers_second_para_paths() {
        // Drives handle_paragraph second-para path (lines 477, 488) and
        // handle_end_paragraph second-para-close path (line 324) via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartParagraph {
                alignment: None,
                id: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "first".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer
            .handle_event(Event::StartParagraph {
                alignment: None,
                id: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "second".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"text\":\"first\""),
            "first paragraph text must appear in list item content"
        );
        assert!(
            json.contains("\"text\":\"second\""),
            "second paragraph text must appear in children"
        );
        assert!(
            json.contains("\"type\":\"paragraph\""),
            "child paragraph block must be emitted"
        );
    }

    #[test]
    fn open_current_list_item_children_state_paths() {
        // Drives open_current_list_item_children with content_array_open=true (line 726)
        // and children_array_open=false (line 737) via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "parent".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "child".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"text\":\"parent\""),
            "parent item text must appear"
        );
        assert!(
            json.contains("\"text\":\"child\""),
            "child item text must appear in children array"
        );
    }

    #[test]
    fn open_list_item_object_with_valid_start_value() {
        // Drives u32::try_from success path for the ordered-list `start` prop
        // and ListStackEntry push fields in open_list_item_object.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(42),
                style_type: docspec_core::ListStyleType::Decimal,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "item 42".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"start\":42"),
            "start prop must be emitted for ordered list with start=42"
        );
        assert!(
            json.contains("\"text\":\"item 42\""),
            "list item text must be emitted"
        );
    }

    #[test]
    fn image_event_with_alt_and_id_fields() {
        // Drives Image match arm pattern bindings (lines 900-901) via raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
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
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"type\":\"image\""),
            "image block must appear"
        );
        assert!(
            json.contains("\"caption\":\"alt text\""),
            "alt text must appear as caption"
        );
        assert!(json.contains("\"id\":\"img-1\""), "image id must appear");
    }

    #[test]
    fn ordered_list_item_with_id_and_start_fields() {
        // Drives StartOrderedListItem match arm pattern bindings (lines 904-905) via
        // raw BlockNoteWriter.
        let mut buf = Vec::<u8>::new();
        let mut writer = BlockNoteWriter::new(&mut buf);
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartOrderedListItem {
                id: Some("li-1".to_string()),
                level: 0,
                start: Some(5),
                style_type: docspec_core::ListStyleType::Decimal,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "five".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndOrderedListItem).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        assert!(json.contains("\"id\":\"li-1\""), "list item id must appear");
        assert!(json.contains("\"start\":5"), "start prop must appear");
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
            assert!(writer
                .handle_event(Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                })
                .is_ok());
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
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "before".to_string(),
                style: TextStyle::default(),
            },
            // Heading inside list item → dropped
            Event::StartHeading { id: None, level: 1 },
            Event::Text {
                content: "heading".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            // Preformatted inside list item → dropped
            Event::StartPreformatted {
                id: None,
                syntax: None,
            },
            Event::Text {
                content: "code".to_string(),
                style: TextStyle::default(),
            },
            Event::EndPreformatted,
            // BlockQuote inside list item → dropped
            Event::StartBlockQuote { id: None },
            Event::Text {
                content: "quote".to_string(),
                style: TextStyle::default(),
            },
            Event::EndBlockQuote,
            // ThematicBreak inside list item → dropped
            Event::ThematicBreak { id: None },
            Event::Text {
                content: "after".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        // Only "before" and "after" should appear; all block content dropped
        assert!(
            json.contains("\"text\":\"before\""),
            "before text must be preserved"
        );
        assert!(
            json.contains("\"text\":\"after\""),
            "after text must be preserved"
        );
        assert!(
            !json.contains("\"text\":\"heading\""),
            "heading text must be dropped"
        );
        assert!(
            !json.contains("\"text\":\"code\""),
            "code text must be dropped"
        );
        assert!(
            !json.contains("\"text\":\"quote\""),
            "quote text must be dropped"
        );
        assert!(!json.contains("\"type\":\"heading\""), "no heading block");
        assert!(!json.contains("\"type\":\"codeBlock\""), "no code block");
        assert!(!json.contains("\"type\":\"quote\""), "no quote block");
    }

    fn list_item_with_children_transition_then(block_events: Vec<Event>) -> String {
        let mut events = vec![
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "first".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "second".to_string(),
                style: TextStyle::default(),
            },
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
        assert!(
            !json.contains("\"type\":\"image\""),
            "image after content\u{2192}children transition must be dropped: {json}"
        );
        assert!(
            !json.contains("leaked"),
            "image url/alt must not leak to output: {json}"
        );
        assert!(
            json.starts_with("[{\"type\":\"bulletListItem\""),
            "bulletListItem must remain the only top-level block: {json}"
        );
    }

    #[test]
    fn thematic_break_after_children_transition_inside_list_item_is_dropped() {
        let json = list_item_with_children_transition_then(vec![Event::ThematicBreak { id: None }]);
        assert!(
            !json.contains("\"type\":\"divider\""),
            "thematic break after content\u{2192}children transition must be dropped: {json}"
        );
        assert!(
            json.starts_with("[{\"type\":\"bulletListItem\""),
            "bulletListItem must remain the only top-level block: {json}"
        );
    }

    #[test]
    fn heading_after_children_transition_inside_list_item_is_dropped() {
        let json = list_item_with_children_transition_then(vec![
            Event::StartHeading { id: None, level: 2 },
            Event::Text {
                content: "leaked-heading".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
        ]);
        assert!(
            !json.contains("\"type\":\"heading\""),
            "heading after content\u{2192}children transition must be dropped: {json}"
        );
        assert!(
            !json.contains("leaked-heading"),
            "heading text must not leak: {json}"
        );
        assert!(
            json.starts_with("[{\"type\":\"bulletListItem\""),
            "bulletListItem must remain the only top-level block: {json}"
        );
    }

    #[test]
    fn blockquote_after_children_transition_inside_list_item_is_dropped() {
        let json = list_item_with_children_transition_then(vec![
            Event::StartBlockQuote { id: None },
            Event::Text {
                content: "leaked-quote".to_string(),
                style: TextStyle::default(),
            },
            Event::EndBlockQuote,
        ]);
        assert!(
            !json.contains("\"type\":\"quote\""),
            "blockquote after content\u{2192}children transition must be dropped: {json}"
        );
        assert!(
            !json.contains("leaked-quote"),
            "blockquote text must not leak: {json}"
        );
        assert!(
            json.starts_with("[{\"type\":\"bulletListItem\""),
            "bulletListItem must remain the only top-level block: {json}"
        );
    }

    #[test]
    fn preformatted_after_children_transition_inside_list_item_is_dropped() {
        let json = list_item_with_children_transition_then(vec![
            Event::StartPreformatted {
                id: None,
                syntax: None,
            },
            Event::Text {
                content: "leaked-code".to_string(),
                style: TextStyle::default(),
            },
            Event::EndPreformatted,
        ]);
        assert!(
            !json.contains("\"type\":\"codeBlock\""),
            "preformatted after content\u{2192}children transition must be dropped: {json}"
        );
        assert!(
            !json.contains("leaked-code"),
            "preformatted text must not leak: {json}"
        );
        assert!(
            json.starts_with("[{\"type\":\"bulletListItem\""),
            "bulletListItem must remain the only top-level block: {json}"
        );
    }

    #[test]
    fn table_after_children_transition_inside_list_item_is_dropped() {
        let json = list_item_with_children_transition_then(vec![
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::Text {
                content: "leaked-cell".to_string(),
                style: TextStyle::default(),
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
        ]);
        assert!(
            !json.contains("\"type\":\"table\""),
            "table after content\u{2192}children transition must be dropped: {json}"
        );
        assert!(
            !json.contains("leaked-cell"),
            "table cell text must not leak: {json}"
        );
        assert!(
            json.starts_with("[{\"type\":\"bulletListItem\""),
            "bulletListItem must remain the only top-level block: {json}"
        );
    }

    #[test]
    fn ordered_list_item_with_explicit_start_emits_start_prop() {
        // Exercises line 771: start prop write for ordered list items
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(3),
                style_type: docspec_core::ListStyleType::Decimal,
            },
            Event::Text {
                content: "item".to_string(),
                style: TextStyle::default(),
            },
            Event::EndOrderedListItem,
            Event::EndDocument,
        ]);
        assert!(
            json.contains("\"start\":3"),
            "start prop must be emitted with value 3"
        );
        assert!(
            json.contains("\"type\":\"numberedListItem\""),
            "must be numberedListItem"
        );
    }

    #[test]
    fn multi_paragraph_list_item_second_paragraph_dispatch() {
        // Exercises lines 477, 488, 324, 344 in handle_paragraph and handle_end_paragraph
        // for the second-and-subsequent paragraph case. Direct event emission without
        // StackTrackingSink to isolate the dispatch logic.
        let mut buf = Vec::<u8>::new();
        let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
        assert!(writer
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::StartParagraph {
                alignment: None,
                id: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "first".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer
            .handle_event(Event::StartParagraph {
                alignment: None,
                id: None,
            })
            .is_ok());
        assert!(writer
            .handle_event(Event::Text {
                content: "second".to_string(),
                style: TextStyle::default(),
            })
            .is_ok());
        assert!(writer.handle_event(Event::EndParagraph).is_ok());
        assert!(writer.handle_event(Event::EndUnorderedListItem).is_ok());
        assert!(writer.handle_event(Event::EndDocument).is_ok());
        let json = String::from_utf8(writer.finish().map(|()| buf.clone()).unwrap_or_default())
            .unwrap_or_default();
        // First paragraph inline content in item's content array
        assert!(
            json.contains("\"text\":\"first\""),
            "first paragraph text must be in content"
        );
        // Second paragraph becomes a child block
        assert!(
            json.contains("\"children\":[{\"type\":\"paragraph\""),
            "second paragraph must be a child block"
        );
        assert!(
            json.contains("\"text\":\"second\""),
            "second paragraph text must appear"
        );
    }

    #[test]
    fn image_after_end_list_item_appears_as_top_level_sibling() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "item".to_string(),
                style: TextStyle::default(),
            },
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
        assert!(
            json.contains("\"bulletListItem\""),
            "list item must be emitted: {json}"
        );
        assert!(
            json.contains("\"type\":\"image\""),
            "image after EndListItem must drain the list stack and emit, not silently drop: {json}"
        );
        assert!(
            json.contains("https://example.com/foo.png"),
            "image url must appear: {json}"
        );
        assert!(
            json.contains("\"type\":\"divider\""),
            "thematic break after EndListItem must drain the list stack and emit, not silently drop: {json}"
        );
    }

    #[test]
    fn heading_after_end_list_item_appears_as_top_level_sibling() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "item".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::StartHeading { level: 2, id: None },
            Event::Text {
                content: "After list".to_string(),
                style: TextStyle::default(),
            },
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert!(
            json.contains("\"bulletListItem\""),
            "list item must be emitted: {json}"
        );
        assert!(
            json.contains("\"type\":\"heading\""),
            "heading after EndListItem must drain the list stack and emit, not silently drop: {json}"
        );
        assert!(
            json.contains("\"text\":\"After list\""),
            "heading inline content must appear: {json}"
        );
    }

    #[test]
    fn start_list_item_inside_dropped_block_in_list_item_is_silently_dropped() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "outer".to_string(),
                style: TextStyle::default(),
            },
            Event::StartBlockQuote { id: None },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::Text {
                content: "inner".to_string(),
                style: TextStyle::default(),
            },
            Event::EndUnorderedListItem,
            Event::EndBlockQuote,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(
            json.contains("\"text\":\"outer\""),
            "outer list item text must appear: {json}"
        );
        assert!(
            !json.contains("\"text\":\"inner\""),
            "inner list item inside dropped blockquote must not corrupt JSON or emit text: {json}"
        );
        let bullet_count = json.matches("\"bulletListItem\"").count();
        assert_eq!(
            bullet_count, 1,
            "only the outer list item must be emitted; inner is dropped: {json}"
        );
    }

    #[test]
    fn paragraph_events_inside_dropped_block_in_list_item_are_fully_absorbed() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartBlockQuote { id: None },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "dropped".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "real".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(
            !json.contains("\"text\":\"dropped\""),
            "text inside dropped blockquote must not appear: {json}"
        );
        assert!(
            json.contains("\"text\":\"real\""),
            "real text must appear after the dropped block: {json}"
        );
        assert!(
            !json.contains("\"children\":[{\"type\":\"paragraph\""),
            "real paragraph must populate content[] not children[]; the dropped paragraph events must not have set first_paragraph_consumed: {json}"
        );
    }

    #[test]
    fn continuation_paragraph_after_nested_list_attaches_to_parent_item() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "outer".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "nested".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "continuation".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        let nested_block = r#"{"type":"bulletListItem","props":{"backgroundColor":"default","textColor":"default","textAlignment":"left"},"content":[{"type":"text","text":"nested","styles":{}}],"children":[]}"#;
        let continuation_block = r#"{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"continuation","styles":{}}],"children":[]}"#;
        let expected_children = format!(r#""children":[{nested_block},{continuation_block}]"#);
        assert!(
            json.contains(&expected_children),
            "continuation paragraph must be a sibling of the nested item inside the outer item's children[], not nested inside the nested item's children[]: {json}"
        );
    }

    // ============================================================================
    // LINK TESTS
    // ============================================================================

    #[test]
    fn link_simple() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            Event::Text {
                content: "text".to_string(),
                style: TextStyle::default(),
            },
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"text","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn nested_start_link_is_silently_ignored() {
        let json = run_direct_writer_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
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
            Event::Text {
                content: "inner".to_string(),
                style: TextStyle::default(),
            },
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        let parsed_result: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(parsed_result.is_ok(), "output must be valid JSON: {json}");
        assert_eq!(json.matches(r#""type":"link""#).count(), 1);
        assert!(json.contains(r#""href":"https://a.example""#));
        assert!(!json.contains(r#""href":"https://b.example""#));
        assert!(
            json.contains(r#""content":[{"type":"link","href":"https://a.example","content":[{"type":"text","text":"inner","styles":{}}]}]"#),
            "text must remain inside the outer link content: {json}"
        );
    }

    #[test]
    fn link_left_open_at_paragraph_end_is_defensively_closed() {
        let json = run_direct_writer_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::StartLink {
                href: "https://x.example".to_string(),
                title: None,
                id: None,
            },
            Event::Text {
                content: "label".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        let parsed_result: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(parsed_result.is_ok(), "output must be valid JSON: {json}");
        assert_eq!(json.matches(r#""type":"link""#).count(), 1);
        assert!(json.contains(r#""href":"https://x.example""#));
        assert!(
            json.contains(r#"{"type":"text","text":"label","styles":{}}"#),
            "link label text must be preserved: {json}"
        );
    }

    #[test]
    fn link_left_open_at_table_cell_end_is_defensively_closed() {
        let json = run_direct_writer_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::StartLink {
                href: "https://cell.example".to_string(),
                title: None,
                id: None,
            },
            Event::Text {
                content: "cell".to_string(),
                style: TextStyle::default(),
            },
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        let parsed_result: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(parsed_result.is_ok(), "output must be valid JSON: {json}");
        assert_eq!(json.matches(r#""type":"link""#).count(), 1);
        assert!(json.contains(r#""href":"https://cell.example""#));
        assert!(
            json.contains(r#"{"type":"text","text":"cell","styles":{}}"#),
            "table cell link text must be preserved: {json}"
        );
    }

    #[test]
    fn link_empty_content_emits_empty_styled_text() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
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
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn link_drops_title_field() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: Some("a title".to_string()),
                id: None,
            },
            Event::Text {
                content: "text".to_string(),
                style: TextStyle::default(),
            },
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert!(
            !json.contains(r#""title""#),
            "title field must not appear in BlockNote link JSON, got: {json}"
        );
        assert!(
            json.contains(r#""type":"link""#),
            "link object must be present"
        );
        assert!(
            json.contains(r#""href":"https://example.com""#),
            "href must be present"
        );
    }

    #[test]
    fn link_with_styled_content_array() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            Event::Text {
                content: "bold".to_string(),
                style: TextStyle::default().bold(),
            },
            Event::Text {
                content: "italic".to_string(),
                style: TextStyle::default().italic(),
            },
            Event::Text {
                content: "plain".to_string(),
                style: TextStyle::default(),
            },
            Event::EndLink,
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"bold","styles":{"bold":true}},{"type":"text","text":"italic","styles":{"italic":true}},{"type":"text","text":"plain","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn link_in_paragraph_alongside_other_text() {
        let json = run_events(&[
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
                content: "before ".to_string(),
                style: TextStyle::default(),
            },
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            Event::Text {
                content: "link".to_string(),
                style: TextStyle::default(),
            },
            Event::EndLink,
            Event::Text {
                content: " after".to_string(),
                style: TextStyle::default(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"before ","styles":{}},{"type":"link","href":"https://example.com","content":[{"type":"text","text":"link","styles":{}}]},{"type":"text","text":" after","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn link_in_heading() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartHeading { level: 1, id: None },
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            Event::Text {
                content: "title link".to_string(),
                style: TextStyle::default(),
            },
            Event::EndLink,
            Event::EndHeading,
            Event::EndDocument,
        ]);
        assert_eq!(
            json,
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"title link","styles":{}}]}],"children":[]}]"#
        );
    }

    #[test]
    fn link_in_list_item() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
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
            Event::Text {
                content: "link".to_string(),
                style: TextStyle::default(),
            },
            Event::EndLink,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(json.contains(r#""type":"bulletListItem""#));
        assert!(json.contains(r#""type":"link""#));
        assert!(json.contains(r#""href":"https://example.com""#));
        assert!(
            json.contains(r#"{"type":"text","text":"link","styles":{}}"#),
            "link content must preserve styled text: {json}"
        );
    }

    #[test]
    fn link_in_blockquote() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            Event::Text {
                content: "link".to_string(),
                style: TextStyle::default(),
            },
            Event::EndLink,
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::EndDocument,
        ]);
        assert!(json.contains(r#""type":"quote""#));
        assert!(json.contains(r#""type":"link""#));
        assert!(json.contains(r#""href":"https://example.com""#));
    }

    #[test]
    fn empty_link_in_blockquote() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartBlockQuote { id: None },
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
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
        assert!(json.contains(r#""type":"quote""#));
        assert!(
            json.contains(r#""content":[{"type":"link","href":"https://example.com","content":[{"type":"text","text":"","styles":{}}]}]"#),
            "quote content must contain the empty link fallback: {json}"
        );
    }

    #[test]
    fn link_in_table_cell() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartTable { id: None },
            Event::StartTableRow { id: None },
            Event::StartTableCell {
                id: None,
                colspan: None,
                rowspan: None,
            },
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
            Event::Text {
                content: "link".to_string(),
                style: TextStyle::default(),
            },
            Event::EndLink,
            Event::EndTableCell,
            Event::EndTableRow,
            Event::EndTable,
            Event::EndDocument,
        ]);
        assert!(json.contains(r#""type":"table""#));
        assert!(json.contains(r#""type":"link""#));
        assert!(json.contains(r#""href":"https://example.com""#));
    }

    #[test]
    fn link_in_dropped_heading_inside_list_emits_no_link() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
            Event::StartHeading { level: 1, id: None },
            Event::StartLink {
                href: "https://x".to_string(),
                title: None,
                id: None,
            },
            Event::Text {
                content: "hidden".to_string(),
                style: TextStyle::default(),
            },
            Event::EndLink,
            Event::EndHeading,
            Event::EndUnorderedListItem,
            Event::EndDocument,
        ]);
        assert!(
            json.contains(r#""type":"bulletListItem""#),
            "list item must still be emitted: {json}"
        );
        assert!(
            !json.contains(r#""type":"link""#),
            "dropped heading content must not leak a phantom link: {json}"
        );
    }
}
