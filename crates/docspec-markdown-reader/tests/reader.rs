//! Unit tests for `MarkdownReader`.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

mod helpers {
    #![allow(clippy::single_call_fn)]

    use docspec_core::{Event, ImageSource, ListStyleType, TableHeaderScope, TextStyle};

    /// Returns a `StartDocument` event with all optional fields set to `None`.
    /// Use in tests that do not exercise document-level metadata.
    pub fn start_document() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    /// Returns a `StartParagraph` event with no alignment or id.
    pub fn start_paragraph() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    /// Returns a `StartHeading` event with no id.
    pub fn start_heading(level: u8) -> Event {
        Event::StartHeading { level, id: None }
    }

    /// Returns a text event with exact content and style.
    pub fn text(content: &str, style: TextStyle) -> Event {
        Event::Text {
            content: content.to_string(),
            style,
        }
    }

    /// Returns a `StartPreformatted` event with no id.
    pub fn start_preformatted(syntax: Option<&str>) -> Event {
        Event::StartPreformatted {
            syntax: syntax.map(str::to_string),
            id: None,
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

    /// Returns a `StartTableHeader` event with column scope and no id.
    pub fn start_table_header() -> Event {
        Event::StartTableHeader {
            abbr: None,
            colspan: None,
            rowspan: None,
            scope: Some(TableHeaderScope::Column),
            id: None,
        }
    }

    /// Returns a `StartTableCell` event with default spans and no id.
    pub fn start_table_cell() -> Event {
        Event::StartTableCell {
            colspan: None,
            rowspan: None,
            id: None,
        }
    }

    /// Returns a `StartUnorderedListItem` event with `Disc` style at the given level and no id.
    pub fn start_unordered_list_item(level: u32) -> Event {
        Event::StartUnorderedListItem {
            style_type: ListStyleType::Disc,
            level,
            id: None,
        }
    }

    /// Returns a `StartOrderedListItem` event with `Decimal` style at the given level and no id.
    pub fn start_ordered_list_item(level: u32, start: Option<u64>) -> Event {
        Event::StartOrderedListItem {
            start,
            style_type: ListStyleType::Decimal,
            level,
            id: None,
        }
    }

    /// Returns an image event for a URI with no id.
    pub fn image_uri(uri: &str, alt: Option<&str>, title: Option<&str>, decorative: bool) -> Event {
        Event::Image {
            source: ImageSource::Uri {
                uri: uri.to_string(),
            },
            alt: alt.map(str::to_string),
            title: title.map(str::to_string),
            decorative,
            id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::helpers;
    use docspec_core::{Event, EventSource as _, ImageSource, ListStyleType, TextStyle};
    use docspec_markdown_reader::MarkdownReader;

    fn collect_events(reader: &mut MarkdownReader<'_>) -> Vec<Event> {
        let mut events = Vec::new();
        loop {
            let result = reader.next_event();
            assert!(result.is_ok(), "next_event failed: {:?}", result.err());
            match result {
                Ok(Some(event)) => events.push(event),
                Ok(None) | Err(_) => break,
            }
        }
        events
    }

    #[test]
    fn blockquote_content_extraction() {
        let markdown = "> Quoted text";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
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
            ]
        );
    }

    #[test]
    fn bold_and_italic_text() {
        let mut reader = MarkdownReader::new("***both***");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("both", TextStyle::default().bold().italic()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn bold_text() {
        let mut reader = MarkdownReader::new("**bold**");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("bold", TextStyle::default().bold()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn document_structure_preserved() {
        let markdown = "# Title\n\nParagraph text.\n\n---\n\n## Subtitle";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_heading(1),
                helpers::text("Title", TextStyle::default()),
                Event::EndHeading,
                helpers::start_paragraph(),
                helpers::text("Paragraph text.", TextStyle::default()),
                Event::EndParagraph,
                Event::ThematicBreak { id: None },
                helpers::start_heading(2),
                helpers::text("Subtitle", TextStyle::default()),
                Event::EndHeading,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn empty_document() {
        let mut reader = MarkdownReader::new("");
        let events = collect_events(&mut reader);

        assert_eq!(events, vec![helpers::start_document(), Event::EndDocument]);
    }

    #[test]
    fn hard_break() {
        let mut reader = MarkdownReader::new("Line one  \nLine two");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("Line one", TextStyle::default()),
                Event::LineBreak,
                helpers::text("Line two", TextStyle::default()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn heading_level_1() {
        let mut reader = MarkdownReader::new("# Hello");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_heading(1),
                helpers::text("Hello", TextStyle::default()),
                Event::EndHeading,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn heading_levels_2_through_6() {
        let expected_levels: [u8; 5] = [2, 3, 4, 5, 6];
        for expected in expected_levels {
            let markdown = format!("{} Heading", "#".repeat(usize::from(expected)));
            let mut reader = MarkdownReader::new(&markdown);
            let events = collect_events(&mut reader);

            assert_eq!(
                events,
                vec![
                    helpers::start_document(),
                    helpers::start_heading(expected),
                    helpers::text("Heading", TextStyle::default()),
                    Event::EndHeading,
                    Event::EndDocument,
                ]
            );
        }
    }

    #[test]
    fn image_with_alt_and_title() {
        let mut reader =
            MarkdownReader::new("![Alt text](https://example.com/img.png \"Image Title\")");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::image_uri(
                    "https://example.com/img.png",
                    Some("Alt text"),
                    Some("Image Title"),
                    false,
                ),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn image_with_alt_only() {
        let mut reader = MarkdownReader::new("![Alt text only](https://example.com/img.png)");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::image_uri(
                    "https://example.com/img.png",
                    Some("Alt text only"),
                    None,
                    false,
                ),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn image_with_no_alt() {
        let mut reader = MarkdownReader::new("![](https://example.com/img.png)");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::image_uri("https://example.com/img.png", None, None, true),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn images_fixture() {
        let markdown = include_str!("../../../tests/fixtures/markdown/images.md");
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::image_uri(
                    "https://example.com/image1.png",
                    Some("Alt text with title"),
                    Some("Image Title"),
                    false,
                ),
                Event::EndParagraph,
                helpers::start_paragraph(),
                helpers::image_uri(
                    "https://example.com/image2.png",
                    Some("Alt text only"),
                    None,
                    false,
                ),
                Event::EndParagraph,
                helpers::start_paragraph(),
                helpers::image_uri("https://example.com/image3.png", None, None, true),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn inline_code() {
        let mut reader = MarkdownReader::new("Use `code` here");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("Use ", TextStyle::default()),
                helpers::text("code", TextStyle::default().code()),
                helpers::text(" here", TextStyle::default()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn inline_code_inherits_bold() {
        let mut reader = MarkdownReader::new("**bold `code` bold**");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("bold ", TextStyle::default().bold()),
                helpers::text("code", TextStyle::default().bold().code()),
                helpers::text(" bold", TextStyle::default().bold()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn inline_code_inherits_italic() {
        let mut reader = MarkdownReader::new("*italic `code` italic*");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("italic ", TextStyle::default().italic()),
                helpers::text("code", TextStyle::default().italic().code()),
                helpers::text(" italic", TextStyle::default().italic()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn inline_code_inherits_strikethrough() {
        let mut reader = MarkdownReader::new("~~strikethrough `code` strikethrough~~");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("strikethrough ", TextStyle::default().strikethrough()),
                helpers::text("code", TextStyle::default().strikethrough().code()),
                helpers::text(" strikethrough", TextStyle::default().strikethrough()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn italic_text() {
        let mut reader = MarkdownReader::new("*italic*");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("italic", TextStyle::default().italic()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn list_content_extraction() {
        let markdown = "- Item one\n- Item two";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_unordered_list_item(0),
                Event::Text {
                    content: "Item one".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndUnorderedListItem,
                helpers::start_unordered_list_item(0),
                Event::Text {
                    content: "Item two".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndUnorderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn nested_content_fixture() {
        let markdown = include_str!("../../../tests/fixtures/markdown/nested_content.md");
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        let text_contents: Vec<&str> = events
            .iter()
            .filter_map(|event| {
                if let Event::Text { content, .. } = event {
                    Some(content.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            text_contents,
            vec!["Header", "This paragraph is inside a table cell."]
        );
    }

    #[test]
    fn nested_list_with_continuation_keeps_parent_item_open() {
        let mut reader = MarkdownReader::new(
            "- Outer A\n  - Inner nested\n\n  Continuation paragraph for Outer A\n- Outer B\n",
        );
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(events.get(2), Some(&helpers::start_paragraph()));
        assert_eq!(events.get(4), Some(&Event::EndParagraph));
        assert_eq!(events.get(5), Some(&helpers::start_unordered_list_item(1)));
        assert_eq!(events.get(7), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.get(8), Some(&helpers::start_paragraph()));
        assert_eq!(events.get(10), Some(&Event::EndParagraph));
        assert_eq!(events.get(11), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.get(12), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(events.len(), 18);
    }

    #[test]
    fn next_event_returns_none_after_end_document() {
        let mut reader = MarkdownReader::new("");
        let events = collect_events(&mut reader);

        assert_eq!(events.len(), 2);

        let first_after_end = reader.next_event();
        assert!(
            first_after_end.is_ok(),
            "next_event failed: {first_after_end:?}"
        );
        assert_eq!(first_after_end.unwrap(), None);
        let second_after_end = reader.next_event();
        assert!(
            second_after_end.is_ok(),
            "next_event failed: {second_after_end:?}"
        );
        assert_eq!(second_after_end.unwrap(), None);
    }

    #[test]
    fn paragraph() {
        let mut reader = MarkdownReader::new("Hello world");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("Hello world", TextStyle::default()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn soft_break_emits_soft_break_event() {
        let mut reader = MarkdownReader::new("Line one\nLine two");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("Line one", TextStyle::default()),
                Event::SoftBreak,
                helpers::text("Line two", TextStyle::default()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn soft_break_in_image_alt_appends_space() {
        // The image alt text flattens the soft break to a single space.
        // No SoftBreak event is emitted — the soft break is absorbed
        // into the image's alt buffer.
        let mut reader = MarkdownReader::new("![alt one\nalt two](image.png)");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::image_uri("image.png", Some("alt one alt two"), None, false),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn thematic_break() {
        let mut reader = MarkdownReader::new("Before\n\n---\n\nAfter");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("Before", TextStyle::default()),
                Event::EndParagraph,
                Event::ThematicBreak { id: None },
                helpers::start_paragraph(),
                helpers::text("After", TextStyle::default()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn code_in_image_alt_appends_to_alt_buffer() {
        let mut reader = MarkdownReader::new("![`code`](https://example.com/img.png)");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::image_uri("https://example.com/img.png", Some("code"), None, false),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn html_events_silently_ignored() {
        let mut reader = MarkdownReader::new("<div>hello</div>");
        let events = collect_events(&mut reader);

        assert_eq!(events, vec![helpers::start_document(), Event::EndDocument]);
    }

    #[test]
    fn blockquote_text_wrapped_in_auto_paragraph() {
        let markdown = "> Quoted";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                Event::StartBlockQuote { id: None },
                helpers::start_paragraph(),
                helpers::text("Quoted", TextStyle::default()),
                Event::EndParagraph,
                Event::EndBlockQuote,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn list_item_emits_start_text_end_directly() {
        let markdown = "- Item";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_unordered_list_item(0),
                helpers::text("Item", TextStyle::default()),
                Event::EndUnorderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn fenced_code_block_with_language() {
        let markdown = "```rust\nfn main() {}\n```";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_preformatted(Some("rust")),
                helpers::text("fn main() {}", TextStyle::default()),
                Event::EndPreformatted,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn fenced_code_block_without_language() {
        let markdown = "```\nsome code\n```";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_preformatted(None),
                helpers::text("some code", TextStyle::default()),
                Event::EndPreformatted,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn indented_code_block() {
        let markdown = "    indented code\n";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_preformatted(None),
                helpers::text("indented code", TextStyle::default()),
                Event::EndPreformatted,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn code_block_preserves_trailing_blank_lines() {
        // Code block with intentional trailing blank lines
        // The parser adds a newline terminator, but we should only remove that one,
        // not all trailing newlines
        let markdown = "```\ncode\n\n\n```";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        // The content should preserve the blank lines (two newlines after "code").
        // Only the parser-added terminator should be removed.
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_preformatted(None),
                helpers::text("code\n\n", TextStyle::default()),
                Event::EndPreformatted,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn code_block_special_chars_pass_through_literally() {
        // Markdown special characters inside a fenced code block must arrive
        // as literal text with no inline styling. pulldown-cmark does not parse
        // inline markdown inside code blocks, and the reader's code_block_buffer
        // branch never consults bold/italic depth.
        let markdown = "```\n*test* **bold** `code` _italic_ ~strike~\n```";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_preformatted(None),
                helpers::text(
                    "*test* **bold** `code` _italic_ ~strike~",
                    TextStyle::default(),
                ),
                Event::EndPreformatted,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn fenced_code_block_empty() {
        // An empty fenced code block emits StartPreformatted immediately
        // followed by EndPreformatted with NO Text event in between
        // (the `if !content.is_empty()` guard at src/lib.rs:234 suppresses it).
        let markdown = "```\n```";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_preformatted(None),
                Event::EndPreformatted,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn code_block_inside_list_item() {
        let markdown = "- item\n\n  ```\n  code\n  ```";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_unordered_list_item(0),
                helpers::start_paragraph(),
                helpers::text("item", TextStyle::default()),
                Event::EndParagraph,
                helpers::start_preformatted(None),
                helpers::text("code", TextStyle::default()),
                Event::EndPreformatted,
                Event::EndUnorderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn strikethrough_basic() {
        let mut reader = MarkdownReader::new("~~struck~~");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("struck", TextStyle::default().strikethrough()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn strikethrough_with_bold() {
        let mut reader = MarkdownReader::new("~~**bold struck**~~");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("bold struck", TextStyle::default().bold().strikethrough()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn strikethrough_with_italic() {
        let mut reader = MarkdownReader::new("~~*italic struck*~~");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text(
                    "italic struck",
                    TextStyle::default().italic().strikethrough()
                ),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn strikethrough_in_paragraph() {
        let markdown = "This is ~~struck~~ text in a paragraph.";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("This is ", TextStyle::default()),
                helpers::text("struck", TextStyle::default().strikethrough()),
                helpers::text(" text in a paragraph.", TextStyle::default()),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn strikethrough_with_bold_and_italic() {
        let mut reader = MarkdownReader::new("~~***bold italic struck***~~");
        let events = collect_events(&mut reader);

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text(
                    "bold italic struck",
                    TextStyle::default().bold().italic().strikethrough(),
                ),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn simple_table_emits_structured_events() {
        let markdown = "| A | B |\n|---|---|\n| C | D |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_table()));
        assert_eq!(events.get(2), Some(&helpers::start_table_row()));
        assert_eq!(events.get(3), Some(&helpers::start_table_header()));
        assert_eq!(events.get(9), Some(&Event::EndTableRow));
        assert_eq!(events.get(10), Some(&helpers::start_table_row()));
        assert_eq!(events.get(18), Some(&Event::EndTable));
    }

    #[test]
    fn table_header_cells_have_column_scope() {
        let markdown = "| H1 | H2 |\n|----|----|";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(events.get(3), Some(&helpers::start_table_header()));
        assert_eq!(events.get(6), Some(&helpers::start_table_header()));
    }

    #[test]
    fn table_body_cells_have_no_scope_field() {
        let markdown = "| H |\n|---|\n| C |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(events.get(3), Some(&helpers::start_table_header()));
        assert_eq!(events.get(7), Some(&helpers::start_table_row()));
        assert_eq!(events.get(8), Some(&helpers::start_table_cell()));
    }

    #[test]
    fn table_cell_text_emits_raw_not_wrapped() {
        let markdown = "| H |\n|---|\n| text |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(events.get(8), Some(&helpers::start_table_cell()));
        assert_eq!(
            events.get(9),
            Some(&helpers::text("text", TextStyle::default()))
        );
        assert_eq!(events.get(10), Some(&Event::EndTableCell));
    }

    #[test]
    fn table_with_inline_formatting_in_cells() {
        let markdown = "| **bold** | `code` |\n|-----------|--------|\n| *italic* | plain |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(
            events.get(4),
            Some(&helpers::text("bold", TextStyle::default().bold()))
        );
        assert_eq!(
            events.get(7),
            Some(&helpers::text("code", TextStyle::default().code()))
        );
        assert_eq!(
            events.get(12),
            Some(&helpers::text("italic", TextStyle::default().italic()))
        );
    }

    #[test]
    fn header_only_table() {
        let markdown = "| H1 | H2 |\n|----|----|";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_table()));
        assert_eq!(events.get(2), Some(&helpers::start_table_row()));
        assert_eq!(events.get(9), Some(&Event::EndTableRow));
        assert_eq!(events.get(10), Some(&Event::EndTable));
        assert_eq!(events.get(11), Some(&Event::EndDocument));
    }

    #[test]
    fn table_with_empty_cells() {
        let markdown = "| A |  |\n|---|---|\n|  | B |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(events.get(6), Some(&helpers::start_table_header()));
        assert_eq!(events.get(7), Some(&Event::EndTableHeader));
        assert_eq!(events.get(10), Some(&helpers::start_table_cell()));
        assert_eq!(events.get(11), Some(&Event::EndTableCell));
    }

    #[test]
    fn multiple_tables_in_sequence() {
        let markdown = "| A |\n|---|\n| B |\n\n| C |\n|---|\n| D |";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_table()));
        assert_eq!(events.get(12), Some(&Event::EndTable));
        assert_eq!(events.get(13), Some(&helpers::start_table()));
        assert_eq!(events.get(24), Some(&Event::EndTable));
    }

    #[test]
    fn simple_bullet_list_emits_unordered_items() {
        let mut reader = MarkdownReader::new("- a\n- b\n- c");
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(
            events.get(2),
            Some(&helpers::text("a", TextStyle::default()))
        );
        assert_eq!(events.get(3), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.get(4), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(events.get(6), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.get(7), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(events.get(9), Some(&Event::EndUnorderedListItem));
    }

    #[test]
    fn simple_numbered_list_emits_ordered_items() {
        let mut reader = MarkdownReader::new("1. a\n2. b\n3. c");
        let events = collect_events(&mut reader);

        assert_eq!(
            events.get(1),
            Some(&helpers::start_ordered_list_item(0, Some(1)))
        );
        assert_eq!(events.get(3), Some(&Event::EndOrderedListItem));
        assert_eq!(
            events.get(4),
            Some(&helpers::start_ordered_list_item(0, None))
        );
        assert_eq!(
            events.get(7),
            Some(&helpers::start_ordered_list_item(0, None))
        );
    }

    #[test]
    fn numbered_list_with_explicit_start() {
        let mut reader = MarkdownReader::new("5. a\n6. b\n7. c");
        let events = collect_events(&mut reader);

        assert_eq!(
            events.get(1),
            Some(&helpers::start_ordered_list_item(0, Some(5)))
        );
        assert_eq!(
            events.get(4),
            Some(&helpers::start_ordered_list_item(0, None))
        );
        assert_eq!(
            events.get(7),
            Some(&helpers::start_ordered_list_item(0, None))
        );
    }

    #[test]
    fn bullet_list_emits_disc_style() {
        let mut reader = MarkdownReader::new("- a");
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_unordered_list_item(0)));
    }

    #[test]
    fn ordered_list_emits_decimal_style() {
        let mut reader = MarkdownReader::new("1. a");
        let events = collect_events(&mut reader);

        assert_eq!(
            events.get(1),
            Some(&helpers::start_ordered_list_item(0, Some(1)))
        );
    }

    #[test]
    fn nested_bullet_lists_emit_increasing_level() {
        let mut reader = MarkdownReader::new("- A\n  - B\n  - C\n- D");
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(events.get(3), Some(&helpers::start_unordered_list_item(1)));
        assert_eq!(events.get(5), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.get(6), Some(&helpers::start_unordered_list_item(1)));
        assert_eq!(events.get(8), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.get(9), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.get(10), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(events.get(12), Some(&Event::EndUnorderedListItem));
    }

    #[test]
    fn mixed_nested_lists() {
        let mut reader = MarkdownReader::new("- A\n  1. B\n  2. C\n- D");
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(
            events.get(3),
            Some(&helpers::start_ordered_list_item(1, Some(1)))
        );
        assert_eq!(events.get(5), Some(&Event::EndOrderedListItem));
        assert_eq!(
            events.get(6),
            Some(&helpers::start_ordered_list_item(1, None))
        );
        assert_eq!(events.get(8), Some(&Event::EndOrderedListItem));
        assert_eq!(events.get(9), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.get(10), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(events.get(12), Some(&Event::EndUnorderedListItem));
    }

    #[test]
    fn list_item_containing_only_nested_list_no_text() {
        let mut reader = MarkdownReader::new("- \n  - Nested item\n- Sibling\n");
        let events = collect_events(&mut reader);

        // Outer item opens without any text of its own
        assert_eq!(events.get(1), Some(&helpers::start_unordered_list_item(0)));
        // Nested item opens immediately inside outer item
        assert_eq!(events.get(2), Some(&helpers::start_unordered_list_item(1)));
        assert_eq!(
            events.get(3),
            Some(&helpers::text("Nested item", TextStyle::default()))
        );
        assert_eq!(events.get(4), Some(&Event::EndUnorderedListItem));
        // Outer item closes after nested
        assert_eq!(events.get(5), Some(&Event::EndUnorderedListItem));
        // Sibling item
        assert_eq!(events.get(6), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(events.get(8), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.len(), 10);
    }

    #[test]
    fn list_items_with_inline_formatting() {
        let mut reader = MarkdownReader::new("- **bold** item\n- *italic* item");
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(
            events.get(2),
            Some(&helpers::text("bold", TextStyle::default().bold()))
        );
        assert_eq!(events.get(4), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.get(5), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(
            events.get(6),
            Some(&helpers::text("italic", TextStyle::default().italic()))
        );
        assert_eq!(events.get(8), Some(&Event::EndUnorderedListItem));
    }

    #[test]
    fn nested_list_with_hard_break_in_continuation() {
        let mut reader = MarkdownReader::new("- A\n  - B\n\n  Line one  \n  Line two\n");
        let events = collect_events(&mut reader);

        // Continuation paragraph is inside parent item
        assert_eq!(events.get(8), Some(&helpers::start_paragraph()));
        assert_eq!(
            events.get(9),
            Some(&helpers::text("Line one", TextStyle::default()))
        );
        // Hard break is inside the continuation paragraph (not orphaned)
        assert_eq!(events.get(10), Some(&Event::LineBreak));
        assert_eq!(
            events.get(11),
            Some(&helpers::text("Line two", TextStyle::default()))
        );
        assert_eq!(events.get(12), Some(&Event::EndParagraph));
        // Parent closes AFTER continuation
        assert_eq!(events.get(13), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.len(), 15);
    }

    #[test]
    fn three_levels_deeply_nested_unordered_list() {
        let mut reader = MarkdownReader::new("- A\n  - B\n    - C\n");
        let events = collect_events(&mut reader);

        assert_eq!(events.get(1), Some(&helpers::start_unordered_list_item(0)));
        assert_eq!(events.get(3), Some(&helpers::start_unordered_list_item(1)));
        assert_eq!(events.get(5), Some(&helpers::start_unordered_list_item(2)));
        assert_eq!(
            events.get(6),
            Some(&helpers::text("C", TextStyle::default()))
        );
        // All three levels close in reverse order
        assert_eq!(events.get(7), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.get(8), Some(&Event::EndUnorderedListItem));
        assert_eq!(events.get(9), Some(&Event::EndUnorderedListItem));
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

        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                Event::StartOrderedListItem {
                    start: Some(5),
                    style_type: ListStyleType::Decimal,
                    level: 0,
                    id: None,
                },
                Event::Text {
                    content: "I2".to_string(),
                    style: TextStyle::default(),
                },
                Event::StartBlockQuote { id: None },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "text".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                helpers::start_unordered_list_item(1),
                Event::Text {
                    content: "[".to_string(),
                    style: TextStyle::default(),
                },
                Event::Text {
                    content: "f".to_string(),
                    style: TextStyle::default(),
                },
                Event::Text {
                    content: "]".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndUnorderedListItem,
                Event::EndBlockQuote,
                Event::EndOrderedListItem,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn link_simple() {
        let markdown = "[text](https://example.com)";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartLink {
                    href: "https://example.com".to_string(),
                    title: None,
                    id: None,
                },
                Event::Text {
                    content: "text".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn link_with_title() {
        let markdown = r#"[text](https://example.com "a title")"#;
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartLink {
                    href: "https://example.com".to_string(),
                    title: Some("a title".to_string()),
                    id: None,
                },
                Event::Text {
                    content: "text".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn link_empty_text() {
        let markdown = "[](https://example.com)";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartLink {
                    href: "https://example.com".to_string(),
                    title: None,
                    id: None,
                },
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn link_styled_content() {
        let markdown = "[**bold** text](https://example.com)";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartLink {
                    href: "https://example.com".to_string(),
                    title: None,
                    id: None,
                },
                Event::Text {
                    content: "bold".to_string(),
                    style: TextStyle::default().bold(),
                },
                Event::Text {
                    content: " text".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn link_with_code_span() {
        let markdown = "[`code`](https://example.com)";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartLink {
                    href: "https://example.com".to_string(),
                    title: None,
                    id: None,
                },
                Event::Text {
                    content: "code".to_string(),
                    style: TextStyle::default().code(),
                },
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn autolink() {
        let markdown = "<https://example.com>";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartLink {
                    href: "https://example.com".to_string(),
                    title: None,
                    id: None,
                },
                Event::Text {
                    content: "https://example.com".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn link_in_heading() {
        let markdown = "# [text](https://example.com)";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                Event::StartHeading { level: 1, id: None },
                Event::StartLink {
                    href: "https://example.com".to_string(),
                    title: None,
                    id: None,
                },
                Event::Text {
                    content: "text".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndLink,
                Event::EndHeading,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn link_in_paragraph_with_surrounding_text() {
        let markdown = "before [link text](https://example.com) after";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "before ".to_string(),
                    style: TextStyle::default(),
                },
                Event::StartLink {
                    href: "https://example.com".to_string(),
                    title: None,
                    id: None,
                },
                Event::Text {
                    content: "link text".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndLink,
                Event::Text {
                    content: " after".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn image_in_link_extracts_image_as_sibling() {
        // pulldown-cmark event sequence: Tag::Link > Tag::Image > Text("alt") > TagEnd::Image > TagEnd::Link
        // Reader should emit: StartParagraph, StartLink, EndLink, Image, EndParagraph
        // (Image appears BEFORE EndParagraph because pulldown fires End(Image) before End(Paragraph))
        let markdown = "[![alt](img.png)](https://example.com)";
        let mut reader = MarkdownReader::new(markdown);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartLink {
                    href: "https://example.com".to_string(),
                    title: None,
                    id: None,
                },
                Event::EndLink,
                Event::Image {
                    source: ImageSource::Uri {
                        uri: "img.png".to_string(),
                    },
                    alt: Some("alt".to_string()),
                    title: None,
                    decorative: false,
                    id: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }
}
