//! Integration tests for `PandocNativeWriter`.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use docspec_core::Event;
use docspec_pandoc_native_writer::PandocNativeWriter;
use std::io::{self, Write};

/// A writer that always fails on write.
struct FailingWriter;

impl Write for FailingWriter {
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("write failed"))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::panic_in_result_fn, clippy::unwrap_in_result)]

    use super::*;
    use docspec_core::EventSink as _;

    fn run(events: impl IntoIterator<Item = Event>) -> String {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = PandocNativeWriter::new(&mut buf);
        for event in events {
            writer.handle_event(event).unwrap();
        }
        writer.finish().unwrap();
        String::from_utf8(buf).unwrap()
    }

    fn try_run(events: impl IntoIterator<Item = Event>) -> docspec_core::Result<String> {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = PandocNativeWriter::new(&mut buf);
        for event in events {
            writer.handle_event(event)?;
        }
        writer.finish()?;
        Ok(String::from_utf8(buf).unwrap())
    }

    fn start_doc() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    fn start_para() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    fn text(s: &str) -> Event {
        Event::Text {
            content: s.to_string(),
        }
    }

    fn start_heading(level: u8, id: Option<&str>) -> Event {
        Event::StartHeading {
            level,
            id: id.map(String::from),
        }
    }

    fn start_style(kind: docspec_core::TextStyleKind) -> Event {
        Event::StartTextStyle { kind, id: None }
    }

    #[test]
    fn empty_document_emits_empty_list() {
        assert_eq!(run([start_doc(), Event::EndDocument]), "[]");
    }

    #[test]
    fn single_empty_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para []]"
        );
    }

    #[test]
    fn single_text_in_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("hi"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"hi\"]]"
        );
    }

    #[test]
    fn two_texts_in_one_paragraph_emit_two_strs() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                text("b"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"a\",Str \"b\"]]"
        );
    }

    #[test]
    fn two_paragraphs_separated_by_comma() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                Event::EndParagraph,
                start_para(),
                text("b"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"a\"],Para [Str \"b\"]]"
        );
    }

    #[test]
    fn text_styles_ignored() {
        let bold_text = Event::Text {
            content: "x".to_string(),
        };
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                bold_text,
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"x\"]]"
        );
    }

    #[test]
    fn text_outside_paragraph_ignored() {
        assert_eq!(run([start_doc(), text("orphan"), Event::EndDocument]), "[]");
    }

    #[test]
    fn text_before_startdocument_ignored() {
        assert_eq!(run([text("a"), start_doc(), Event::EndDocument]), "[]");
    }

    #[test]
    fn events_after_enddocument_ignored() {
        assert_eq!(
            run([
                start_doc(),
                Event::EndDocument,
                start_para(),
                text("x"),
                Event::EndParagraph
            ]),
            "[]"
        );
    }

    #[test]
    fn double_start_document_is_noop() {
        assert_eq!(run([start_doc(), start_doc(), Event::EndDocument]), "[]");
    }

    #[test]
    fn orphan_end_paragraph_is_noop() {
        assert_eq!(
            run([start_doc(), Event::EndParagraph, Event::EndDocument]),
            "[]"
        );
    }

    #[test]
    fn orphan_end_document_is_noop() {
        assert_eq!(run([Event::EndDocument]), "");
    }

    #[test]
    fn ignored_events_do_not_break_structure() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::StartBlockQuote { id: None },
                Event::EndBlockQuote,
                Event::EndDocument
            ]),
            "[Para [Str \"x\"]]"
        );
    }

    #[test]
    fn thematic_break_standalone() {
        assert_eq!(
            run([
                start_doc(),
                Event::ThematicBreak { id: None },
                Event::EndDocument
            ]),
            "[HorizontalRule]"
        );
    }

    #[test]
    fn thematic_break_between_paragraphs() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                Event::EndParagraph,
                Event::ThematicBreak { id: None },
                start_para(),
                text("b"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"a\"],HorizontalRule,Para [Str \"b\"]]"
        );
    }

    #[test]
    fn thematic_break_with_id_ignored() {
        assert_eq!(
            run([
                start_doc(),
                Event::ThematicBreak {
                    id: Some("hr1".to_string()),
                },
                Event::EndDocument
            ]),
            "[HorizontalRule]"
        );
    }

    #[test]
    fn thematic_break_outside_document_ignored() {
        assert_eq!(
            run([
                Event::ThematicBreak { id: None },
                start_doc(),
                Event::EndDocument
            ]),
            "[]"
        );
    }

    #[test]
    fn thematic_break_inside_paragraph_ignored() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                Event::ThematicBreak { id: None },
                text("b"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"a\",Str \"b\"]]"
        );
    }

    #[test]
    fn line_break_inside_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                Event::LineBreak,
                text("b"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"a\",LineBreak,Str \"b\"]]"
        );
    }

    #[test]
    fn line_break_at_start_of_paragraph_no_leading_comma() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                Event::LineBreak,
                text("a"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [LineBreak,Str \"a\"]]"
        );
    }

    #[test]
    fn line_break_outside_paragraph_ignored() {
        assert_eq!(
            run([start_doc(), Event::LineBreak, Event::EndDocument]),
            "[]"
        );
    }

    #[test]
    fn soft_break_inside_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                Event::SoftBreak,
                text("b"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"a\",SoftBreak,Str \"b\"]]"
        );
    }

    #[test]
    fn soft_break_at_start_of_paragraph_no_leading_comma() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                Event::SoftBreak,
                text("a"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [SoftBreak,Str \"a\"]]"
        );
    }

    #[test]
    fn soft_break_outside_paragraph_ignored() {
        assert_eq!(
            run([start_doc(), Event::SoftBreak, Event::EndDocument]),
            "[]"
        );
    }

    #[test]
    fn mixed_breaks_and_text_in_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                Event::SoftBreak,
                text("b"),
                Event::LineBreak,
                text("c"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"a\",SoftBreak,Str \"b\",LineBreak,Str \"c\"]]"
        );
    }

    #[test]
    fn escaped_text_quotes_and_backslashes() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a\"b\\c"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"a\\\"b\\\\c\"]]"
        );
    }

    #[test]
    fn escaped_text_newline_and_tab() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a\nb\tc"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"a\\nb\\tc\"]]"
        );
    }

    #[test]
    fn escaped_text_nul_byte() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("\u{0}"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"\\NUL\"]]"
        );
    }

    #[test]
    fn escaped_text_unicode_raw_utf8() {
        let result = run([
            start_doc(),
            start_para(),
            text("it\u{2019}s"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        let expected_bytes: Vec<u8> = b"[Para [Str \"it\xe2\x80\x99s\"]]".to_vec();
        assert_eq!(result.as_bytes(), expected_bytes.as_slice());
    }

    #[test]
    fn gap_escape_so_followed_by_h() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("\u{0e}H"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"\\SO\\&H\"]]"
        );
    }

    #[test]
    fn gap_escape_decimal_followed_by_digit() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("\u{1}5"),
                Event::EndParagraph,
                Event::EndDocument
            ]),
            "[Para [Str \"\\1\\&5\"]]"
        );
    }

    #[test]
    fn non_ascii_emits_raw_utf8_no_gap_escape() {
        let result = run([
            start_doc(),
            start_para(),
            text("\u{a0}5"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        let expected_bytes: Vec<u8> = b"[Para [Str \"\xc2\xa05\"]]".to_vec();
        assert_eq!(result.as_bytes(), expected_bytes.as_slice());
    }

    #[test]
    fn finish_without_enddocument_best_effort_closes() {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = PandocNativeWriter::new(&mut buf);
        writer.handle_event(start_doc()).unwrap();
        writer.handle_event(start_para()).unwrap();
        writer.handle_event(text("hi")).unwrap();
        writer.finish().unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "[Para [Str \"hi\"]]");
    }

    #[test]
    fn write_error_propagates() {
        let mut writer = PandocNativeWriter::new(FailingWriter);
        let result = writer.handle_event(start_doc());
        assert!(result.is_err());
    }

    #[test]
    fn try_run_returns_ok_for_valid_events() {
        let result = try_run([
            start_doc(),
            start_para(),
            text("ok"),
            Event::EndParagraph,
            Event::EndDocument,
        ]);
        assert_eq!(result.unwrap(), "[Para [Str \"ok\"]]");
    }

    #[test]
    fn heading_basic_level_1_no_id() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, None),
                text("Hello"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 1 (\"\",[],[]) [Str \"Hello\"]]"
        );
    }

    #[test]
    fn heading_with_id() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(2, Some("sec1")),
                text("Hello"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 2 (\"sec1\",[],[]) [Str \"Hello\"]]"
        );
    }

    #[test]
    fn heading_empty_body() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, None),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 1 (\"\",[],[]) []]"
        );
    }

    #[test]
    fn heading_levels_1_through_9() {
        for level in 1..=9 {
            let out = run([
                start_doc(),
                start_heading(level, None),
                text("h"),
                Event::EndHeading,
                Event::EndDocument,
            ]);
            assert_eq!(out, format!("[Header {level} (\"\",[],[]) [Str \"h\"]]"));
        }
    }

    #[test]
    fn heading_level_zero_passes_through_raw() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(0, None),
                text("h"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 0 (\"\",[],[]) [Str \"h\"]]"
        );
    }

    #[test]
    fn heading_level_above_nine_passes_through_raw() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(42, None),
                text("h"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 42 (\"\",[],[]) [Str \"h\"]]"
        );
    }

    #[test]
    fn heading_level_u8_max_passes_through_raw() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(u8::MAX, None),
                text("h"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 255 (\"\",[],[]) [Str \"h\"]]"
        );
    }

    #[test]
    fn heading_with_multiple_text_runs_separated_by_comma() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, None),
                text("first"),
                text("second"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 1 (\"\",[],[]) [Str \"first\",Str \"second\"]]"
        );
    }

    #[test]
    fn heading_with_line_break_inline() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, None),
                text("a"),
                Event::LineBreak,
                text("b"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 1 (\"\",[],[]) [Str \"a\",LineBreak,Str \"b\"]]"
        );
    }

    #[test]
    fn heading_with_soft_break_inline() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, None),
                text("a"),
                Event::SoftBreak,
                text("b"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 1 (\"\",[],[]) [Str \"a\",SoftBreak,Str \"b\"]]"
        );
    }

    #[test]
    fn heading_id_with_special_chars_is_escaped() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, Some("a\"b\\c")),
                text("x"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 1 (\"a\\\"b\\\\c\",[],[]) [Str \"x\"]]"
        );
    }

    #[test]
    fn heading_between_paragraphs_with_comma_separation() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("before"),
                Event::EndParagraph,
                start_heading(2, None),
                text("title"),
                Event::EndHeading,
                start_para(),
                text("after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Str \"before\"],Header 2 (\"\",[],[]) [Str \"title\"],Para [Str \"after\"]]"
        );
    }

    #[test]
    fn multiple_consecutive_headings() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, None),
                text("one"),
                Event::EndHeading,
                start_heading(2, Some("two")),
                text("two"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 1 (\"\",[],[]) [Str \"one\"],Header 2 (\"two\",[],[]) [Str \"two\"]]"
        );
    }

    #[test]
    fn heading_outside_document_ignored() {
        assert_eq!(
            run([start_heading(1, None), text("dropped"), Event::EndHeading,]),
            ""
        );
    }

    #[test]
    fn start_heading_inside_paragraph_is_silently_dropped() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                start_heading(1, None),
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Str \"a\",Str \"b\"]]"
        );
    }

    #[test]
    fn stray_end_heading_inside_paragraph_does_not_close_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                Event::EndHeading,
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Str \"a\",Str \"b\"]]"
        );
    }

    #[test]
    fn stray_end_paragraph_inside_heading_does_not_close_heading() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, None),
                text("a"),
                Event::EndParagraph,
                text("b"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 1 (\"\",[],[]) [Str \"a\",Str \"b\"]]"
        );
    }

    #[test]
    fn bold_wraps_text_inside_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                start_style(docspec_core::TextStyleKind::Bold),
                text("b"),
                Event::EndTextStyle,
                text("c"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Str \"a\",Strong [Str \"b\"],Str \"c\"]]"
        );
    }

    #[test]
    fn italic_wraps_text_inside_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Italic),
                text("hello"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Emph [Str \"hello\"]]]"
        );
    }

    #[test]
    fn strikethrough_wraps_text_inside_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Strikethrough),
                text("gone"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Strikeout [Str \"gone\"]]]"
        );
    }

    #[test]
    fn underline_wraps_text_inside_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Underline),
                text("under"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Underline [Str \"under\"]]]"
        );
    }

    #[test]
    fn subscript_wraps_text_inside_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Subscript),
                text("2"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Subscript [Str \"2\"]]]"
        );
    }

    #[test]
    fn superscript_wraps_text_inside_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Superscript),
                text("th"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Superscript [Str \"th\"]]]"
        );
    }

    #[test]
    fn adjacent_styles_each_emit_their_wrapper() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Bold),
                text("b"),
                Event::EndTextStyle,
                start_style(docspec_core::TextStyleKind::Italic),
                text("i"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Strong [Str \"b\"],Emph [Str \"i\"]]]"
        );
    }

    #[test]
    fn nested_styles_emit_nested_wrappers() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Bold),
                start_style(docspec_core::TextStyleKind::Italic),
                text("bi"),
                Event::EndTextStyle,
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Strong [Emph [Str \"bi\"]]]]"
        );
    }

    #[test]
    fn style_inside_heading_emits_wrapper() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(2, Some("h")),
                text("a "),
                start_style(docspec_core::TextStyleKind::Bold),
                text("bold"),
                Event::EndTextStyle,
                text(" b"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 2 (\"h\",[],[]) [Str \"a \",Strong [Str \"bold\"],Str \" b\"]]"
        );
    }

    #[test]
    fn style_with_multiple_inlines_inside_wrapper() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Bold),
                text("a"),
                Event::SoftBreak,
                text("b"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Strong [Str \"a\",SoftBreak,Str \"b\"]]]"
        );
    }

    #[test]
    fn code_style_emits_code_construct_inside_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a "),
                start_style(docspec_core::TextStyleKind::Code),
                text("code"),
                Event::EndTextStyle,
                text(" b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Str \"a \",Code (\"\",[],[]) \"code\",Str \" b\"]]"
        );
    }

    #[test]
    fn code_style_inside_bold_emits_code_inside_strong() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Bold),
                start_style(docspec_core::TextStyleKind::Code),
                text("a"),
                Event::EndTextStyle,
                text("b"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Strong [Code (\"\",[],[]) \"a\",Str \"b\"]]]"
        );
    }

    #[test]
    fn nested_styles_inside_code_are_absorbed_into_buffer() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Code),
                start_style(docspec_core::TextStyleKind::Bold),
                text("a"),
                Event::EndTextStyle,
                text("b"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Code (\"\",[],[]) \"ab\"]]"
        );
    }

    #[test]
    fn mark_style_text_flattens_into_paragraph() {
        let color = docspec_core::Color::Rgb {
            r: 255,
            g: 255,
            b: 0,
        };
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a "),
                start_style(docspec_core::TextStyleKind::Mark(color)),
                text("mark"),
                Event::EndTextStyle,
                text(" b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Str \"a \",Str \"mark\",Str \" b\"]]"
        );
    }

    #[test]
    fn text_color_style_text_flattens_into_paragraph() {
        let color = docspec_core::Color::Rgb { r: 255, g: 0, b: 0 };
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a "),
                start_style(docspec_core::TextStyleKind::TextColor(color)),
                text("red"),
                Event::EndTextStyle,
                text(" b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Str \"a \",Str \"red\",Str \" b\"]]"
        );
    }

    #[test]
    fn start_text_style_outside_inline_block_is_dropped() {
        assert_eq!(
            run([
                start_doc(),
                start_style(docspec_core::TextStyleKind::Bold),
                text("b"),
                Event::EndTextStyle,
                Event::EndDocument,
            ]),
            "[]"
        );
    }

    #[test]
    fn stray_end_text_style_outside_inline_block_is_noop() {
        assert_eq!(
            run([start_doc(), Event::EndTextStyle, Event::EndDocument]),
            "[]"
        );
    }

    #[test]
    fn stray_end_text_style_inside_paragraph_does_not_close_paragraph() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                Event::EndTextStyle,
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Str \"a\",Str \"b\"]]"
        );
    }

    #[test]
    fn unclosed_style_at_end_of_paragraph_is_closed_defensively() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Bold),
                text("a"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Strong [Str \"a\"]]]"
        );
    }

    #[test]
    fn unclosed_style_at_end_of_heading_is_closed_defensively() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, None),
                start_style(docspec_core::TextStyleKind::Italic),
                text("x"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 1 (\"\",[],[]) [Emph [Str \"x\"]]]"
        );
    }

    #[test]
    fn unclosed_style_at_end_of_document_is_closed_defensively() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Bold),
                text("a"),
                Event::EndDocument,
            ]),
            "[Para [Strong [Str \"a\"]]]"
        );
    }

    fn start_pre(id: Option<&str>, syntax: Option<&str>) -> Event {
        Event::StartPreformatted {
            id: id.map(String::from),
            syntax: syntax.map(String::from),
        }
    }

    #[test]
    fn preformatted_with_no_id_no_syntax_emits_empty_attr() {
        assert_eq!(
            run([
                start_doc(),
                start_pre(None, None),
                text("hello"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "[CodeBlock (\"\",[],[]) \"hello\"]"
        );
    }

    #[test]
    fn preformatted_with_syntax_emits_language_class() {
        assert_eq!(
            run([
                start_doc(),
                start_pre(None, Some("rust")),
                text("fn main() {}"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "[CodeBlock (\"\",[\"rust\"],[]) \"fn main() {}\"]"
        );
    }

    #[test]
    fn preformatted_with_id_and_syntax_emits_both() {
        assert_eq!(
            run([
                start_doc(),
                start_pre(Some("ex1"), Some("python")),
                text("print('hi')"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "[CodeBlock (\"ex1\",[\"python\"],[]) \"print('hi')\"]"
        );
    }

    #[test]
    fn preformatted_concatenates_multiple_text_events() {
        assert_eq!(
            run([
                start_doc(),
                start_pre(None, None),
                text("line1\n"),
                text("line2\n"),
                text("line3"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "[CodeBlock (\"\",[],[]) \"line1\\nline2\\nline3\"]"
        );
    }

    #[test]
    fn preformatted_preserves_literal_newlines_in_text() {
        assert_eq!(
            run([
                start_doc(),
                start_pre(None, None),
                text("a\nb\nc"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "[CodeBlock (\"\",[],[]) \"a\\nb\\nc\"]"
        );
    }

    #[test]
    fn preformatted_with_empty_content_emits_empty_string() {
        assert_eq!(
            run([
                start_doc(),
                start_pre(None, None),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "[CodeBlock (\"\",[],[]) \"\"]"
        );
    }

    #[test]
    fn preformatted_escapes_quotes_and_backslashes_in_content() {
        assert_eq!(
            run([
                start_doc(),
                start_pre(None, Some("c")),
                text("printf(\"hello\\n\");"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "[CodeBlock (\"\",[\"c\"],[]) \"printf(\\\"hello\\\\n\\\");\"]"
        );
    }

    #[test]
    fn preformatted_escapes_id_attribute() {
        assert_eq!(
            run([
                start_doc(),
                start_pre(Some("a\"b"), None),
                text("x"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "[CodeBlock (\"a\\\"b\",[],[]) \"x\"]"
        );
    }

    #[test]
    fn preformatted_between_paragraphs_emits_block_separators() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("before"),
                Event::EndParagraph,
                start_pre(None, Some("sh")),
                text("ls -la"),
                Event::EndPreformatted,
                start_para(),
                text("after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Str \"before\"],CodeBlock (\"\",[\"sh\"],[]) \"ls -la\",Para [Str \"after\"]]"
        );
    }

    #[test]
    fn start_preformatted_inside_paragraph_is_dropped_but_text_passes_through() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("a"),
                start_pre(None, Some("rust")),
                text("nope"),
                Event::EndPreformatted,
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Str \"a\",Str \"nope\",Str \"b\"]]"
        );
    }

    #[test]
    fn stray_end_preformatted_is_noop() {
        assert_eq!(
            run([start_doc(), Event::EndPreformatted, Event::EndDocument]),
            "[]"
        );
    }

    #[test]
    fn inline_code_with_no_id_emits_empty_attr() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Code),
                text("let x = 1;"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Code (\"\",[],[]) \"let x = 1;\"]]"
        );
    }

    #[test]
    fn inline_code_with_id_emits_id() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: docspec_core::TextStyleKind::Code,
                    id: Some("ref1".to_string()),
                },
                text("ref"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Code (\"ref1\",[],[]) \"ref\"]]"
        );
    }

    #[test]
    fn inline_code_between_plain_text_emits_separators() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                text("before "),
                start_style(docspec_core::TextStyleKind::Code),
                text("x"),
                Event::EndTextStyle,
                text(" after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Str \"before \",Code (\"\",[],[]) \"x\",Str \" after\"]]"
        );
    }

    #[test]
    fn inline_code_inside_heading() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, Some("h")),
                text("Use "),
                start_style(docspec_core::TextStyleKind::Code),
                text("foo()"),
                Event::EndTextStyle,
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 1 (\"h\",[],[]) [Str \"Use \",Code (\"\",[],[]) \"foo()\"]]"
        );
    }

    #[test]
    fn inline_code_inside_wrapper_style() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Bold),
                text("bold "),
                start_style(docspec_core::TextStyleKind::Code),
                text("code"),
                Event::EndTextStyle,
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Strong [Str \"bold \",Code (\"\",[],[]) \"code\"]]]"
        );
    }

    #[test]
    fn inline_code_concatenates_multiple_text_events() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Code),
                text("a"),
                text("b"),
                text("c"),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Code (\"\",[],[]) \"abc\"]]"
        );
    }

    #[test]
    fn inline_code_escapes_quotes_in_content() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Code),
                text("say \"hi\""),
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Code (\"\",[],[]) \"say \\\"hi\\\"\"]]"
        );
    }

    #[test]
    fn inline_code_outside_inline_block_is_dropped() {
        assert_eq!(
            run([
                start_doc(),
                start_style(docspec_core::TextStyleKind::Code),
                text("x"),
                Event::EndTextStyle,
                Event::EndDocument,
            ]),
            "[]"
        );
    }

    #[test]
    fn unclosed_inline_code_at_end_of_paragraph_does_not_swallow_following_block() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Code),
                text("x"),
                Event::EndParagraph,
                start_para(),
                text("y"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Code (\"\",[],[]) \"x\"],Para [Str \"y\"]]"
        );
    }

    #[test]
    fn unclosed_inline_code_at_end_of_heading_does_not_swallow_following_block() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, None),
                start_style(docspec_core::TextStyleKind::Code),
                text("x"),
                Event::EndHeading,
                start_para(),
                text("y"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Header 1 (\"\",[],[]) [Code (\"\",[],[]) \"x\"],Para [Str \"y\"]]"
        );
    }

    #[test]
    fn unclosed_inline_code_at_end_of_paragraph_is_flushed_defensively() {
        assert_eq!(
            run([
                start_doc(),
                start_para(),
                start_style(docspec_core::TextStyleKind::Code),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "[Para [Code (\"\",[],[]) \"x\"]]"
        );
    }

    #[test]
    fn unclosed_inline_code_at_end_of_heading_is_flushed_defensively() {
        assert_eq!(
            run([
                start_doc(),
                start_heading(1, None),
                start_style(docspec_core::TextStyleKind::Code),
                text("x"),
                Event::EndHeading,
                Event::EndDocument,
            ]),
            "[Header 1 (\"\",[],[]) [Code (\"\",[],[]) \"x\"]]"
        );
    }

    #[test]
    fn unclosed_preformatted_at_end_of_document_is_flushed_defensively() {
        assert_eq!(
            run([
                start_doc(),
                start_pre(None, Some("rust")),
                text("fn main"),
                Event::EndDocument,
            ]),
            "[CodeBlock (\"\",[\"rust\"],[]) \"fn main\"]"
        );
    }

    #[test]
    fn nested_inline_events_inside_preformatted_are_ignored() {
        assert_eq!(
            run([
                start_doc(),
                start_pre(None, None),
                text("a"),
                Event::LineBreak,
                Event::SoftBreak,
                text("b"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "[CodeBlock (\"\",[],[]) \"ab\"]"
        );
    }
}
