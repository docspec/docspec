//! Unit tests for `MarkdownReader`.

#[cfg(test)]
mod tests {
    use docspec_core::{Event, EventSource as _, ImageSource};
    use docspec_markdown_reader::MarkdownReader;

    fn collect_events(reader: &mut MarkdownReader<'_>) -> Vec<Event> {
        let mut events = Vec::new();
        loop {
            let result = reader.next_event();
            assert!(result.is_ok(), "next_event failed: {:?}", result.err());
            match result.unwrap_or_default() {
                Some(event) => events.push(event),
                None => break,
            }
        }
        events
    }

    #[test]
    fn blockquote_content_extraction() {
        let markdown = "> Quoted text";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartBlockQuote { .. })));
        assert!(events.iter().any(|e| matches!(e, Event::EndBlockQuote)));

        let has_quoted = events
            .iter()
            .any(|e| matches!(e, Event::Text { content, .. } if content.contains("Quoted")));
        assert!(has_quoted);
    }

    #[test]
    fn bold_and_italic_text() {
        let mut reader = MarkdownReader::new("***both***");
        let events = collect_events(&mut reader);

        let text_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    bold: true,
                    italic: true,
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, bold: true, italic: true, .. }) if content == "both"
        ));
    }

    #[test]
    fn bold_text() {
        let mut reader = MarkdownReader::new("**bold**");
        let events = collect_events(&mut reader);

        let text_event = events
            .iter()
            .find(|e| matches!(e, Event::Text { bold: true, .. }));
        assert!(matches!(
            text_event,
            Some(Event::Text { content, bold: true, italic: false, .. }) if content == "bold"
        ));
    }

    #[test]
    fn document_structure_preserved() {
        let markdown = "# Title\n\nParagraph text.\n\n---\n\n## Subtitle";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let event_types: Vec<&str> = events
            .iter()
            .map(|e| match e {
                Event::StartDocument { .. } => "StartDocument",
                Event::EndDocument => "EndDocument",
                Event::StartHeading { level: 1, .. } => "StartHeading(1)",
                Event::StartHeading { level: 2, .. } => "StartHeading(2)",
                Event::EndHeading => "EndHeading",
                Event::StartParagraph { .. } => "StartParagraph",
                Event::EndParagraph => "EndParagraph",
                Event::Text { .. } => "Text",
                Event::EndBlockQuote
                | Event::EndCaption
                | Event::EndCheckListItem
                | Event::EndDefinitionDetail
                | Event::EndDefinitionList
                | Event::EndDefinitionTerm
                | Event::EndFootnote
                | Event::EndLink
                | Event::EndOrderedListItem
                | Event::EndPreformatted
                | Event::EndTable
                | Event::EndTableCell
                | Event::EndTableHeader
                | Event::EndTableRow
                | Event::EndUnorderedListItem
                | Event::FootnoteRef { .. }
                | Event::Image { .. }
                | Event::LineBreak
                | Event::StartBlockQuote { .. }
                | Event::StartCaption { .. }
                | Event::StartCheckListItem { .. }
                | Event::StartDefinitionDetail { .. }
                | Event::StartDefinitionList { .. }
                | Event::StartDefinitionTerm { .. }
                | Event::StartFootnote { .. }
                | Event::StartHeading { .. }
                | Event::StartLink { .. }
                | Event::StartOrderedListItem { .. }
                | Event::StartPreformatted { .. }
                | Event::StartTable { .. }
                | Event::StartTableCell { .. }
                | Event::StartTableHeader { .. }
                | Event::StartTableRow { .. }
                | Event::StartUnorderedListItem { .. }
                | Event::ThematicBreak { .. }
                | _ => "Other",
            })
            .collect();

        assert_eq!(
            event_types,
            vec![
                "StartDocument",
                "StartHeading(1)",
                "Text",
                "EndHeading",
                "StartParagraph",
                "Text",
                "EndParagraph",
                "Other",
                "StartHeading(2)",
                "Text",
                "EndHeading",
                "EndDocument"
            ]
        );
    }

    #[test]
    fn empty_document() {
        let mut reader = MarkdownReader::new("");
        let events = collect_events(&mut reader);

        assert_eq!(events.len(), 2);
        assert!(matches!(
            events.first(),
            Some(Event::StartDocument {
                id: None,
                language: None,
                metadata: None
            })
        ));
        assert!(matches!(events.get(1), Some(Event::EndDocument)));
    }

    #[test]
    fn hard_break() {
        let mut reader = MarkdownReader::new("Line one  \nLine two");
        let events = collect_events(&mut reader);

        assert!(events.iter().any(|e| matches!(e, Event::LineBreak)));
    }

    #[test]
    fn heading_level_1() {
        let mut reader = MarkdownReader::new("# Hello");
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.first(),
            Some(Event::StartDocument {
                id: None,
                language: None,
                metadata: None
            })
        ));
        assert!(matches!(
            events.get(1),
            Some(Event::StartHeading { level: 1, .. })
        ));
        assert!(matches!(
            events.get(2),
            Some(Event::Text { content, bold: false, italic: false, .. }) if content == "Hello"
        ));
        assert!(matches!(events.get(3), Some(Event::EndHeading)));
        assert!(matches!(events.get(4), Some(Event::EndDocument)));
    }

    #[test]
    fn heading_levels_2_through_6() {
        let expected_levels: [u8; 5] = [2, 3, 4, 5, 6];
        for expected in expected_levels {
            let markdown = format!("{} Heading", "#".repeat(usize::from(expected)));
            let mut reader = MarkdownReader::new(&markdown);
            let events = collect_events(&mut reader);

            assert!(matches!(
                events.get(1),
                Some(Event::StartHeading { level, .. }) if *level == expected
            ));
        }
    }

    #[test]
    fn image_with_alt_and_title() {
        let mut reader =
            MarkdownReader::new("![Alt text](https://example.com/img.png \"Image Title\")");
        let events = collect_events(&mut reader);

        let image_event = events.iter().find(|e| matches!(e, Event::Image { .. }));
        assert!(matches!(
            image_event,
            Some(Event::Image {
                source: ImageSource::Uri { uri },
                alt: Some(alt),
                title: Some(title),
                decorative: false,
                ..
            }) if uri == "https://example.com/img.png"
                && alt == "Alt text"
                && title == "Image Title"
        ));
    }

    #[test]
    fn image_with_alt_only() {
        let mut reader = MarkdownReader::new("![Alt text only](https://example.com/img.png)");
        let events = collect_events(&mut reader);

        let image_event = events.iter().find(|e| matches!(e, Event::Image { .. }));
        assert!(matches!(
            image_event,
            Some(Event::Image { alt: Some(alt), title: None, .. }) if alt == "Alt text only"
        ));
    }

    #[test]
    fn image_with_no_alt() {
        let mut reader = MarkdownReader::new("![](https://example.com/img.png)");
        let events = collect_events(&mut reader);

        let image_event = events.iter().find(|e| matches!(e, Event::Image { .. }));
        assert!(matches!(
            image_event,
            Some(Event::Image {
                alt: None,
                decorative: true,
                ..
            })
        ));
    }

    #[test]
    fn images_fixture() {
        let markdown = include_str!("../../../tests/fixtures/markdown/images.md");
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let images: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::Image { .. }))
            .collect();
        assert_eq!(images.len(), 3);

        assert!(matches!(
            images.first(),
            Some(Event::Image { alt: Some(alt), title: Some(title), .. })
            if alt == "Alt text with title" && title == "Image Title"
        ));
        assert!(matches!(
            images.get(1),
            Some(Event::Image { alt: Some(alt), title: None, .. }) if alt == "Alt text only"
        ));
        assert!(matches!(
            images.get(2),
            Some(Event::Image {
                alt: None,
                decorative: true,
                ..
            })
        ));
    }

    #[test]
    fn inline_code() {
        let mut reader = MarkdownReader::new("Use `code` here");
        let events = collect_events(&mut reader);

        let code_event = events
            .iter()
            .find(|e| matches!(e, Event::Text { code: true, .. }));
        assert!(matches!(
            code_event,
            Some(Event::Text { content, code: true, .. }) if content == "code"
        ));
    }

    #[test]
    fn italic_text() {
        let mut reader = MarkdownReader::new("*italic*");
        let events = collect_events(&mut reader);

        let text_event = events
            .iter()
            .find(|e| matches!(e, Event::Text { italic: true, .. }));
        assert!(matches!(
            text_event,
            Some(Event::Text { content, bold: false, italic: true, .. }) if content == "italic"
        ));
    }

    #[test]
    fn list_content_extraction() {
        let markdown = "- Item one\n- Item two";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartUnorderedListItem { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::EndUnorderedListItem)));

        let has_item_one = events
            .iter()
            .any(|e| matches!(e, Event::Text { content, .. } if content.contains("Item one")));
        let has_item_two = events
            .iter()
            .any(|e| matches!(e, Event::Text { content, .. } if content.contains("Item two")));

        assert!(has_item_one);
        assert!(has_item_two);
    }

    #[test]
    fn nested_content_fixture() {
        let markdown = include_str!("../../../tests/fixtures/markdown/nested_content.md");
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let has_paragraph_text = events.iter().any(
                |e| matches!(e, Event::Text { content, .. } if content.contains("paragraph") && content.contains("table")),
            );
        assert!(has_paragraph_text);
    }

    #[test]
    fn next_event_returns_none_after_end_document() {
        let mut reader = MarkdownReader::new("");
        let events = collect_events(&mut reader);

        assert_eq!(events.len(), 2);

        assert!(matches!(reader.next_event(), Ok(None)));
        assert!(matches!(reader.next_event(), Ok(None)));
    }

    #[test]
    fn paragraph() {
        let mut reader = MarkdownReader::new("Hello world");
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(1),
            Some(Event::StartParagraph {
                alignment: None,
                ..
            })
        ));
        assert!(matches!(
            events.get(2),
            Some(Event::Text { content, .. }) if content == "Hello world"
        ));
        assert!(matches!(events.get(3), Some(Event::EndParagraph)));
    }

    #[test]
    fn soft_break_becomes_space_text() {
        let mut reader = MarkdownReader::new("Line one\nLine two");
        let events = collect_events(&mut reader);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Text { content, .. } if content == " ")),
            "SoftBreak should emit a space Text event"
        );
    }

    #[test]
    fn table_content_extraction() {
        let markdown = "| Header |\n|--------|\n| Cell content |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(!events.iter().any(|e| matches!(e, Event::StartTable { .. })));
        assert!(!events.iter().any(|e| matches!(e, Event::EndTable)));

        let has_header = events
            .iter()
            .any(|e| matches!(e, Event::Text { content, .. } if content.contains("Header")));
        let has_cell = events
            .iter()
            .any(|e| matches!(e, Event::Text { content, .. } if content.contains("Cell content")));

        assert!(has_header, "Table header text should be extracted");
        assert!(has_cell, "Table cell text should be extracted");
    }

    #[test]
    fn thematic_break() {
        let mut reader = MarkdownReader::new("Before\n\n---\n\nAfter");
        let events = collect_events(&mut reader);

        assert!(events
            .iter()
            .any(|e| matches!(e, Event::ThematicBreak { .. })));
    }

    #[test]
    fn code_in_image_alt_appends_to_alt_buffer() {
        let mut reader = MarkdownReader::new("![`code`](https://example.com/img.png)");
        let events = collect_events(&mut reader);

        let image_event = events.iter().find(|e| matches!(e, Event::Image { .. }));
        assert!(matches!(
            image_event,
            Some(Event::Image { alt: Some(alt), .. }) if alt == "code"
        ));
    }

    #[test]
    fn html_events_silently_ignored() {
        let mut reader = MarkdownReader::new("<div>hello</div>");
        let events = collect_events(&mut reader);

        assert!(matches!(events.first(), Some(Event::StartDocument { .. })));
        assert!(matches!(events.last(), Some(Event::EndDocument)));
    }

    #[test]
    fn soft_break_in_image_alt_appends_space() {
        let mut reader = MarkdownReader::new("![alt\ntext](https://example.com/img.png)");
        let events = collect_events(&mut reader);

        let image_event = events.iter().find(|e| matches!(e, Event::Image { .. }));
        assert!(matches!(
            image_event,
            Some(Event::Image { alt: Some(alt), .. }) if alt == "alt text"
        ));
    }

    #[test]
    fn table_cell_text_wrapped_in_auto_paragraph() {
        let markdown = "| Header |\n|--------|\n| Cell |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let paragraph_count = events
            .iter()
            .filter(|e| matches!(e, Event::StartParagraph { .. }))
            .count();
        assert_eq!(
            paragraph_count, 2,
            "each cell text should auto-open a paragraph"
        );

        let end_paragraph_count = events
            .iter()
            .filter(|e| matches!(e, Event::EndParagraph))
            .count();
        assert_eq!(end_paragraph_count, 2);
    }

    #[test]
    fn blockquote_text_wrapped_in_auto_paragraph() {
        let markdown = "> Quoted";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartParagraph { .. })));
        assert!(events.iter().any(|e| matches!(e, Event::EndParagraph)));
    }

    #[test]
    fn list_item_text_wrapped_in_auto_paragraph() {
        let markdown = "- Item";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartParagraph { .. })));
        assert!(events.iter().any(|e| matches!(e, Event::EndParagraph)));
    }

    #[test]
    fn table_cell_with_inline_code_auto_paragraph() {
        let markdown = "| `code` |\n|--------|\n| data |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let code_event = events
            .iter()
            .find(|e| matches!(e, Event::Text { code: true, .. }));
        assert!(matches!(
            code_event,
            Some(Event::Text { content, code: true, .. }) if content == "code"
        ));
    }

    #[test]
    fn table_cell_with_soft_break_auto_paragraph() {
        let markdown = "| a\nb |\n|-----|\n| c |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Text { content, .. } if content == " ")),
            "SoftBreak in table cell should emit space Text"
        );
    }

    #[test]
    fn fenced_code_block_with_language() {
        let markdown = "```rust\nfn main() {}\n```";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(events.iter().any(|e| matches!(
            e,
            Event::StartPreformatted {
                syntax: Some(lang),
                ..
            } if lang == "rust"
        )));
        assert!(events.iter().any(|e| matches!(e, Event::EndPreformatted)));

        let has_content = events
            .iter()
            .any(|e| matches!(e, Event::Text { content, .. } if content.contains("fn main")));
        assert!(has_content);
    }

    #[test]
    fn fenced_code_block_without_language() {
        let markdown = "```\nsome code\n```";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartPreformatted { syntax: None, .. })));
        assert!(events.iter().any(|e| matches!(e, Event::EndPreformatted)));
    }

    #[test]
    fn indented_code_block() {
        let markdown = "    indented code\n";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartPreformatted { syntax: None, .. })));
        assert!(events.iter().any(|e| matches!(e, Event::EndPreformatted)));

        let has_content = events
            .iter()
            .any(|e| matches!(e, Event::Text { content, .. } if content.contains("indented")));
        assert!(has_content);
    }

    #[test]
    fn code_block_preserves_trailing_blank_lines() {
        let markdown = "```\ncode\n\n\n```";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let text_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    content,
                    code: true,
                    ..
                } if content.contains("code")
            )
        });

        assert!(matches!(
            text_event,
            Some(Event::Text { content, .. }) if content == "code\n\n"
        ));
    }

    #[test]
    fn strikethrough_basic() {
        let mut reader = MarkdownReader::new("~~struck~~");
        let events = collect_events(&mut reader);

        let text_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    strikethrough: true,
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, strikethrough: true, .. }) if content == "struck"
        ));
    }

    #[test]
    fn strikethrough_with_bold() {
        let mut reader = MarkdownReader::new("~~**bold struck**~~");
        let events = collect_events(&mut reader);

        let text_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    bold: true,
                    strikethrough: true,
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, bold: true, strikethrough: true, .. }) if content == "bold struck"
        ));
    }

    #[test]
    fn strikethrough_with_italic() {
        let mut reader = MarkdownReader::new("~~*italic struck*~~");
        let events = collect_events(&mut reader);

        let text_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    italic: true,
                    strikethrough: true,
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, italic: true, strikethrough: true, .. }) if content == "italic struck"
        ));
    }

    #[test]
    fn strikethrough_in_paragraph() {
        let markdown = "This is ~~struck~~ text in a paragraph.";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartParagraph { .. })));

        let struck_text = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    content,
                    strikethrough: true,
                    ..
                } if content == "struck"
            )
        });
        assert!(
            struck_text.is_some(),
            "Should find strikethrough text in paragraph"
        );
    }

    #[test]
    fn strikethrough_with_bold_and_italic() {
        let mut reader = MarkdownReader::new("~~***bold italic struck***~~");
        let events = collect_events(&mut reader);

        let text_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    bold: true,
                    italic: true,
                    strikethrough: true,
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, bold: true, italic: true, strikethrough: true, .. }) if content == "bold italic struck"
        ));
    }

    #[test]
    fn ordered_list_emits_events() {
        let markdown = "1. First\n2. Second\n3. Third";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let ordered_starts: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::StartOrderedListItem { .. }))
            .collect();
        assert_eq!(ordered_starts.len(), 3, "Expected 3 ordered list items");

        assert!(matches!(
            ordered_starts.first(),
            Some(Event::StartOrderedListItem {
                level: 1,
                start: Some(1),
                ..
            })
        ));

        assert!(matches!(
            ordered_starts.get(1),
            Some(Event::StartOrderedListItem {
                level: 1,
                start: None,
                ..
            })
        ));
    }

    #[test]
    fn unordered_list_emits_events() {
        let markdown = "- Item one\n- Item two";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let unordered_starts: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::StartUnorderedListItem { .. }))
            .collect();
        assert_eq!(unordered_starts.len(), 2, "Expected 2 unordered list items");
    }

    #[test]
    fn task_list_emits_events() {
        let markdown = "- [x] Done\n- [ ] Todo";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let check_starts: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::StartCheckListItem { .. }))
            .collect();
        assert_eq!(check_starts.len(), 2, "Expected 2 check list items");

        assert!(matches!(
            check_starts.first(),
            Some(Event::StartCheckListItem { checked: true, .. })
        ));

        assert!(matches!(
            check_starts.get(1),
            Some(Event::StartCheckListItem { checked: false, .. })
        ));
    }

    #[test]
    fn nested_list_levels() {
        let markdown = "- Level 1\n  - Level 2\n    - Level 3";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let items: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let Event::StartUnorderedListItem { level, .. } = e {
                    Some(*level)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(items, vec![1, 2, 3], "Expected levels 1, 2, 3");
    }

    #[test]
    fn mixed_list_types() {
        let markdown = "1. Ordered\n- Unordered";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartOrderedListItem { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartUnorderedListItem { .. })));
    }

    #[test]
    fn ordered_list_with_custom_start_number() {
        let markdown = "5. First\n6. Second";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let ordered_starts: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::StartOrderedListItem { .. }))
            .collect();
        assert_eq!(ordered_starts.len(), 2, "Expected 2 ordered list items");

        assert!(matches!(
            ordered_starts.first(),
            Some(Event::StartOrderedListItem {
                level: 1,
                start: Some(5),
                ..
            })
        ));
    }

    #[test]
    fn nested_task_list() {
        let markdown = "- [x] Parent\n  - [ ] Child";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let check_starts: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::StartCheckListItem { .. }))
            .collect();
        assert_eq!(check_starts.len(), 2, "Expected 2 check list items");

        assert!(matches!(
            check_starts.first(),
            Some(Event::StartCheckListItem { level: 1, .. })
        ));
        assert!(matches!(
            check_starts.get(1),
            Some(Event::StartCheckListItem { level: 2, .. })
        ));
    }

    #[test]
    fn list_items_have_end_events() {
        let markdown = "- Item one\n- Item two";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let end_unordered: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::EndUnorderedListItem))
            .collect();
        assert_eq!(
            end_unordered.len(),
            2,
            "Expected 2 EndUnorderedListItem events"
        );
    }

    #[test]
    fn ordered_list_items_have_end_events() {
        let markdown = "1. First\n2. Second";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let end_ordered: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::EndOrderedListItem))
            .collect();
        assert_eq!(end_ordered.len(), 2, "Expected 2 EndOrderedListItem events");
    }

    #[test]
    fn task_list_items_have_end_events() {
        let markdown = "- [x] Done\n- [ ] Todo";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let end_check: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::EndCheckListItem))
            .collect();
        assert_eq!(end_check.len(), 2, "Expected 2 EndCheckListItem events");
    }

    #[test]
    fn empty_list_item() {
        let markdown = "- \n- Item";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let unordered_starts: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::StartUnorderedListItem { .. }))
            .collect();
        assert_eq!(
            unordered_starts.len(),
            2,
            "Expected 2 list items including empty"
        );
    }

    /// Regression test: ordered parent with unordered child must emit correct End events.
    /// Bug: single `item_state` field gets overwritten by child, causing parent to emit wrong End.
    #[test]
    fn nested_mixed_list_types_correct_end_events() {
        let markdown = "1. Parent\n   - Child";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let list_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::StartOrderedListItem { .. }
                        | Event::EndOrderedListItem
                        | Event::StartUnorderedListItem { .. }
                        | Event::EndUnorderedListItem
                )
            })
            .collect();

        assert_eq!(list_events.len(), 4, "Expected 4 list events");
        assert!(matches!(
            list_events.first(),
            Some(Event::StartOrderedListItem { level: 1, .. })
        ));
        assert!(matches!(
            list_events.get(1),
            Some(Event::StartUnorderedListItem { level: 2, .. })
        ));
        assert!(matches!(
            list_events.get(2),
            Some(Event::EndUnorderedListItem)
        ));
        assert!(matches!(
            list_events.get(3),
            Some(Event::EndOrderedListItem)
        ));
    }

    /// Regression test: deeply nested lists maintain correct Start/End pairing.
    /// Bug: `item_state` stack corruption causes wrong End events at depth > 2.
    #[test]
    fn deeply_nested_lists_correct_pairing() {
        let markdown = "- L1\n  - L2\n    - L3";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let list_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::StartUnorderedListItem { .. } | Event::EndUnorderedListItem
                )
            })
            .collect();

        assert_eq!(
            list_events.len(),
            6,
            "Expected 6 list events (3 starts, 3 ends)"
        );
        assert!(matches!(
            list_events.first(),
            Some(Event::StartUnorderedListItem { level: 1, .. })
        ));
        assert!(matches!(
            list_events.get(1),
            Some(Event::StartUnorderedListItem { level: 2, .. })
        ));
        assert!(matches!(
            list_events.get(2),
            Some(Event::StartUnorderedListItem { level: 3, .. })
        ));
        assert!(matches!(
            list_events.get(3),
            Some(Event::EndUnorderedListItem)
        ));
        assert!(matches!(
            list_events.get(4),
            Some(Event::EndUnorderedListItem)
        ));
        assert!(matches!(
            list_events.get(5),
            Some(Event::EndUnorderedListItem)
        ));
    }

    /// Regression test: task list parent with regular child.
    /// Bug: task marker could be consumed by wrong item.
    #[test]
    fn nested_task_list_marker_applies_to_correct_item() {
        let markdown = "- [x] Parent\n  - Child";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let check_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::StartCheckListItem { .. } | Event::EndCheckListItem
                )
            })
            .collect();

        let unordered_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::StartUnorderedListItem { .. } | Event::EndUnorderedListItem
                )
            })
            .collect();

        assert_eq!(check_events.len(), 2, "Parent should be check list item");
        assert_eq!(
            unordered_events.len(),
            2,
            "Child should be unordered list item"
        );

        assert!(matches!(
            check_events.first(),
            Some(Event::StartCheckListItem {
                level: 1,
                checked: true,
                ..
            })
        ));
        assert!(matches!(
            unordered_events.first(),
            Some(Event::StartUnorderedListItem { level: 2, .. })
        ));
    }

    /// Regression test: image inside list item should emit proper Start/End events.
    /// Bug: image as first content in list item may not trigger `StartUnorderedListItem` emission.
    #[test]
    fn image_inside_list_item_emits_correct_events() {
        let markdown = "- ![alt text](http://example.com/img.png)";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let list_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::StartUnorderedListItem { .. } | Event::EndUnorderedListItem
                )
            })
            .collect();

        let image_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::Image { .. }))
            .collect();

        assert_eq!(
            list_events.len(),
            2,
            "Expected Start and End list item events"
        );
        assert_eq!(image_events.len(), 1, "Expected one image event");

        assert!(matches!(
            list_events.first(),
            Some(Event::StartUnorderedListItem { level: 1, .. })
        ));
        assert!(matches!(
            list_events.get(1),
            Some(Event::EndUnorderedListItem)
        ));

        assert!(matches!(
            image_events.first(),
            Some(Event::Image { alt: Some(alt), .. }) if alt == "alt text"
        ));
    }

    /// Regression test: list item with only an image (no text) should still emit events.
    #[test]
    fn list_item_with_image_only_no_text() {
        let markdown = "1. ![](http://example.com/img.png)";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let list_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::StartOrderedListItem { .. } | Event::EndOrderedListItem
                )
            })
            .collect();

        assert_eq!(
            list_events.len(),
            2,
            "Expected Start and End ordered list item events"
        );
        assert!(matches!(
            list_events.first(),
            Some(Event::StartOrderedListItem {
                level: 1,
                start: Some(1),
                ..
            })
        ));
    }

    /// Regression test: nested list with image in parent.
    #[test]
    fn nested_list_with_image_in_parent() {
        let markdown = "- ![img](http://example.com/img.png)\n  - Child text";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let list_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::StartUnorderedListItem { .. } | Event::EndUnorderedListItem
                )
            })
            .collect();

        assert_eq!(list_events.len(), 4, "Expected 2 starts and 2 ends");
        assert!(matches!(
            list_events.first(),
            Some(Event::StartUnorderedListItem { level: 1, .. })
        ));
        assert!(matches!(
            list_events.get(1),
            Some(Event::StartUnorderedListItem { level: 2, .. })
        ));
    }
}
