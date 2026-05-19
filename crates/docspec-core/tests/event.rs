//! Tests.

#[cfg(test)]
mod tests {
    use docspec_core::*;
    use docspec_core::{
        Author, Color, DocumentMeta, ImageSource, ListStyleType, ListType, TableHeaderScope,
        TextAlignment,
    };

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
    fn end_list_item() {
        let event = Event::EndListItem;
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
    fn footnote_ref() {
        let event = Event::FootnoteRef { id: 42 };
        let cloned = event.clone();
        assert_eq!(event, cloned);
        assert_eq!(event, Event::FootnoteRef { id: 42 });
    }

    #[test]
    fn image_asset() {
        let event = Event::Image {
            source: ImageSource::Asset {
                asset_id: "img_001".to_string(),
            },
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
    fn start_list_item_ordered() {
        let event = Event::StartListItem {
            id: None,
            level: 1,
            list_type: ListType::Ordered,
            start: Some(1),
            style_type: Some(ListStyleType::Decimal),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_list_item_unordered() {
        let event = Event::StartListItem {
            id: None,
            level: 2,
            list_type: ListType::Unordered,
            start: None,
            style_type: Some(ListStyleType::Disc),
        };
        assert_eq!(
            event,
            Event::StartListItem {
                id: None,
                level: 2,
                list_type: ListType::Unordered,
                start: None,
                style_type: Some(ListStyleType::Disc),
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
    fn text_with_all_textstyle_fields() {
        let event = Event::Text {
            content: "Formatted text".to_string(),
            style: TextStyle::default()
                .bold()
                .italic()
                .code()
                .strikethrough()
                .underline()
                .subscript()
                .superscript()
                .mark(Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 0,
                }),
        };
        assert_eq!(
            event,
            Event::Text {
                content: "Formatted text".to_string(),
                style: TextStyle::default()
                    .bold()
                    .italic()
                    .code()
                    .strikethrough()
                    .underline()
                    .subscript()
                    .superscript()
                    .mark(Color::Rgb {
                        r: 255,
                        g: 255,
                        b: 0,
                    }),
            }
        );
    }

    #[test]
    fn text_plain() {
        let event = Event::Text {
            content: "Hello, world!".to_string(),
            style: TextStyle::default(),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn text_with_bold_only() {
        let event = Event::Text {
            content: "Bold text".to_string(),
            style: TextStyle::default().bold(),
        };
        assert_eq!(
            event,
            Event::Text {
                content: "Bold text".to_string(),
                style: TextStyle::default().bold(),
            }
        );
    }

    #[test]
    fn text_with_mark_color() {
        let event = Event::Text {
            content: "Highlighted".to_string(),
            style: TextStyle::default().mark(Color::Rgb {
                r: 255,
                g: 255,
                b: 0,
            }),
        };
        assert_eq!(
            event,
            Event::Text {
                content: "Highlighted".to_string(),
                style: TextStyle::default().mark(Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 0,
                }),
            }
        );
    }

    #[test]
    fn thematic_break() {
        let event = Event::ThematicBreak { id: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }
}
