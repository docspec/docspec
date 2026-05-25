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
        AssetProvider, Event, EventSink as _, ImageSource, StackTrackingSink, TextStyle,
    };

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
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Item","styles":{}}],"children":[]}]"#
        );
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
}
