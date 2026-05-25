//! Unit tests for `MarkdownReader`.
#![allow(clippy::expect_used, clippy::unwrap_used)]

mod helpers {
    #![allow(clippy::single_call_fn)]

    use docspec_core::{Event, ListStyleType};

    /// Returns a `StartDocument` event with all optional fields set to `None`.
    /// Use in tests that do not exercise document-level metadata.
    pub fn start_document() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    /// Returns a `StartTable` event with no id.
    pub fn start_table() -> Event {
        Event::StartTable { id: None }
    }

    /// Returns a `StartTableRow` event with no id.
    pub fn start_table_row() -> Event {
        Event::StartTableRow { id: None }
    }

    /// Returns a `StartUnorderedListItem` event with `Disc` style at the given level and no id.
    pub fn start_unordered_list_item(level: u32) -> Event {
        Event::StartUnorderedListItem {
            style_type: ListStyleType::Disc,
            level,
            id: None,
        }
    }

    /// Maps a `DocSpec` event to a short type-name string for use in
    /// document-structure assertions. Returns `"Other"` for any variant not
    /// explicitly listed (including future variants added to `docspec_core::Event`).
    pub fn event_type_name(e: &Event) -> &'static str {
        match e {
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
            | Event::EndDefinitionDetail
            | Event::EndDefinitionList
            | Event::EndDefinitionTerm
            | Event::EndFootnote
            | Event::EndLink
            | Event::EndOrderedListItem
            | Event::EndUnorderedListItem
            | Event::EndPreformatted
            | Event::EndTable
            | Event::EndTableCell
            | Event::EndTableHeader
            | Event::EndTableRow
            | Event::FootnoteRef { .. }
            | Event::Image { .. }
            | Event::LineBreak
            | Event::StartBlockQuote { .. }
            | Event::StartCaption { .. }
            | Event::StartDefinitionDetail { .. }
            | Event::StartDefinitionList { .. }
            | Event::StartDefinitionTerm { .. }
            | Event::StartFootnote { .. }
            | Event::StartHeading { .. }
            | Event::StartLink { .. }
            | Event::StartOrderedListItem { .. }
            | Event::StartUnorderedListItem { .. }
            | Event::StartPreformatted { .. }
            | Event::StartTable { .. }
            | Event::StartTableCell { .. }
            | Event::StartTableHeader { .. }
            | Event::StartTableRow { .. }
            | Event::ThematicBreak { .. }
            | _ => "Other",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::helpers;
    use docspec_core::{
        Event, EventSource as _, ImageSource, ListStyleType, TableHeaderScope, TextStyle,
    };
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
                    style: TextStyle {
                        bold: true,
                        italic: true,
                        ..
                    },
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, style: TextStyle { bold: true, italic: true, .. }, .. }) if content == "both"
        ));
    }

    #[test]
    fn bold_text() {
        let mut reader = MarkdownReader::new("**bold**");
        let events = collect_events(&mut reader);

        let text_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    style: TextStyle { bold: true, .. },
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, style: TextStyle { bold: true, italic: false, .. }, .. }) if content == "bold"
        ));
    }

    #[test]
    fn document_structure_preserved() {
        let markdown = "# Title\n\nParagraph text.\n\n---\n\n## Subtitle";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let event_types: Vec<&str> = events.iter().map(helpers::event_type_name).collect();

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
        assert_eq!(events.first(), Some(&helpers::start_document()));
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

        assert_eq!(events.first(), Some(&helpers::start_document()));
        assert!(matches!(
            events.get(1),
            Some(Event::StartHeading { level: 1, .. })
        ));
        assert!(matches!(
            events.get(2),
            Some(Event::Text { content, style: TextStyle { bold: false, italic: false, .. }, .. }) if content == "Hello"
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

        let code_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    style: TextStyle { code: true, .. },
                    ..
                }
            )
        });
        assert!(matches!(
            code_event,
            Some(Event::Text { content, style: TextStyle { code: true, .. }, .. }) if content == "code"
        ));
    }

    #[test]
    fn inline_code_inherits_bold() {
        let mut reader = MarkdownReader::new("**bold `code` bold**");
        let events = collect_events(&mut reader);

        let code_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    style: TextStyle { code: true, .. },
                    ..
                }
            )
        });
        assert!(
            matches!(
                code_event,
                Some(Event::Text {
                    content,
                    style: TextStyle { code: true, bold: true, .. },
                    ..
                }) if content == "code"
            ),
            "Code inside bold should inherit bold formatting"
        );
    }

    #[test]
    fn inline_code_inherits_italic() {
        let mut reader = MarkdownReader::new("*italic `code` italic*");
        let events = collect_events(&mut reader);

        let code_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    style: TextStyle { code: true, .. },
                    ..
                }
            )
        });
        assert!(
            matches!(
                code_event,
                Some(Event::Text {
                    content,
                    style: TextStyle { code: true, italic: true, .. },
                    ..
                }) if content == "code"
            ),
            "Code inside italic should inherit italic formatting"
        );
    }

    #[test]
    fn inline_code_inherits_strikethrough() {
        let mut reader = MarkdownReader::new("~~strikethrough `code` strikethrough~~");
        let events = collect_events(&mut reader);

        let code_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    style: TextStyle { code: true, .. },
                    ..
                }
            )
        });
        assert!(
            matches!(
                code_event,
                Some(Event::Text {
                    content,
                    style: TextStyle { code: true, strikethrough: true, .. },
                    ..
                }) if content == "code"
            ),
            "Code inside strikethrough should inherit strikethrough formatting"
        );
    }

    #[test]
    fn italic_text() {
        let mut reader = MarkdownReader::new("*italic*");
        let events = collect_events(&mut reader);

        let text_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text {
                    style: TextStyle { italic: true, .. },
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, style: TextStyle { bold: false, italic: true, .. }, .. }) if content == "italic"
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
    fn nested_list_with_continuation_keeps_parent_item_open() {
        let mut reader = MarkdownReader::new(
            "- Outer A\n  - Inner nested\n\n  Continuation paragraph for Outer A\n- Outer B\n",
        );
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(1),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(events.get(2), Some(Event::StartParagraph { .. })));
        assert!(matches!(events.get(4), Some(Event::EndParagraph)));
        assert!(matches!(
            events.get(5),
            Some(Event::StartUnorderedListItem { level: 1, .. })
        ));
        assert!(matches!(events.get(7), Some(Event::EndUnorderedListItem)));
        assert!(matches!(events.get(8), Some(Event::StartParagraph { .. })));
        assert!(matches!(events.get(10), Some(Event::EndParagraph)));
        assert!(matches!(events.get(11), Some(Event::EndUnorderedListItem)));
        assert!(matches!(
            events.get(12),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert_eq!(events.len(), 18);
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
    fn list_item_emits_start_text_end_directly() {
        let markdown = "- Item";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(1),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(
            events.get(2),
            Some(Event::Text { content, .. }) if content == "Item"
        ));
        assert!(matches!(events.get(3), Some(Event::EndUnorderedListItem)));
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
        // Code block with intentional trailing blank lines
        // The parser adds a newline terminator, but we should only remove that one,
        // not all trailing newlines
        let markdown = "```\ncode\n\n\n```";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        // Find the text event inside the code block
        let text_event = events.iter().find(|e| {
            matches!(
                e,
                Event::Text { content, style: TextStyle { code: true, .. }, .. } if content.contains("code")
            )
        });

        // The content should preserve the blank lines (two newlines after "code")
        // Only the parser-added terminator should be removed
        // Use exact equality to catch regressions
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
                    style: TextStyle {
                        strikethrough: true,
                        ..
                    },
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, style: TextStyle { strikethrough: true, .. }, .. }) if content == "struck"
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
                    style: TextStyle {
                        bold: true,
                        strikethrough: true,
                        ..
                    },
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, style: TextStyle { bold: true, strikethrough: true, .. }, .. }) if content == "bold struck"
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
                    style: TextStyle {
                        italic: true,
                        strikethrough: true,
                        ..
                    },
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, style: TextStyle { italic: true, strikethrough: true, .. }, .. }) if content == "italic struck"
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
                Event::Text { content, style: TextStyle { strikethrough: true, .. }, .. } if content == "struck"
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
                    style: TextStyle {
                        bold: true,
                        italic: true,
                        strikethrough: true,
                        ..
                    },
                    ..
                }
            )
        });
        assert!(matches!(
            text_event,
            Some(Event::Text { content, style: TextStyle { bold: true, italic: true, strikethrough: true, .. }, .. }) if content == "bold italic struck"
        ));
    }

    #[test]
    fn simple_table_emits_structured_events() {
        let markdown = "| A | B |\n|---|---|\n| C | D |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_table()));
        assert_eq!(events.get(2), Some(&helpers::start_table_row()));
        assert!(matches!(
            events.get(3),
            Some(Event::StartTableHeader { .. })
        ));
        assert!(matches!(events.get(9), Some(Event::EndTableRow)));
        assert_eq!(events.get(10), Some(&helpers::start_table_row()));
        assert!(matches!(events.get(18), Some(Event::EndTable)));
    }

    #[test]
    fn table_header_cells_have_column_scope() {
        let markdown = "| H1 | H2 |\n|----|----|";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(3),
            Some(Event::StartTableHeader {
                scope: Some(TableHeaderScope::Column),
                ..
            })
        ));
        assert!(matches!(
            events.get(6),
            Some(Event::StartTableHeader {
                scope: Some(TableHeaderScope::Column),
                ..
            })
        ));
    }

    #[test]
    fn table_body_cells_have_no_scope_field() {
        let markdown = "| H |\n|---|\n| C |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(3),
            Some(Event::StartTableHeader { .. })
        ));
        assert_eq!(events.get(7), Some(&helpers::start_table_row()));
        assert!(matches!(events.get(8), Some(Event::StartTableCell { .. })));
    }

    #[test]
    fn table_cell_text_emits_raw_not_wrapped() {
        let markdown = "| H |\n|---|\n| text |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(matches!(events.get(8), Some(Event::StartTableCell { .. })));
        assert!(matches!(events.get(9), Some(Event::Text { .. })));
        assert!(matches!(events.get(10), Some(Event::EndTableCell)));
    }

    #[test]
    fn table_with_inline_formatting_in_cells() {
        let markdown = "| **bold** | `code` |\n|-----------|--------|\n| *italic* | plain |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(4),
            Some(Event::Text {
                content,
                style: TextStyle { bold: true, .. },
                ..
            }) if content == "bold"
        ));
        assert!(matches!(
            events.get(7),
            Some(Event::Text {
                content,
                style: TextStyle { code: true, .. },
                ..
            }) if content == "code"
        ));
        assert!(matches!(
            events.get(12),
            Some(Event::Text {
                content,
                style: TextStyle { italic: true, .. },
                ..
            }) if content == "italic"
        ));
    }

    #[test]
    fn header_only_table() {
        let markdown = "| H1 | H2 |\n|----|----|";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(matches!(events.get(1), Some(Event::StartTable { .. })));
        assert!(matches!(events.get(2), Some(Event::StartTableRow { .. })));
        assert!(matches!(events.get(9), Some(Event::EndTableRow)));
        assert!(matches!(events.get(10), Some(Event::EndTable)));
        assert!(matches!(events.get(11), Some(Event::EndDocument)));
    }

    #[test]
    fn table_with_empty_cells() {
        let markdown = "| A |  |\n|---|---|\n|  | B |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(6),
            Some(Event::StartTableHeader { .. })
        ));
        assert!(matches!(events.get(7), Some(Event::EndTableHeader)));
        assert!(matches!(events.get(10), Some(Event::StartTableCell { .. })));
        assert!(matches!(events.get(11), Some(Event::EndTableCell)));
    }

    #[test]
    fn multiple_tables_in_sequence() {
        let markdown = "| A |\n|---|\n| B |\n\n| C |\n|---|\n| D |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert!(matches!(events.get(1), Some(Event::StartTable { .. })));
        assert!(matches!(events.get(12), Some(Event::EndTable)));
        assert!(matches!(events.get(13), Some(Event::StartTable { .. })));
        assert!(matches!(events.get(24), Some(Event::EndTable)));
    }

    #[test]
    fn simple_bullet_list_emits_unordered_items() {
        let mut reader = MarkdownReader::new("- a\n- b\n- c");
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_unordered_list_item(0)));
        assert!(matches!(events.get(2), Some(Event::Text { .. })));
        assert!(matches!(events.get(3), Some(Event::EndUnorderedListItem)));
        assert!(matches!(
            events.get(4),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(events.get(6), Some(Event::EndUnorderedListItem)));
        assert!(matches!(
            events.get(7),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(events.get(9), Some(Event::EndUnorderedListItem)));
    }

    #[test]
    fn simple_numbered_list_emits_ordered_items() {
        let mut reader = MarkdownReader::new("1. a\n2. b\n3. c");
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(1),
            Some(Event::StartOrderedListItem {
                start: Some(1),
                level: 0,
                ..
            })
        ));
        assert!(matches!(events.get(3), Some(Event::EndOrderedListItem)));
        assert!(matches!(
            events.get(4),
            Some(Event::StartOrderedListItem {
                start: None,
                level: 0,
                ..
            })
        ));
        assert!(matches!(
            events.get(7),
            Some(Event::StartOrderedListItem {
                start: None,
                level: 0,
                ..
            })
        ));
    }

    #[test]
    fn numbered_list_with_explicit_start() {
        let mut reader = MarkdownReader::new("5. a\n6. b\n7. c");
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(1),
            Some(Event::StartOrderedListItem {
                start: Some(5),
                level: 0,
                ..
            })
        ));
        assert!(matches!(
            events.get(4),
            Some(Event::StartOrderedListItem {
                start: None,
                level: 0,
                ..
            })
        ));
        assert!(matches!(
            events.get(7),
            Some(Event::StartOrderedListItem {
                start: None,
                level: 0,
                ..
            })
        ));
    }

    #[test]
    fn bullet_list_emits_disc_style() {
        let mut reader = MarkdownReader::new("- a");
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(1),
            Some(Event::StartUnorderedListItem {
                style_type: ListStyleType::Disc,
                ..
            })
        ));
    }

    #[test]
    fn ordered_list_emits_decimal_style() {
        let mut reader = MarkdownReader::new("1. a");
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(1),
            Some(Event::StartOrderedListItem {
                style_type: ListStyleType::Decimal,
                ..
            })
        ));
    }

    #[test]
    fn nested_bullet_lists_emit_increasing_level() {
        let mut reader = MarkdownReader::new("- A\n  - B\n  - C\n- D");
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(1),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(
            events.get(3),
            Some(Event::StartUnorderedListItem { level: 1, .. })
        ));
        assert!(matches!(events.get(5), Some(Event::EndUnorderedListItem)));
        assert!(matches!(
            events.get(6),
            Some(Event::StartUnorderedListItem { level: 1, .. })
        ));
        assert!(matches!(events.get(8), Some(Event::EndUnorderedListItem)));
        assert!(matches!(events.get(9), Some(Event::EndUnorderedListItem)));
        assert!(matches!(
            events.get(10),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(events.get(12), Some(Event::EndUnorderedListItem)));
    }

    #[test]
    fn mixed_nested_lists() {
        let mut reader = MarkdownReader::new("- A\n  1. B\n  2. C\n- D");
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(1),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(
            events.get(3),
            Some(Event::StartOrderedListItem {
                start: Some(1),
                level: 1,
                ..
            })
        ));
        assert!(matches!(events.get(5), Some(Event::EndOrderedListItem)));
        assert!(matches!(
            events.get(6),
            Some(Event::StartOrderedListItem {
                start: None,
                level: 1,
                ..
            })
        ));
        assert!(matches!(events.get(8), Some(Event::EndOrderedListItem)));
        assert!(matches!(events.get(9), Some(Event::EndUnorderedListItem)));
        assert!(matches!(
            events.get(10),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(events.get(12), Some(Event::EndUnorderedListItem)));
    }

    #[test]
    fn list_item_containing_only_nested_list_no_text() {
        let mut reader = MarkdownReader::new("- \n  - Nested item\n- Sibling\n");
        let events = collect_events(&mut reader);

        // Outer item opens without any text of its own
        assert!(matches!(
            events.get(1),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        // Nested item opens immediately inside outer item
        assert!(matches!(
            events.get(2),
            Some(Event::StartUnorderedListItem { level: 1, .. })
        ));
        assert!(matches!(
            events.get(3),
            Some(Event::Text { content, .. }) if content == "Nested item"
        ));
        assert!(matches!(events.get(4), Some(Event::EndUnorderedListItem)));
        // Outer item closes after nested
        assert!(matches!(events.get(5), Some(Event::EndUnorderedListItem)));
        // Sibling item
        assert!(matches!(
            events.get(6),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(events.get(8), Some(Event::EndUnorderedListItem)));
        assert_eq!(events.len(), 10);
    }

    #[test]
    fn list_items_with_inline_formatting() {
        let mut reader = MarkdownReader::new("- **bold** item\n- *italic* item");
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(1),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(
            events.get(2),
            Some(Event::Text {
                style: TextStyle { bold: true, .. },
                ..
            })
        ));
        assert!(matches!(events.get(4), Some(Event::EndUnorderedListItem)));
        assert!(matches!(
            events.get(5),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(
            events.get(6),
            Some(Event::Text {
                style: TextStyle { italic: true, .. },
                ..
            })
        ));
        assert!(matches!(events.get(8), Some(Event::EndUnorderedListItem)));
    }

    #[test]
    fn nested_list_with_hard_break_in_continuation() {
        let mut reader = MarkdownReader::new("- A\n  - B\n\n  Line one  \n  Line two\n");
        let events = collect_events(&mut reader);

        // Continuation paragraph is inside parent item
        assert!(matches!(events.get(8), Some(Event::StartParagraph { .. })));
        assert!(matches!(
            events.get(9),
            Some(Event::Text { content, .. }) if content == "Line one"
        ));
        // Hard break is inside the continuation paragraph (not orphaned)
        assert!(matches!(events.get(10), Some(Event::LineBreak)));
        assert!(matches!(
            events.get(11),
            Some(Event::Text { content, .. }) if content == "Line two"
        ));
        assert!(matches!(events.get(12), Some(Event::EndParagraph)));
        // Parent closes AFTER continuation
        assert!(matches!(events.get(13), Some(Event::EndUnorderedListItem)));
        assert_eq!(events.len(), 15);
    }

    #[test]
    fn three_levels_deeply_nested_unordered_list() {
        let mut reader = MarkdownReader::new("- A\n  - B\n    - C\n");
        let events = collect_events(&mut reader);

        assert!(matches!(
            events.get(1),
            Some(Event::StartUnorderedListItem { level: 0, .. })
        ));
        assert!(matches!(
            events.get(3),
            Some(Event::StartUnorderedListItem { level: 1, .. })
        ));
        assert!(matches!(
            events.get(5),
            Some(Event::StartUnorderedListItem { level: 2, .. })
        ));
        assert!(matches!(
            events.get(6),
            Some(Event::Text { content, .. }) if content == "C"
        ));
        // All three levels close in reverse order
        assert!(matches!(events.get(7), Some(Event::EndUnorderedListItem)));
        assert!(matches!(events.get(8), Some(Event::EndUnorderedListItem)));
        assert!(matches!(events.get(9), Some(Event::EndUnorderedListItem)));
        assert_eq!(events.len(), 11);
    }

    #[test]
    fn list_inside_blockquote_inside_list_item() {
        // Regression: list nested inside a blockquote inside a list item must produce
        // a well-formed event stream. The outer item's EndOrderedListItem must be
        // emitted AFTER the inner EndBlockQuote, not before.
        let markdown = "5) I2\n   > text\n   > - [f]\n";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let end_blockquote_idx = events
            .iter()
            .position(|e| matches!(e, Event::EndBlockQuote))
            .expect("EndBlockQuote must be present");
        let end_outer_item_idx = events
            .iter()
            .rposition(|e| matches!(e, Event::EndOrderedListItem))
            .expect("EndOrderedListItem must be present");

        assert!(
            end_blockquote_idx < end_outer_item_idx,
            "EndBlockQuote (at {end_blockquote_idx}) must precede outer EndOrderedListItem \
             (at {end_outer_item_idx}); events: {events:#?}"
        );
    }
}
