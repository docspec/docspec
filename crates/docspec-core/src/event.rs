//! Event types for the streaming document pipeline.
//!
//! Events represent the atomic units of document structure. Sources emit events
//! in document order; sinks consume them. This decouples all readers from all writers.

/// A streaming document event.
///
/// Events flow from [`crate::EventSource`] readers to [`crate::EventSink`] writers. The enum is
/// marked `#[non_exhaustive]` to allow adding new event types in future versions.
///
/// Events come in three categories:
/// - **Start/End pairs**: Container elements like headings, paragraphs, tables
/// - **Self-contained**: Standalone elements like text, images, line breaks
/// - **Block vs Inline**: Block events create new vertical sections; inline events flow within blocks
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// End a block quote.
    EndBlockQuote,

    /// End a table caption.
    EndCaption,

    /// End a definition detail.
    EndDefinitionDetail,

    /// End a definition list.
    EndDefinitionList,

    /// End a definition term.
    EndDefinitionTerm,

    /// End a document.
    EndDocument,

    /// End a footnote definition.
    EndFootnote,

    /// End a heading.
    EndHeading,

    /// End a hyperlink.
    EndLink,

    /// End a list item.
    EndListItem,

    /// End a paragraph.
    EndParagraph,

    /// End a preformatted block.
    EndPreformatted,

    /// End a table.
    EndTable,

    /// End a table data cell.
    EndTableCell,

    /// End a table header cell.
    EndTableHeader,

    /// End a table row.
    EndTableRow,

    /// A reference to a footnote.
    FootnoteRef {
        /// The footnote identifier being referenced.
        id: u32,
    },

    /// An image reference.
    Image {
        /// Alternative text for accessibility.
        alt: Option<String>,
        /// Whether the image is purely decorative (no alt text needed).
        decorative: bool,
        /// Source of the image (embedded asset or external URI).
        source: crate::ImageSource,
        /// Optional tooltip text.
        title: Option<String>,
    },

    /// A hard line break within a paragraph.
    LineBreak,

    /// Begin a block quote.
    StartBlockQuote,

    /// Begin a table caption.
    StartCaption,

    /// Begin a definition detail (description).
    StartDefinitionDetail,

    /// Begin a definition list.
    StartDefinitionList,

    /// Begin a definition term.
    StartDefinitionTerm,

    /// Begin a document with optional language and metadata.
    StartDocument {
        /// BCP 47 language tag (e.g., "en", "en-US", "zh-Hans").
        language: Option<String>,
        /// Document metadata including title, authors, and description.
        metadata: Option<crate::DocumentMeta>,
    },

    /// Begin a footnote definition.
    StartFootnote {
        /// Unique identifier for this footnote.
        id: u32,
    },

    /// Begin a heading of the given level.
    StartHeading {
        /// Heading level, 1–9 (1 is most prominent).
        level: u8,
    },

    /// Begin a hyperlink.
    StartLink {
        /// URL or URI target of the link.
        href: String,
        /// Optional tooltip text.
        title: Option<String>,
    },

    /// Begin a list item.
    StartListItem {
        /// Nesting level (1 = top-level).
        level: u8,
        /// Whether the list is ordered or unordered.
        list_type: crate::ListType,
        /// Starting number for ordered lists (None = continue from previous).
        start: Option<u32>,
        /// Visual style for the list marker.
        style_type: Option<crate::ListStyleType>,
    },

    /// Begin a paragraph with optional alignment.
    StartParagraph {
        /// Text alignment for the paragraph.
        alignment: Option<crate::TextAlignment>,
    },

    /// Begin a preformatted (code) block with optional syntax highlighting.
    StartPreformatted {
        /// Language identifier for syntax highlighting (e.g., "rust", "python").
        syntax: Option<String>,
    },

    /// Begin a table.
    StartTable,

    /// Begin a table data cell.
    StartTableCell {
        /// Number of columns this cell spans.
        colspan: Option<u32>,
        /// Number of rows this cell spans.
        rowspan: Option<u32>,
    },

    /// Begin a table header cell.
    StartTableHeader {
        /// Abbreviated content for accessibility.
        abbr: Option<String>,
        /// Number of columns this cell spans.
        colspan: Option<u32>,
        /// Number of rows this cell spans.
        rowspan: Option<u32>,
        /// Whether this header applies to a column or row.
        scope: Option<crate::TableHeaderScope>,
    },

    /// Begin a table row.
    StartTableRow,

    /// A text run with formatting attributes.
    Text {
        /// Bold formatting.
        bold: bool,
        /// Monospace/code formatting.
        code: bool,
        /// The text content.
        content: String,
        /// Italic formatting.
        italic: bool,
        /// Highlight/mark color.
        mark: Option<crate::Color>,
        /// Strikethrough formatting.
        strikethrough: bool,
        /// Subscript formatting.
        subscript: bool,
        /// Superscript formatting.
        superscript: bool,
        /// Underline formatting.
        underline: bool,
    },

    /// A horizontal rule / thematic break.
    ThematicBreak,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
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
        let event1 = Event::StartHeading { level: 1 };
        let event2 = Event::StartHeading { level: 2 };
        assert_ne!(event1, event2);
    }

    #[test]
    fn partial_eq_different_variants() {
        let event1 = Event::StartHeading { level: 1 };
        let event2 = Event::EndHeading;
        assert_ne!(event1, event2);
    }

    #[test]
    fn partial_eq_same_variant() {
        let event1 = Event::StartHeading { level: 2 };
        let event2 = Event::StartHeading { level: 2 };
        assert_eq!(event1, event2);
    }

    #[test]
    fn partial_eq_unit_variants() {
        assert_eq!(Event::EndDocument, Event::EndDocument);
        assert_eq!(Event::ThematicBreak, Event::ThematicBreak);
        assert_eq!(Event::LineBreak, Event::LineBreak);
        assert_ne!(Event::EndDocument, Event::ThematicBreak);
    }

    #[test]
    fn start_block_quote() {
        let event = Event::StartBlockQuote;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_caption() {
        let event = Event::StartCaption;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_definition_detail() {
        let event = Event::StartDefinitionDetail;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_definition_list() {
        let event = Event::StartDefinitionList;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_definition_term() {
        let event = Event::StartDefinitionTerm;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_document_minimal() {
        let event = Event::StartDocument {
            language: None,
            metadata: None,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_document_with_language() {
        let event = Event::StartDocument {
            language: Some("en-US".to_string()),
            metadata: None,
        };
        assert_eq!(
            event,
            Event::StartDocument {
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
            language: Some("en".to_string()),
            metadata: Some(meta.clone()),
        };
        assert_eq!(
            event,
            Event::StartDocument {
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
        let event = Event::StartHeading { level: 1 };
        let cloned = event.clone();
        assert_eq!(event, cloned);
        assert_eq!(event, Event::StartHeading { level: 1 });
    }

    #[test]
    fn start_heading_levels() {
        for lvl in 1..=9 {
            let event = Event::StartHeading { level: lvl };
            assert_eq!(event, Event::StartHeading { level: lvl });
        }
    }

    #[test]
    fn start_link() {
        let event = Event::StartLink {
            href: "https://example.com".to_string(),
            title: Some("Example Link".to_string()),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_link_no_title() {
        let event = Event::StartLink {
            href: "https://rust-lang.org".to_string(),
            title: None,
        };
        assert_eq!(
            event,
            Event::StartLink {
                href: "https://rust-lang.org".to_string(),
                title: None,
            }
        );
    }

    #[test]
    fn start_list_item_ordered() {
        let event = Event::StartListItem {
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
            level: 2,
            list_type: ListType::Unordered,
            start: None,
            style_type: Some(ListStyleType::Disc),
        };
        assert_eq!(
            event,
            Event::StartListItem {
                level: 2,
                list_type: ListType::Unordered,
                start: None,
                style_type: Some(ListStyleType::Disc),
            }
        );
    }

    #[test]
    fn start_paragraph_no_alignment() {
        let event = Event::StartParagraph { alignment: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_paragraph_with_alignment() {
        let event = Event::StartParagraph {
            alignment: Some(TextAlignment::Center),
        };
        assert_eq!(
            event,
            Event::StartParagraph {
                alignment: Some(TextAlignment::Center),
            }
        );
    }

    #[test]
    fn start_preformatted_no_syntax() {
        let event = Event::StartPreformatted { syntax: None };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_preformatted_with_syntax() {
        let event = Event::StartPreformatted {
            syntax: Some("rust".to_string()),
        };
        assert_eq!(
            event,
            Event::StartPreformatted {
                syntax: Some("rust".to_string()),
            }
        );
    }

    #[test]
    fn start_table() {
        let event = Event::StartTable;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_table_cell_minimal() {
        let event = Event::StartTableCell {
            colspan: None,
            rowspan: None,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_table_cell_with_spans() {
        let event = Event::StartTableCell {
            colspan: Some(3),
            rowspan: Some(2),
        };
        assert_eq!(
            event,
            Event::StartTableCell {
                colspan: Some(3),
                rowspan: Some(2),
            }
        );
    }

    #[test]
    fn start_table_header_full() {
        let event = Event::StartTableHeader {
            scope: Some(TableHeaderScope::Column),
            abbr: Some("Qty".to_string()),
            colspan: Some(2),
            rowspan: Some(1),
        };
        assert_eq!(
            event,
            Event::StartTableHeader {
                scope: Some(TableHeaderScope::Column),
                abbr: Some("Qty".to_string()),
                colspan: Some(2),
                rowspan: Some(1),
            }
        );
    }

    #[test]
    fn start_table_header_minimal() {
        let event = Event::StartTableHeader {
            scope: None,
            abbr: None,
            colspan: None,
            rowspan: None,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn start_table_row() {
        let event = Event::StartTableRow;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn text_all_nine_fields_accessible() {
        let event = Event::Text {
            content: "Formatted text".to_string(),
            bold: true,
            italic: true,
            code: true,
            strikethrough: true,
            underline: true,
            subscript: true,
            superscript: true,
            mark: Some(Color::Rgb {
                r: 255,
                g: 255,
                b: 0,
            }),
        };
        assert_eq!(
            event,
            Event::Text {
                content: "Formatted text".to_string(),
                bold: true,
                italic: true,
                code: true,
                strikethrough: true,
                underline: true,
                subscript: true,
                superscript: true,
                mark: Some(Color::Rgb {
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
            bold: false,
            italic: false,
            code: false,
            strikethrough: false,
            underline: false,
            subscript: false,
            superscript: false,
            mark: None,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn text_with_bold_only() {
        let event = Event::Text {
            content: "Bold text".to_string(),
            bold: true,
            italic: false,
            code: false,
            strikethrough: false,
            underline: false,
            subscript: false,
            superscript: false,
            mark: None,
        };
        assert_eq!(
            event,
            Event::Text {
                content: "Bold text".to_string(),
                bold: true,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            }
        );
    }

    #[test]
    fn text_with_mark_color() {
        let event = Event::Text {
            content: "Highlighted".to_string(),
            bold: false,
            italic: false,
            code: false,
            strikethrough: false,
            underline: false,
            subscript: false,
            superscript: false,
            mark: Some(Color::Rgb {
                r: 255,
                g: 255,
                b: 0,
            }),
        };
        assert_eq!(
            event,
            Event::Text {
                content: "Highlighted".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: Some(Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 0,
                }),
            }
        );
    }

    #[test]
    fn thematic_break() {
        let event = Event::ThematicBreak;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }
}
