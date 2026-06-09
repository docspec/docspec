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
                Event::StartHeading { level: 1, id: None },
                Event::EndHeading,
                start_para(),
                text("x"),
                Event::EndParagraph,
                Event::ThematicBreak { id: None },
                Event::EndDocument
            ]),
            "[Para [Str \"x\"]]"
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
}
