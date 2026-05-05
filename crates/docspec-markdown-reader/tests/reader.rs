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
                | Event::EndDefinitionDetail
                | Event::EndDefinitionList
                | Event::EndDefinitionTerm
                | Event::EndFootnote
                | Event::EndLink
                | Event::EndListItem
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
                | Event::StartListItem { .. }
                | Event::StartPreformatted { .. }
                | Event::StartTable { .. }
                | Event::StartTableCell { .. }
                | Event::StartTableHeader { .. }
                | Event::StartTableRow { .. }
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

        assert!(!events
            .iter()
            .any(|e| matches!(e, Event::StartListItem { .. })));
        assert!(!events.iter().any(|e| matches!(e, Event::EndListItem)));

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
                Event::Text {
                    content,
                    code: true,
                    ..
                } if content.contains("code")
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
}
