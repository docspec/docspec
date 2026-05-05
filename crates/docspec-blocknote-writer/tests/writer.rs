//! Integration tests for `BlockNoteWriter`.

extern crate alloc;

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use std::collections::HashMap;
    use std::io;
    use std::io::Write;

    use docspec_blocknote_writer::BlockNoteWriter;
    use docspec_core::{AssetProvider, Event, EventSink as _, ImageSource, StackTrackingSink};

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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: true,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: true,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: true,
                italic: true,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::EndParagraph,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Second".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::EndHeading,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Body".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: true,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::EndParagraph,
            Event::EndBlockQuote,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "normal".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::LineBreak,
            Event::Text {
                content: "line2".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::LineBreak,
            Event::Text {
                content: "line3".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::EndHeading,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Body".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::EndParagraph,
            Event::StartBlockQuote { id: None },
            Event::Text {
                content: "Quote".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::EndParagraph,
            Event::StartBlockQuote { id: None },
            Event::Text {
                content: "Quote".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::EndBlockQuote,
            Event::StartHeading { level: 2, id: None },
            Event::Text {
                content: "Head".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
    fn list_item_and_table_tracked_on_stack() {
        let json = run_events(&[
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartListItem {
                id: None,
                level: 1,
                list_type: docspec_core::ListType::Unordered,
                start: None,
                style_type: None,
            },
            Event::Text {
                content: "Item".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::EndListItem,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::Text {
                content: "World".to_string(),
                bold: true,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::EndParagraph,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Second".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            },
            Event::EndHeading,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
            Event::Text {
                content: "Body".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                    bold: false,
                    italic: false,
                    code: false,
                    strikethrough: false,
                    underline: false,
                    subscript: false,
                    superscript: false,
                    mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
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
}
