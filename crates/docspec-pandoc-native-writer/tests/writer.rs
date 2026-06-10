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
    fn styled_input_dropped() {
        let styled_events = vec![
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartParagraph {
                id: None,
                alignment: None,
            },
            Event::StartTextStyle {
                kind: docspec_core::TextStyleKind::Bold,
                id: None,
            },
            Event::Text {
                content: "x".to_string(),
            },
            Event::EndTextStyle,
            Event::EndParagraph,
            Event::EndDocument,
        ];
        let unstyled_events = vec![
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::StartParagraph {
                id: None,
                alignment: None,
            },
            Event::Text {
                content: "x".to_string(),
            },
            Event::EndParagraph,
            Event::EndDocument,
        ];
        let styled_output = run(styled_events);
        let unstyled_output = run(unstyled_events);
        assert_eq!(styled_output, unstyled_output);
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
                Event::StartPreformatted {
                    syntax: None,
                    id: None
                },
                Event::EndPreformatted,
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
}
