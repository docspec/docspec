//! Stack module tests.

#![allow(clippy::too_many_lines)]

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
        assert!(
            result.is_ok(),
            "fixture event should be accepted: {result:?}"
        );
    }

    fn assert_invalid_sequence(result: &Result<()>, expected: &str, found: &str, message: &str) {
        assert!(matches!(
            result,
            Err(Error::InvalidSequence {
                expected: actual_expected,
                found: actual_found,
                message: actual_message,
            }) if actual_expected == expected && actual_found == found && actual_message == message
        ));
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
        assert_invalid_sequence(
            &result,
            "Table",
            "Blockquote",
            "End event for Blockquote does not match any open block",
        );
    }

    #[test]
    fn end_without_start_returns_error() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        let result = sink.handle_event(Event::EndParagraph);
        assert_invalid_sequence(
            &result,
            "open block",
            "Paragraph",
            "received End event with empty stack",
        );
    }

    #[test]
    fn end_document_without_start_returns_error() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        let result = sink.handle_event(Event::EndDocument);
        assert_invalid_sequence(
            &result,
            "open Document",
            "EndDocument",
            "EndDocument received without StartDocument",
        );
    }

    #[test]
    fn start_document_after_finish_returns_error() {
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
        send(&mut sink, Event::EndDocument);

        let result = sink.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        assert_invalid_sequence(
            &result,
            "end of stream",
            "StartDocument",
            "StartDocument received after document already finished",
        );
    }

    #[test]
    fn any_event_after_finish_returns_error() {
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
        send(&mut sink, Event::EndDocument);

        let result = sink.handle_event(Event::Text {
            content: "orphan".to_string(),
        });
        assert_invalid_sequence(
            &result,
            "end of stream",
            "Text { content: \"orphan\" }",
            "event received after document already finished",
        );
    }

    #[test]
    fn nested_link_returns_error() {
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartLink {
                href: "https://example.com".to_string(),
                id: None,
                title: None,
            },
        );

        let result = sink.handle_event(Event::StartLink {
            href: "https://nested.com".to_string(),
            id: None,
            title: None,
        });
        assert_invalid_sequence(
            &result,
            "no nested links",
            "StartLink",
            "StartLink received while another link is already open",
        );
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

    #[test]
    fn is_inside_text_style_fresh_instance() {
        let mock = MockSink::new();
        let sink = StackTrackingSink::new(mock);
        assert!(!sink.is_inside_text_style(TextStyleKind::Bold));
    }

    #[test]
    fn start_text_style_inside_paragraph() {
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
            ]
        );
    }

    #[test]
    fn start_text_style_inside_heading() {
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
        send(&mut sink, Event::StartHeading { id: None, level: 1 });
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { id: None, level: 1 },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
            ]
        );
    }

    #[test]
    fn start_text_style_at_root_auto_paragraph() {
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
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
            ]
        );
    }

    #[test]
    fn start_text_style_auto_paragraph_style_on_stack() {
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
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );

        assert!(sink.is_inside_text_style(TextStyleKind::Bold));
        assert!(sink.is_inside(BlockKind::Paragraph));
    }

    #[test]
    fn end_text_style_happy_path() {
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(&mut sink, Event::EndTextStyle);

        assert!(!sink.is_inside_text_style(TextStyleKind::Bold));
        let events = sink.into_inner().events;
        assert_eq!(
            events.last(),
            Some(&Event::EndTextStyle),
            "last forwarded event should be EndTextStyle"
        );
    }

    #[test]
    fn end_text_style_empty_stack_errors() {
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );

        let result = sink.handle_event(Event::EndTextStyle);
        assert_invalid_sequence(
            &result,
            "open text style",
            "EndTextStyle",
            "EndTextStyle received with empty style stack",
        );
    }

    #[test]
    fn end_text_style_pops_lifo() {
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Italic,
            },
        );
        send(&mut sink, Event::EndTextStyle);

        assert!(!sink.is_inside_text_style(TextStyleKind::Italic));
        assert!(sink.is_inside_text_style(TextStyleKind::Bold));
    }

    #[test]
    fn styles_drain_before_end_paragraph() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(&mut sink, Event::EndParagraph);

        assert!(!sink.is_inside_text_style(TextStyleKind::Bold));
        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::EndTextStyle,
                Event::EndParagraph,
            ]
        );
    }

    #[test]
    fn styles_drain_lifo_before_block_end() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Italic,
            },
        );
        send(&mut sink, Event::EndParagraph);

        assert!(!sink.is_inside_text_style(TextStyleKind::Bold));
        assert!(!sink.is_inside_text_style(TextStyleKind::Italic));
        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Italic,
                },
                Event::EndTextStyle,
                Event::EndTextStyle,
                Event::EndParagraph,
            ]
        );
    }

    #[test]
    fn styles_drained_on_end_document() {
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(&mut sink, Event::EndDocument);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn styles_drained_on_end_document_multiple() {
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
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Italic,
            },
        );
        send(&mut sink, Event::EndDocument);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Italic,
                },
                Event::EndTextStyle,
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn styles_drain_before_end_heading() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);
        let mark = TextStyleKind::Mark(Color::Rgb { r: 255, g: 0, b: 0 });

        send(&mut sink, Event::StartHeading { id: None, level: 1 });
        send(&mut sink, Event::StartTextStyle { kind: mark });
        send(&mut sink, Event::EndHeading);

        assert!(!sink.is_inside_text_style(mark));
        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartHeading { id: None, level: 1 },
                Event::StartTextStyle { kind: mark },
                Event::EndTextStyle,
                Event::EndHeading,
            ]
        );
    }

    #[test]
    fn same_kind_nesting() {
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(&mut sink, Event::EndTextStyle);
        send(&mut sink, Event::EndTextStyle);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::EndTextStyle,
                Event::EndTextStyle,
            ]
        );
    }

    #[test]
    fn mark_color_nesting() {
        let blue = TextStyleKind::Mark(Color::Rgb { r: 0, g: 0, b: 255 });
        let red = TextStyleKind::Mark(Color::Rgb { r: 255, g: 0, b: 0 });
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(&mut sink, Event::StartTextStyle { kind: blue });
        send(&mut sink, Event::StartTextStyle { kind: red });
        send(&mut sink, Event::EndTextStyle);
        send(&mut sink, Event::EndTextStyle);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle { kind: blue },
                Event::StartTextStyle { kind: red },
                Event::EndTextStyle,
                Event::EndTextStyle,
            ]
        );
    }

    #[test]
    fn empty_span() {
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(&mut sink, Event::EndTextStyle);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::EndTextStyle,
            ]
        );
    }

    #[test]
    fn style_spans_block_boundary() {
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
        send(&mut sink, Event::StartBlockQuote { id: None });
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(&mut sink, Event::EndBlockQuote);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
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
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndBlockQuote,
            ]
        );
    }

    #[test]
    fn style_open_at_end_document() {
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
        send(&mut sink, Event::StartBlockQuote { id: None });
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Italic,
            },
        );
        send(&mut sink, Event::EndDocument);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
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
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Italic,
                },
                Event::EndTextStyle,
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndBlockQuote,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn end_style_empty_stack() {
        let mut sink = StackTrackingSink::new(MockSink::new());
        let result = sink.handle_event(Event::EndTextStyle);
        assert_invalid_sequence(
            &result,
            "open text style",
            "EndTextStyle",
            "EndTextStyle received with empty style stack",
        );

        let mock = MockSink::new();
        let mut sink_inner = StackTrackingSink::new(mock);
        send(
            &mut sink_inner,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(
            &mut sink_inner,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        let result_inner = sink_inner.handle_event(Event::EndTextStyle);
        assert_invalid_sequence(
            &result_inner,
            "open text style",
            "EndTextStyle",
            "EndTextStyle received with empty style stack",
        );

        let mock_b = MockSink::new();
        let mut sink_b = StackTrackingSink::new(mock_b);
        send(
            &mut sink_b,
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
        );
        send(
            &mut sink_b,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        let result_b = sink_b.handle_event(Event::EndTextStyle);
        assert_invalid_sequence(
            &result_b,
            "open text style",
            "EndTextStyle",
            "EndTextStyle received with empty style stack",
        );
    }

    #[test]
    fn start_style_outside_content() {
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(&mut sink, Event::EndParagraph);
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
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
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
            ]
        );
    }

    #[test]
    fn code_inside_preformatted() {
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
        send(
            &mut sink,
            Event::StartPreformatted {
                id: None,
                syntax: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Code,
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "fn main() {}".to_string(),
            },
        );
        send(&mut sink, Event::EndTextStyle);
        send(&mut sink, Event::EndPreformatted);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartPreformatted {
                    id: None,
                    syntax: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Code,
                },
                Event::Text {
                    content: "fn main() {}".to_string(),
                },
                Event::EndTextStyle,
                Event::EndPreformatted,
            ]
        );
    }

    #[test]
    fn end_document_drains_all_remaining_block_kinds() {
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
        send(&mut sink, Event::StartCaption { id: None });
        send(&mut sink, Event::StartDefinitionList { id: None });
        send(&mut sink, Event::StartDefinitionDetail { id: None });
        send(&mut sink, Event::StartDefinitionTerm { id: None });
        send(&mut sink, Event::StartFootnote { id: 1 });
        send(
            &mut sink,
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(1),
                style_type: docspec_core::ListStyleType::Decimal,
            },
        );
        send(
            &mut sink,
            Event::StartTableHeader {
                abbr: None,
                colspan: None,
                id: None,
                rowspan: None,
                scope: None,
            },
        );
        send(
            &mut sink,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
        );
        send(&mut sink, Event::EndDocument);

        let events = sink.into_inner().events;
        assert!(events.contains(&Event::EndCaption));
        assert!(events.contains(&Event::EndDefinitionList));
        assert!(events.contains(&Event::EndDefinitionDetail));
        assert!(events.contains(&Event::EndDefinitionTerm));
        assert!(events.contains(&Event::EndFootnote));
        assert!(events.contains(&Event::EndOrderedListItem));
        assert!(events.contains(&Event::EndTableHeader));
        assert!(events.contains(&Event::EndUnorderedListItem));
        assert!(events.contains(&Event::EndDocument));
    }

    #[test]
    fn block_kind_for_start_all_variants() {
        assert_eq!(
            block_kind_for_start(&Event::StartBlockQuote { id: None }),
            Some(BlockKind::Blockquote)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartCaption { id: None }),
            Some(BlockKind::Caption)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartDefinitionDetail { id: None }),
            Some(BlockKind::DefinitionDetail)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartDefinitionList { id: None }),
            Some(BlockKind::DefinitionList)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartDefinitionTerm { id: None }),
            Some(BlockKind::DefinitionTerm)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            }),
            Some(BlockKind::Document)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartFootnote { id: 1 }),
            Some(BlockKind::Footnote)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartHeading { id: None, level: 1 }),
            Some(BlockKind::Heading)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartLink {
                href: "https://example.com".to_string(),
                id: None,
                title: None,
            }),
            Some(BlockKind::Link)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(1),
                style_type: docspec_core::ListStyleType::Decimal,
            }),
            Some(BlockKind::OrderedListItem)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartParagraph {
                alignment: None,
                id: None,
            }),
            Some(BlockKind::Paragraph)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartPreformatted {
                id: None,
                syntax: None,
            }),
            Some(BlockKind::Preformatted)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartTable { id: None }),
            Some(BlockKind::Table)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartTableCell {
                colspan: None,
                id: None,
                rowspan: None,
            }),
            Some(BlockKind::TableCell)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartTableHeader {
                abbr: None,
                colspan: None,
                id: None,
                rowspan: None,
                scope: None,
            }),
            Some(BlockKind::TableHeader)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartTableRow { id: None }),
            Some(BlockKind::TableRow)
        );
        assert_eq!(
            block_kind_for_start(&Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            }),
            Some(BlockKind::UnorderedListItem)
        );
        assert_eq!(
            block_kind_for_start(&Event::Text {
                content: "hello".to_string(),
            }),
            None
        );
    }

    #[test]
    fn block_kind_for_end_all_variants() {
        assert_eq!(
            block_kind_for_end(&Event::EndBlockQuote),
            Some(BlockKind::Blockquote)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndCaption),
            Some(BlockKind::Caption)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndDefinitionDetail),
            Some(BlockKind::DefinitionDetail)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndDefinitionList),
            Some(BlockKind::DefinitionList)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndDefinitionTerm),
            Some(BlockKind::DefinitionTerm)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndDocument),
            Some(BlockKind::Document)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndFootnote),
            Some(BlockKind::Footnote)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndHeading),
            Some(BlockKind::Heading)
        );
        assert_eq!(block_kind_for_end(&Event::EndLink), Some(BlockKind::Link));
        assert_eq!(
            block_kind_for_end(&Event::EndOrderedListItem),
            Some(BlockKind::OrderedListItem)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndParagraph),
            Some(BlockKind::Paragraph)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndPreformatted),
            Some(BlockKind::Preformatted)
        );
        assert_eq!(block_kind_for_end(&Event::EndTable), Some(BlockKind::Table));
        assert_eq!(
            block_kind_for_end(&Event::EndTableCell),
            Some(BlockKind::TableCell)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndTableHeader),
            Some(BlockKind::TableHeader)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndTableRow),
            Some(BlockKind::TableRow)
        );
        assert_eq!(
            block_kind_for_end(&Event::EndUnorderedListItem),
            Some(BlockKind::UnorderedListItem)
        );
        assert_eq!(
            block_kind_for_end(&Event::Text {
                content: "hello".to_string(),
            }),
            None
        );
    }

    #[test]
    fn caption_block_roundtrip() {
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
        send(&mut sink, Event::StartCaption { id: None });
        send(
            &mut sink,
            Event::Text {
                content: "caption text".to_string(),
            },
        );
        send(&mut sink, Event::EndCaption);
        send(&mut sink, Event::EndDocument);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartCaption { id: None },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "caption text".to_string(),
                },
                Event::EndParagraph,
                Event::EndCaption,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn definition_list_roundtrip() {
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
        send(&mut sink, Event::StartDefinitionList { id: None });
        send(&mut sink, Event::StartDefinitionTerm { id: None });
        send(
            &mut sink,
            Event::Text {
                content: "term".to_string(),
            },
        );
        send(&mut sink, Event::EndDefinitionTerm);
        send(&mut sink, Event::StartDefinitionDetail { id: None });
        send(
            &mut sink,
            Event::Text {
                content: "detail".to_string(),
            },
        );
        send(&mut sink, Event::EndDefinitionDetail);
        send(&mut sink, Event::EndDefinitionList);
        send(&mut sink, Event::EndDocument);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartDefinitionList { id: None },
                Event::StartDefinitionTerm { id: None },
                Event::Text {
                    content: "term".to_string(),
                },
                Event::EndDefinitionTerm,
                Event::StartDefinitionDetail { id: None },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "detail".to_string(),
                },
                Event::EndParagraph,
                Event::EndDefinitionDetail,
                Event::EndDefinitionList,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn definition_term_is_content_bearing() {
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
        send(&mut sink, Event::StartDefinitionList { id: None });
        send(&mut sink, Event::StartDefinitionTerm { id: None });
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(&mut sink, Event::EndDefinitionTerm);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartDefinitionList { id: None },
                Event::StartDefinitionTerm { id: None },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::EndTextStyle,
                Event::EndDefinitionTerm,
            ]
        );
    }

    #[test]
    fn footnote_block_roundtrip() {
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
        send(&mut sink, Event::StartFootnote { id: 1 });
        send(
            &mut sink,
            Event::Text {
                content: "footnote content".to_string(),
            },
        );
        send(&mut sink, Event::EndFootnote);
        send(&mut sink, Event::EndDocument);

        assert!(sink.into_inner().events.contains(&Event::EndFootnote));
    }

    #[test]
    fn ordered_list_item_roundtrip() {
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
        send(
            &mut sink,
            Event::StartOrderedListItem {
                id: None,
                level: 0,
                start: Some(1),
                style_type: docspec_core::ListStyleType::Decimal,
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "item".to_string(),
            },
        );
        send(&mut sink, Event::EndOrderedListItem);
        send(&mut sink, Event::EndDocument);

        assert!(sink
            .into_inner()
            .events
            .contains(&Event::EndOrderedListItem));
    }

    #[test]
    fn unordered_list_item_roundtrip() {
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
        send(
            &mut sink,
            Event::StartUnorderedListItem {
                id: None,
                level: 0,
                style_type: docspec_core::ListStyleType::Disc,
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "bullet".to_string(),
            },
        );
        send(&mut sink, Event::EndUnorderedListItem);
        send(&mut sink, Event::EndDocument);

        assert!(sink
            .into_inner()
            .events
            .contains(&Event::EndUnorderedListItem));
    }

    #[test]
    fn table_header_roundtrip() {
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
            Event::StartTableHeader {
                abbr: None,
                colspan: None,
                id: None,
                rowspan: None,
                scope: None,
            },
        );
        send(
            &mut sink,
            Event::Text {
                content: "header".to_string(),
            },
        );
        send(&mut sink, Event::EndTableHeader);
        send(&mut sink, Event::EndTableRow);
        send(&mut sink, Event::EndTable);
        send(&mut sink, Event::EndDocument);

        assert!(sink.into_inner().events.contains(&Event::EndTableHeader));
    }

    #[test]
    fn leaf_events_forwarded_through_stack() {
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(&mut sink, Event::LineBreak);
        send(&mut sink, Event::SoftBreak);
        send(&mut sink, Event::FootnoteRef { id: 1 });
        send(&mut sink, Event::EndParagraph);
        send(&mut sink, Event::EndDocument);

        let events = sink.into_inner().events;
        assert!(events.contains(&Event::LineBreak));
        assert!(events.contains(&Event::SoftBreak));
        assert!(events.contains(&Event::FootnoteRef { id: 1 }));
    }

    #[test]
    fn thematic_break_without_open_paragraph() {
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
        send(&mut sink, Event::ThematicBreak { id: None });
        send(&mut sink, Event::EndDocument);

        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::ThematicBreak { id: None },
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn auto_close_through_remaining_block_kinds() {
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
        send(&mut sink, Event::StartDefinitionList { id: None });
        send(&mut sink, Event::StartDefinitionDetail { id: None });
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(&mut sink, Event::EndDefinitionList);

        let events = sink.into_inner().events;
        assert!(events.contains(&Event::EndParagraph));
        assert!(events.contains(&Event::EndDefinitionDetail));
        assert!(events.contains(&Event::EndDefinitionList));
    }

    #[test]
    fn sink_finish_called() {
        let mock = MockSink::new();
        let sink = StackTrackingSink::new(mock);
        let result = sink.finish();
        assert!(result.is_ok());
    }

    #[test]
    fn mismatched_end_with_open_style_does_not_drain_style() {
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
        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );

        // Attempt to end a mismatched block (EndBlockQuote instead of EndParagraph)
        let result = sink.handle_event(Event::EndBlockQuote);

        // Should return InvalidSequence error
        assert_invalid_sequence(
            &result,
            "Paragraph",
            "Blockquote",
            "End event for Blockquote does not match any open block",
        );

        // Style stack should NOT be drained: Bold should still be open
        assert!(sink.is_inside_text_style(TextStyleKind::Bold));

        // No synthetic EndTextStyle should have been forwarded
        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
            ]
        );
    }

    #[test]
    fn thematic_break_drains_styles_before_end_paragraph() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(&mut sink, Event::ThematicBreak { id: None });

        assert!(!sink.is_inside_text_style(TextStyleKind::Bold));
        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::ThematicBreak { id: None },
            ]
        );
    }

    #[test]
    fn new_block_start_drains_styles_before_implicit_end_paragraph() {
        let mock = MockSink::new();
        let mut sink = StackTrackingSink::new(mock);

        send(
            &mut sink,
            Event::StartParagraph {
                alignment: None,
                id: None,
            },
        );
        send(
            &mut sink,
            Event::StartTextStyle {
                kind: TextStyleKind::Bold,
            },
        );
        send(&mut sink, Event::StartHeading { id: None, level: 1 });

        assert!(!sink.is_inside_text_style(TextStyleKind::Bold));
        let events = sink.into_inner().events;
        assert_eq!(
            events,
            vec![
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                },
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::StartHeading { id: None, level: 1 },
            ]
        );
    }
}
