//! Tests.

#[cfg(test)]
mod tests {
    use docspec_core::*;
    use docspec_core::{
        Author, Color, DocumentMeta, ImageSource, ListStyleType, TableHeaderScope, TextAlignment,
    };

    #[derive(Debug)]
    struct TestAssetHandle {
        asset_id: String,
        content_type: Option<String>,
    }

    impl docspec_core::AssetHandle for TestAssetHandle {
        fn content_type(&self) -> Option<std::borrow::Cow<'_, str>> {
            self.content_type.as_deref().map(std::borrow::Cow::Borrowed)
        }

        fn stream_to(&self, _w: &mut dyn std::io::Write) -> std::io::Result<u64> {
            Ok(0)
        }

        fn asset_id(&self) -> &str {
            &self.asset_id
        }
    }

    #[test]
    fn end_block_quote() {
        let event = Event::EndBlockQuote;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_caption() {
        let event = Event::EndCaption;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_definition_detail() {
        let event = Event::EndDefinitionDetail;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_definition_list() {
        let event = Event::EndDefinitionList;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_definition_term() {
        let event = Event::EndDefinitionTerm;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_document() {
        let event = Event::EndDocument;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_footnote() {
        let event = Event::EndFootnote;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_heading() {
        let event = Event::EndHeading;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_link() {
        let event = Event::EndLink;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_ordered_list_item() {
        let event = Event::EndOrderedListItem;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_unordered_list_item() {
        let event = Event::EndUnorderedListItem;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_paragraph() {
        let event = Event::EndParagraph;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_preformatted() {
        let event = Event::EndPreformatted;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_table() {
        let event = Event::EndTable;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_table_cell() {
        let event = Event::EndTableCell;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_table_header() {
        let event = Event::EndTableHeader;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_table_row() {
        let event = Event::EndTableRow;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn end_text_style() {
        let event = Event::EndTextStyle;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn footnote_ref() {
        let event = Event::FootnoteRef { id: 42 };
        let cloned = event.clone();
        assert_eq!(event, cloned);
        assert_eq!(event, Event::FootnoteRef { id: 42 });
    }

    #[test]
    fn image_asset() {
        let event = Event::Image {
            source: ImageSource::Asset(std::sync::Arc::new(TestAssetHandle {
                asset_id: "img_001".to_string(),
                content_type: None,
            })),
            alt: Some("A picture".to_string()),
            title: Some("Image Title".to_string()),
            decorative: false,
            id: None,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn image_uri() {
        let event = Event::Image {
            source: ImageSource::Uri {
                uri: "https://example.com/image.png".to_string(),
            },
            alt: None,
            title: None,
            decorative: true,
            id: None,
        };
        assert_eq!(
            event,
            Event::Image {
                source: ImageSource::Uri {
                    uri: "https://example.com/image.png".to_string(),
                },
                alt: None,
                title: None,
                decorative: true,
                id: None,
            }
        );
    }

    #[test]
    fn line_break() {
        let event = Event::LineBreak;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn soft_break() {
        let event = Event::SoftBreak;
        assert_eq!(event, Event::SoftBreak);
    }

    #[test]
    fn partial_eq_different_fields() {
        let event1 = Event::StartHeading { level: 1, id: None };
        let event2 = Event::StartHeading { level: 2, id: None };
        assert_ne!(event1, event2);
    }

    #[test]
    fn partial_eq_different_variants() {
        let event1 = Event::StartHeading { level: 1, id: None };
        let event2 = Event::EndHeading;
        assert_ne!(event1, event2);
    }

    #[test]
    fn partial_eq_same_variant() {
        let event1 = Event::StartHeading { level: 2, id: None };
        let event2 = Event::StartHeading { level: 2, id: None };
        assert_eq!(event1, event2);
    }

    #[test]
    fn partial_eq_unit_variants() {
        assert_eq!(Event::EndDocument, Event::EndDocument);
        assert_eq!(
            Event::ThematicBreak { id: None },
            Event::ThematicBreak { id: None }
        );
        assert_eq!(Event::LineBreak, Event::LineBreak);
        assert_ne!(Event::EndDocument, Event::ThematicBreak { id: None });
    }

    #[test]
    fn start_block_quote() {
        let event = Event::StartBlockQuote { id: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_caption() {
        let event = Event::StartCaption { id: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_definition_detail() {
        let event = Event::StartDefinitionDetail { id: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_definition_list() {
        let event = Event::StartDefinitionList { id: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_definition_term() {
        let event = Event::StartDefinitionTerm { id: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_document_minimal() {
        let event = Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_document_with_language() {
        let event = Event::StartDocument {
            id: None,
            language: Some("en-US".to_string()),
            metadata: None,
        };
        assert_eq!(
            event,
            Event::StartDocument {
                id: None,
                language: Some("en-US".to_string()),
                metadata: None,
            }
        );
    }

    #[test]
    fn start_document_with_metadata() {
        let meta = DocumentMeta {
            title: Some("Test Document".to_string()),
            authors: Some(vec![Author {
                name: "Test Author".to_string(),
                email: Some("test@example.com".to_string()),
            }]),
            description: Some("A test document".to_string()),
        };
        let event = Event::StartDocument {
            id: None,
            language: Some("en".to_string()),
            metadata: Some(meta.clone()),
        };
        assert_eq!(
            event,
            Event::StartDocument {
                id: None,
                language: Some("en".to_string()),
                metadata: Some(meta),
            }
        );
    }

    #[test]
    fn start_footnote() {
        let event = Event::StartFootnote { id: 1 };
        let cloned = event.clone();
        assert_eq!(event, cloned);
        assert_eq!(event, Event::StartFootnote { id: 1 });
    }

    #[test]
    fn start_heading() {
        let event = Event::StartHeading { level: 1, id: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
        assert_eq!(event, Event::StartHeading { level: 1, id: None });
    }

    #[test]
    fn start_heading_levels() {
        for lvl in 1..=9 {
            let event = Event::StartHeading {
                level: lvl,
                id: None,
            };
            assert_eq!(
                event,
                Event::StartHeading {
                    level: lvl,
                    id: None
                }
            );
        }
    }

    #[test]
    fn start_link() {
        let event = Event::StartLink {
            href: "https://example.com".to_string(),
            id: None,
            title: Some("Example Link".to_string()),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_link_no_title() {
        let event = Event::StartLink {
            href: "https://rust-lang.org".to_string(),
            id: None,
            title: None,
        };
        assert_eq!(
            event,
            Event::StartLink {
                href: "https://rust-lang.org".to_string(),
                id: None,
                title: None,
            }
        );
    }

    #[test]
    fn start_ordered_list_item() {
        let event = Event::StartOrderedListItem {
            id: None,
            level: 0,
            start: Some(1),
            style_type: ListStyleType::Decimal,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_unordered_list_item() {
        let event = Event::StartUnorderedListItem {
            id: None,
            level: 1,
            style_type: ListStyleType::Disc,
        };
        assert_eq!(
            event,
            Event::StartUnorderedListItem {
                id: None,
                level: 1,
                style_type: ListStyleType::Disc,
            }
        );
    }

    #[test]
    fn start_paragraph_no_alignment() {
        let event = Event::StartParagraph {
            alignment: None,
            id: None,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_paragraph_with_alignment() {
        let event = Event::StartParagraph {
            alignment: Some(TextAlignment::Center),
            id: None,
        };
        assert_eq!(
            event,
            Event::StartParagraph {
                alignment: Some(TextAlignment::Center),
                id: None,
            }
        );
    }

    #[test]
    fn start_preformatted_no_syntax() {
        let event = Event::StartPreformatted {
            id: None,
            syntax: None,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_preformatted_with_syntax() {
        let event = Event::StartPreformatted {
            id: None,
            syntax: Some("rust".to_string()),
        };
        assert_eq!(
            event,
            Event::StartPreformatted {
                id: None,
                syntax: Some("rust".to_string()),
            }
        );
    }

    #[test]
    fn start_table() {
        let event = Event::StartTable { id: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_table_cell_minimal() {
        let event = Event::StartTableCell {
            colspan: None,
            id: None,
            rowspan: None,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_table_cell_with_spans() {
        let event = Event::StartTableCell {
            colspan: Some(3),
            id: None,
            rowspan: Some(2),
        };
        assert_eq!(
            event,
            Event::StartTableCell {
                colspan: Some(3),
                id: None,
                rowspan: Some(2),
            }
        );
    }

    #[test]
    fn start_table_header_full() {
        let event = Event::StartTableHeader {
            abbr: Some("Qty".to_string()),
            colspan: Some(2),
            id: None,
            rowspan: Some(1),
            scope: Some(TableHeaderScope::Column),
        };
        assert_eq!(
            event,
            Event::StartTableHeader {
                abbr: Some("Qty".to_string()),
                colspan: Some(2),
                id: None,
                rowspan: Some(1),
                scope: Some(TableHeaderScope::Column),
            }
        );
    }

    #[test]
    fn start_table_header_minimal() {
        let event = Event::StartTableHeader {
            abbr: None,
            colspan: None,
            id: None,
            rowspan: None,
            scope: None,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_table_row() {
        let event = Event::StartTableRow { id: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_text_style_with_id() {
        let event = Event::StartTextStyle {
            kind: TextStyleKind::Bold,
            id: Some("strong-1".to_string()),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
        assert_eq!(
            event,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
                id: Some("strong-1".to_string()),
            }
        );
    }

    #[test]
    fn text_plain() {
        let event = Event::Text {
            content: "Hello, world!".to_string(),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn text_style_kind_bold() {
        let event = Event::StartTextStyle {
            kind: TextStyleKind::Bold,
            id: None,
        };
        assert_eq!(
            event,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
                id: None,
            }
        );
    }

    #[test]
    fn text_style_kind_italic() {
        let event = Event::StartTextStyle {
            kind: TextStyleKind::Italic,
            id: None,
        };
        assert_eq!(
            event,
            Event::StartTextStyle {
                kind: TextStyleKind::Italic,
                id: None,
            }
        );
    }

    #[test]
    fn text_style_kind_code() {
        let event = Event::StartTextStyle {
            kind: TextStyleKind::Code,
            id: None,
        };
        assert_eq!(
            event,
            Event::StartTextStyle {
                kind: TextStyleKind::Code,
                id: None,
            }
        );
    }

    #[test]
    fn text_style_kind_strikethrough() {
        let event = Event::StartTextStyle {
            kind: TextStyleKind::Strikethrough,
            id: None,
        };
        assert_eq!(
            event,
            Event::StartTextStyle {
                kind: TextStyleKind::Strikethrough,
                id: None,
            }
        );
    }

    #[test]
    fn text_style_kind_underline() {
        let event = Event::StartTextStyle {
            kind: TextStyleKind::Underline,
            id: None,
        };
        assert_eq!(
            event,
            Event::StartTextStyle {
                kind: TextStyleKind::Underline,
                id: None,
            }
        );
    }

    #[test]
    fn text_style_kind_subscript() {
        let event = Event::StartTextStyle {
            kind: TextStyleKind::Subscript,
            id: None,
        };
        assert_eq!(
            event,
            Event::StartTextStyle {
                kind: TextStyleKind::Subscript,
                id: None,
            }
        );
    }

    #[test]
    fn text_style_kind_superscript() {
        let event = Event::StartTextStyle {
            kind: TextStyleKind::Superscript,
            id: None,
        };
        assert_eq!(
            event,
            Event::StartTextStyle {
                kind: TextStyleKind::Superscript,
                id: None,
            }
        );
    }

    #[test]
    fn text_style_kind_mark() {
        let color = Color::Rgb {
            r: 255,
            g: 255,
            b: 0,
        };
        let event = Event::StartTextStyle {
            kind: TextStyleKind::Mark(color.clone()),
            id: None,
        };
        assert_eq!(
            event,
            Event::StartTextStyle {
                kind: TextStyleKind::Mark(color),
                id: None,
            }
        );
    }

    #[test]
    fn text_color_variant_round_trips() {
        let color = Color::Rgb {
            r: 17,
            g: 34,
            b: 51,
        };
        let kind = TextStyleKind::TextColor(color.clone());
        let cloned = kind.clone();
        assert_eq!(kind, cloned);
        let event = Event::StartTextStyle {
            kind: TextStyleKind::TextColor(color),
            id: None,
        };
        let cloned_event = event.clone();
        assert_eq!(event, cloned_event);
    }

    #[test]
    fn thematic_break() {
        let event = Event::ThematicBreak { id: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }
}
