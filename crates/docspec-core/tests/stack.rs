//! Stack module tests.

#[cfg(test)]
mod tests {
    use docspec_core::*;

    struct MockSink {
        events: Vec<Event>,
    }

    impl MockSink {
        fn new() -> Self {
            Self { events: Vec::new() }
        }
    }

    impl EventSink for MockSink {
        fn finish(self) -> Result<()> {
            Ok(())
        }

        fn handle_event(&mut self, event: Event) -> Result<()> {
            self.events.push(event);
            Ok(())
        }
    }

    fn send(sink: &mut StackTrackingSink<MockSink>, event: Event) {
        let result = sink.handle_event(event);
        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn block_kind_clone() {
        let kind = BlockKind::Paragraph;
        let cloned = kind;
        assert_eq!(kind, cloned);
    }

    #[test]
    fn block_kind_copy() {
        let kind = BlockKind::Heading;
        let copied: BlockKind = kind;
        assert_eq!(kind, copied);
    }

    #[test]
    fn block_kind_debug() {
        let kind = BlockKind::Document;
        let debug_str = format!("{kind:?}");
        assert_eq!(debug_str, "Document");
    }

    #[test]
    fn block_kind_eq() {
        assert_eq!(BlockKind::Paragraph, BlockKind::Paragraph);
        assert_ne!(BlockKind::Paragraph, BlockKind::Heading);
    }

    #[test]
    fn block_kind_for_end_blockquote() {
        let event = Event::EndBlockQuote;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Blockquote));
    }

    #[test]
    fn block_kind_for_end_caption() {
        let event = Event::EndCaption;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Caption));
    }

    #[test]
    fn block_kind_for_end_definition_detail() {
        let event = Event::EndDefinitionDetail;
        assert_eq!(
            block_kind_for_end(&event),
            Some(BlockKind::DefinitionDetail)
        );
    }

    #[test]
    fn block_kind_for_end_definition_list() {
        let event = Event::EndDefinitionList;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::DefinitionList));
    }

    #[test]
    fn block_kind_for_end_definition_term() {
        let event = Event::EndDefinitionTerm;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::DefinitionTerm));
    }

    #[test]
    fn block_kind_for_end_document() {
        let event = Event::EndDocument;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Document));
    }

    #[test]
    fn block_kind_for_end_footnote() {
        let event = Event::EndFootnote;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Footnote));
    }

    #[test]
    fn block_kind_for_end_heading() {
        let event = Event::EndHeading;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Heading));
    }

    #[test]
    fn block_kind_for_end_link() {
        let event = Event::EndLink;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Link));
    }

    #[test]
    fn block_kind_for_end_ordered_list_item() {
        let event = Event::EndOrderedListItem;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::OrderedListItem));
    }

    #[test]
    fn block_kind_for_end_unordered_list_item() {
        let event = Event::EndUnorderedListItem;
        assert_eq!(
            block_kind_for_end(&event),
            Some(BlockKind::UnorderedListItem)
        );
    }

    #[test]
    fn block_kind_for_end_paragraph() {
        let event = Event::EndParagraph;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Paragraph));
    }

    #[test]
    fn block_kind_for_end_preformatted() {
        let event = Event::EndPreformatted;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Preformatted));
    }

    #[test]
    fn block_kind_for_end_table() {
        let event = Event::EndTable;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::Table));
    }

    #[test]
    fn block_kind_for_end_table_cell() {
        let event = Event::EndTableCell;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::TableCell));
    }

    #[test]
    fn block_kind_for_end_table_header() {
        let event = Event::EndTableHeader;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::TableHeader));
    }

    #[test]
    fn block_kind_for_end_table_row() {
        let event = Event::EndTableRow;
        assert_eq!(block_kind_for_end(&event), Some(BlockKind::TableRow));
    }

    #[test]
    fn block_kind_for_end_text_returns_none() {
        let event = Event::Text {
            content: "hello".to_string(),
            style: TextStyle::default(),
        };
        assert_eq!(block_kind_for_end(&event), None);
    }

    #[test]
    fn block_kind_for_start_blockquote() {
        let event = Event::StartBlockQuote { id: None };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Blockquote));
    }

    #[test]
    fn block_kind_for_start_caption() {
        let event = Event::StartCaption { id: None };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Caption));
    }

    #[test]
    fn block_kind_for_start_definition_detail() {
        let event = Event::StartDefinitionDetail { id: None };
        assert_eq!(
            block_kind_for_start(&event),
            Some(BlockKind::DefinitionDetail)
        );
    }

    #[test]
    fn block_kind_for_start_definition_list() {
        let event = Event::StartDefinitionList { id: None };
        assert_eq!(
            block_kind_for_start(&event),
            Some(BlockKind::DefinitionList)
        );
    }

    #[test]
    fn block_kind_for_start_definition_term() {
        let event = Event::StartDefinitionTerm { id: None };
        assert_eq!(
            block_kind_for_start(&event),
            Some(BlockKind::DefinitionTerm)
        );
    }

    #[test]
    fn block_kind_for_start_document() {
        let event = Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Document));
    }

    #[test]
    fn block_kind_for_start_footnote() {
        let event = Event::StartFootnote { id: 1 };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Footnote));
    }

    #[test]
    fn block_kind_for_start_heading() {
        let event = Event::StartHeading { id: None, level: 1 };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Heading));
    }

    #[test]
    fn block_kind_for_start_link() {
        let event = Event::StartLink {
            href: "https://example.com".to_string(),
            id: None,
            title: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Link));
    }

    #[test]
    fn block_kind_for_start_ordered_list_item() {
        let event = Event::StartOrderedListItem {
            id: None,
            level: 0,
            start: Some(1),
            style_type: docspec_core::ListStyleType::Decimal,
        };
        assert_eq!(
            block_kind_for_start(&event),
            Some(BlockKind::OrderedListItem)
        );
    }

    #[test]
    fn block_kind_for_start_unordered_list_item() {
        let event = Event::StartUnorderedListItem {
            id: None,
            level: 0,
            style_type: docspec_core::ListStyleType::Disc,
        };
        assert_eq!(
            block_kind_for_start(&event),
            Some(BlockKind::UnorderedListItem)
        );
    }

    #[test]
    fn block_kind_for_start_paragraph() {
        let event = Event::StartParagraph {
            alignment: None,
            id: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Paragraph));
    }

    #[test]
    fn block_kind_for_start_preformatted() {
        let event = Event::StartPreformatted {
            id: None,
            syntax: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Preformatted));
    }

    #[test]
    fn block_kind_for_start_table() {
        let event = Event::StartTable { id: None };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::Table));
    }

    #[test]
    fn block_kind_for_start_table_cell() {
        let event = Event::StartTableCell {
            colspan: None,
            id: None,
            rowspan: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::TableCell));
    }

    #[test]
    fn block_kind_for_start_table_header() {
        let event = Event::StartTableHeader {
            abbr: None,
            colspan: None,
            id: None,
            rowspan: None,
            scope: None,
        };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::TableHeader));
    }

    #[test]
    fn block_kind_for_start_table_row() {
        let event = Event::StartTableRow { id: None };
        assert_eq!(block_kind_for_start(&event), Some(BlockKind::TableRow));
    }

    #[test]
    fn block_kind_for_start_text_returns_none() {
        let event = Event::Text {
            content: "hello".to_string(),
            style: TextStyle::default(),
        };
        assert_eq!(block_kind_for_start(&event), None);
    }

    #[test]
    fn new_creates_empty_stack() {
        let mock = MockSink::new();
        let sink = StackTrackingSink::new(mock);
        assert!(sink.stack().is_empty());
    }

    #[test]
    fn sink_finish_forwards_to_inner() {
        let mock = MockSink::new();
        let sink = StackTrackingSink::new(mock);
        let result = sink.finish();
        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn sink_handle_event_forwards_to_inner() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        let event = Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        };
        let result = sink.handle_event(event);
        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn stack_tracks_nesting() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(&mut sink, Event::StartTable { id: None });
        send(&mut sink, Event::StartTableRow { id: None });
        send(
            &mut sink,
            Event::StartTableCell {
                colspan: None,
                id: None,
                rowspan: None,
            },
        );

        assert!(sink.is_inside(BlockKind::Document));
        assert!(sink.is_inside(BlockKind::Table));
        assert!(sink.is_inside(BlockKind::TableRow));
        assert!(sink.is_inside(BlockKind::TableCell));
        assert!(!sink.is_inside(BlockKind::Paragraph));
        assert!(!sink.has_open_content());
    }

    #[test]
    fn mismatched_end_returns_error() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(&mut sink, Event::StartTable { id: None });

        let result = sink.handle_event(Event::EndBlockQuote);
        assert!(result.is_err());
        let err_str = format!("{result:?}");
        assert!(err_str.contains("Blockquote"));
    }

    #[test]
    fn end_without_start_returns_error() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        let result = sink.handle_event(Event::EndParagraph);
        assert!(result.is_err());
        let err_str = format!("{result:?}");
        assert!(err_str.contains("empty stack"));
    }

    #[test]
    fn end_document_without_start_returns_error() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        let result = sink.handle_event(Event::EndDocument);
        assert!(result.is_err());
        let err_str = format!("{result:?}");
        assert!(err_str.contains("EndDocument received without StartDocument"));
    }

    #[test]
    fn start_document_after_finish_returns_error() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        assert!(sink
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(sink.handle_event(Event::EndDocument).is_ok());

        let result = sink.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert!(result.is_err());
        let err_str = format!("{result:?}");
        assert!(err_str.contains("StartDocument received after document already finished"));
    }

    #[test]
    fn any_event_after_finish_returns_error() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        assert!(sink
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(sink.handle_event(Event::EndDocument).is_ok());

        let result = sink.handle_event(Event::Text {
            content: "orphan".to_string(),
            style: TextStyle::default(),
        });
        assert!(result.is_err());
        let err_str = format!("{result:?}");
        assert!(err_str.contains("event received after document already finished"));
    }

    #[test]
    fn nested_link_returns_error() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        assert!(sink
            .handle_event(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
            .is_ok());
        assert!(sink
            .handle_event(Event::StartParagraph {
                alignment: None,
                id: None,
            })
            .is_ok());
        assert!(sink
            .handle_event(Event::StartLink {
                href: "https://example.com".to_string(),
                id: None,
                title: None,
            })
            .is_ok());

        let result = sink.handle_event(Event::StartLink {
            href: "https://nested.com".to_string(),
            id: None,
            title: None,
        });
        assert!(result.is_err());
        let err_str = format!("{result:?}");
        assert!(err_str.contains("StartLink received while another link is already open"));
    }

    #[test]
    fn link_is_content_bearing() {
        let mut sink = StackTrackingSink::new(MockSink::new());

        assert!(!sink.has_open_content());

        send(
            &mut sink,
            Event::StartLink {
                href: "https://example.com".to_string(),
                title: None,
                id: None,
            },
        );

        assert!(sink.has_open_content());

        send(&mut sink, Event::EndLink);

        assert!(!sink.has_open_content());
    }
}
