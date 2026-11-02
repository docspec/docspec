//! Integration tests for `HtmlWriter`.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use core::cell::Cell;
use docspec_core::{Error, Event, EventSink as _, ImageSource, ListStyleType, TextStyleKind};
use docspec_html_writer::HtmlWriter;
use docspec_test_utils::FailingWriter;
use std::io::{self, Write};
use std::rc::Rc;

struct FlushTrackingWriter {
    buf: Vec<u8>,
    flushed: Rc<Cell<bool>>,
}

impl Write for FlushTrackingWriter {
    fn flush(&mut self) -> io::Result<()> {
        self.flushed.set(true);
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::panic_in_result_fn)]

    use super::*;

    #[test]
    fn empty_document_produces_minimal_html() {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = HtmlWriter::new(&mut buf);
        let _r1 = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        let _r2 = writer.handle_event(Event::EndDocument);
        let _r3 = writer.finish();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "<html><body></body></html>");
    }

    #[test]
    fn finish_flushes_writer() {
        let flushed = Rc::new(Cell::new(false));
        let mut writer = HtmlWriter::new(FlushTrackingWriter {
            buf: Vec::new(),
            flushed: Rc::clone(&flushed),
        });
        let _r1 = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        let _r2 = writer.handle_event(Event::StartParagraph {
            alignment: None,
            id: None,
        });
        let _r3 = writer.handle_event(Event::Text {
            content: "hello".to_string(),
        });
        let _r4 = writer.handle_event(Event::EndParagraph);
        let _r5 = writer.handle_event(Event::EndDocument);
        let _r6 = writer.finish();
        assert!(flushed.get());
    }

    #[test]
    fn multiple_paragraphs_render_in_order() {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = HtmlWriter::new(&mut buf);
        let _r1 = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        let _r2 = writer.handle_event(Event::StartParagraph {
            alignment: None,
            id: None,
        });
        let _r3 = writer.handle_event(Event::Text {
            content: "first".to_string(),
        });
        let _r4 = writer.handle_event(Event::EndParagraph);
        let _r5 = writer.handle_event(Event::StartParagraph {
            alignment: None,
            id: None,
        });
        let _r6 = writer.handle_event(Event::Text {
            content: "second".to_string(),
        });
        let _r7 = writer.handle_event(Event::EndParagraph);
        let _r8 = writer.handle_event(Event::EndDocument);
        let _r9 = writer.finish();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(
            output,
            "<html><body><p>first</p><p>second</p></body></html>"
        );
    }

    #[test]
    fn single_paragraph_with_text() {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = HtmlWriter::new(&mut buf);
        let _r1 = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        let _r2 = writer.handle_event(Event::StartParagraph {
            alignment: None,
            id: None,
        });
        let _r3 = writer.handle_event(Event::Text {
            content: "hello world".to_string(),
        });
        let _r4 = writer.handle_event(Event::EndParagraph);
        let _r5 = writer.handle_event(Event::EndDocument);
        let _r6 = writer.finish();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "<html><body><p>hello world</p></body></html>");
    }

    #[test]
    fn text_outside_paragraph_is_ignored() {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = HtmlWriter::new(&mut buf);
        let _r1 = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        let _r2 = writer.handle_event(Event::Text {
            content: "ignored".to_string(),
        });
        let _r3 = writer.handle_event(Event::EndDocument);
        let _r4 = writer.finish();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "<html><body></body></html>");
    }

    #[test]
    fn text_special_chars_are_escaped() {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = HtmlWriter::new(&mut buf);
        let _r1 = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        let _r2 = writer.handle_event(Event::StartParagraph {
            alignment: None,
            id: None,
        });
        let _r3 = writer.handle_event(Event::Text {
            content: "a & b < c > d".to_string(),
        });
        let _r4 = writer.handle_event(Event::EndParagraph);
        let _r5 = writer.handle_event(Event::EndDocument);
        let _r6 = writer.finish();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(
            output,
            "<html><body><p>a &amp; b &lt; c &gt; d</p></body></html>"
        );
    }

    #[test]
    fn unsupported_events_are_silently_ignored() {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = HtmlWriter::new(&mut buf);
        let _r1 = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        let _r2 = writer.handle_event(Event::StartHeading { level: 1, id: None });
        let _r3 = writer.handle_event(Event::EndHeading);
        let _r4 = writer.handle_event(Event::StartUnorderedListItem {
            id: None,
            level: 0,
            style_type: ListStyleType::Disc,
        });
        let _r5 = writer.handle_event(Event::EndUnorderedListItem);
        let _r6 = writer.handle_event(Event::StartTable { id: None });
        let _r7 = writer.handle_event(Event::EndTable);
        let _r8 = writer.handle_event(Event::ThematicBreak { id: None });
        let _r9 = writer.handle_event(Event::Image {
            alt: None,
            decorative: false,
            id: None,
            source: ImageSource::Uri {
                uri: "x".to_string(),
            },
            title: None,
        });
        let _r10 = writer.handle_event(Event::LineBreak);
        let _r11 = writer.handle_event(Event::SoftBreak);
        let _r12 = writer.handle_event(Event::StartLink {
            href: "x".to_string(),
            id: None,
            title: None,
        });
        let _r13 = writer.handle_event(Event::EndLink);
        let _r14 = writer.handle_event(Event::EndDocument);
        let _r15 = writer.finish();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "<html><body></body></html>");
    }

    #[test]
    fn write_failure_propagates_error() {
        let mut writer =
            HtmlWriter::new(FailingWriter::new(0).with_kind(io::ErrorKind::BrokenPipe));
        let result = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        match result {
            Err(Error::Io { source }) => {
                assert_eq!(source.kind(), io::ErrorKind::BrokenPipe);
            }
            _ => {
                panic!("expected Err(Error::Io)");
            }
        }
    }

    #[test]
    fn styled_input_unstyled_output() {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = HtmlWriter::new(&mut buf);
        let _r1 = writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        });
        let _r2 = writer.handle_event(Event::StartParagraph {
            alignment: None,
            id: None,
        });
        let _r3 = writer.handle_event(Event::StartTextStyle {
            kind: TextStyleKind::Bold,
            id: None,
        });
        let _r4 = writer.handle_event(Event::Text {
            content: "hi".to_string(),
        });
        let _r5 = writer.handle_event(Event::EndTextStyle);
        let _r6 = writer.handle_event(Event::EndParagraph);
        let _r7 = writer.handle_event(Event::EndDocument);
        let _r8 = writer.finish();
        let output = String::from_utf8(buf).unwrap();
        // HTML writer drops styles — no <b> tag
        assert!(output.contains("hi"));
        assert!(!output.contains("<b>"));
    }

    fn run(events: impl IntoIterator<Item = Event>) -> String {
        let mut buf: Vec<u8> = Vec::new();
        let writer = HtmlWriter::new(&mut buf);
        docspec_test_utils::drive(writer, events);
        String::from_utf8(buf).unwrap()
    }

    fn start_pre(syntax: Option<&str>) -> Event {
        Event::StartPreformatted {
            id: None,
            syntax: syntax.map(String::from),
        }
    }

    fn text(content: &str) -> Event {
        Event::Text {
            content: content.to_string(),
        }
    }

    fn start_document() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    fn start_paragraph() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    #[test]
    fn preformatted_with_syntax_emits_language_class() {
        assert_eq!(
            run([
                start_document(),
                start_pre(Some("rust")),
                text("fn main() {}"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code class=\"language-rust\">fn main() {}</code></pre></body></html>"
        );
    }

    #[test]
    fn preformatted_without_syntax_emits_bare_code() {
        assert_eq!(
            run([
                start_document(),
                start_pre(None),
                text("x"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code>x</code></pre></body></html>"
        );
    }

    #[test]
    fn preformatted_with_empty_syntax_emits_no_class() {
        assert_eq!(
            run([
                start_document(),
                start_pre(Some("")),
                text("x"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code>x</code></pre></body></html>"
        );
    }

    #[test]
    fn empty_preformatted_emits_empty_code() {
        assert_eq!(
            run([
                start_document(),
                start_pre(None),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code></code></pre></body></html>"
        );
    }

    #[test]
    fn preformatted_escapes_html_special_chars() {
        assert_eq!(
            run([
                start_document(),
                start_pre(None),
                text("if a < b && c > d { \"q\" }"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code>if a &lt; b &amp;&amp; c &gt; d { \"q\" }</code></pre></body></html>"
        );
    }

    #[test]
    fn preformatted_escapes_syntax_in_class_attribute() {
        assert_eq!(
            run([
                start_document(),
                start_pre(Some("c\"&x")),
                text("x"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code class=\"language-c&quot;&amp;x\">x</code></pre></body></html>"
        );
    }

    #[test]
    fn preformatted_keeps_whitespace_and_literal_newlines() {
        assert_eq!(
            run([
                start_document(),
                start_pre(None),
                text("\n  indented\n\ttab"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code>\n  indented\n\ttab</code></pre></body></html>"
        );
    }

    #[test]
    fn preformatted_concatenates_text_events() {
        assert_eq!(
            run([
                start_document(),
                start_pre(None),
                text("a"),
                text("b"),
                text("c"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code>abc</code></pre></body></html>"
        );
    }

    #[test]
    fn line_and_soft_breaks_inside_preformatted_each_emit_a_newline() {
        assert_eq!(
            run([
                start_document(),
                start_pre(None),
                text("a"),
                Event::LineBreak,
                text("b"),
                Event::SoftBreak,
                text("c"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code>a\nb\nc</code></pre></body></html>"
        );
    }

    #[test]
    fn breaks_inside_paragraph_are_still_dropped() {
        assert_eq!(
            run([
                start_document(),
                start_paragraph(),
                text("a"),
                Event::LineBreak,
                Event::SoftBreak,
                text("b"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "<html><body><p>ab</p></body></html>"
        );
    }

    #[test]
    fn preformatted_between_paragraphs_keeps_document_order() {
        assert_eq!(
            run([
                start_document(),
                start_paragraph(),
                text("before"),
                Event::EndParagraph,
                start_pre(None),
                text("code"),
                Event::EndPreformatted,
                start_paragraph(),
                text("after"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "<html><body><p>before</p><pre><code>code</code></pre><p>after</p></body></html>"
        );
    }

    #[test]
    fn preformatted_id_is_not_written() {
        assert_eq!(
            run([
                start_document(),
                Event::StartPreformatted {
                    id: Some("c1".to_string()),
                    syntax: None,
                },
                text("x"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code>x</code></pre></body></html>"
        );
    }

    #[test]
    fn unclosed_preformatted_is_closed_at_end_document() {
        assert_eq!(
            run([
                start_document(),
                start_pre(Some("sh")),
                text("ls"),
                Event::EndDocument,
            ]),
            "<html><body><pre><code class=\"language-sh\">ls</code></pre></body></html>"
        );
    }

    #[test]
    fn start_preformatted_inside_paragraph_is_ignored() {
        assert_eq!(
            run([
                start_document(),
                start_paragraph(),
                text("a"),
                start_pre(None),
                text("b"),
                Event::EndPreformatted,
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "<html><body><p>ab</p></body></html>"
        );
    }

    #[test]
    fn start_paragraph_inside_preformatted_is_ignored() {
        assert_eq!(
            run([
                start_document(),
                start_pre(None),
                text("a"),
                start_paragraph(),
                text("b"),
                Event::EndParagraph,
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code>ab</code></pre></body></html>"
        );
    }

    #[test]
    fn nested_start_preformatted_is_ignored() {
        assert_eq!(
            run([
                start_document(),
                start_pre(None),
                text("a"),
                start_pre(Some("rust")),
                text("b"),
                Event::EndPreformatted,
                text("c"),
                Event::EndPreformatted,
                Event::EndDocument,
            ]),
            "<html><body><pre><code>ab</code></pre></body></html>"
        );
    }

    #[test]
    fn stray_end_preformatted_is_noop() {
        assert_eq!(
            run([
                start_document(),
                Event::EndPreformatted,
                start_paragraph(),
                text("x"),
                Event::EndParagraph,
                Event::EndDocument,
            ]),
            "<html><body><p>x</p></body></html>"
        );
    }

    #[test]
    fn preformatted_outside_document_is_ignored() {
        assert_eq!(
            run([
                start_pre(None),
                text("x"),
                Event::EndPreformatted,
                start_document(),
                Event::EndDocument,
            ]),
            "<html><body></body></html>"
        );
    }

    #[test]
    fn text_after_preformatted_outside_any_block_is_dropped() {
        assert_eq!(
            run([
                start_document(),
                start_pre(None),
                text("in"),
                Event::EndPreformatted,
                text("out"),
                Event::EndDocument,
            ]),
            "<html><body><pre><code>in</code></pre></body></html>"
        );
    }

    /// A `Write` whose bytes stay readable while the writer under test owns a
    /// clone, so a test can see what was written before a block ends.
    #[derive(Clone, Default)]
    struct SharedBuffer(Rc<core::cell::RefCell<Vec<u8>>>);

    impl Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl SharedBuffer {
        fn contents(&self) -> String {
            String::from_utf8(self.0.borrow().clone()).unwrap()
        }
    }

    #[test]
    fn preformatted_text_is_written_before_end_preformatted() {
        let out = SharedBuffer::default();
        let mut writer = HtmlWriter::new(out.clone());
        for event in [
            start_document(),
            start_pre(Some("rust")),
            text("fn main() {"),
            Event::LineBreak,
        ] {
            writer.handle_event(event).unwrap();
        }
        assert_eq!(
            out.contents(),
            "<html><body><pre><code class=\"language-rust\">fn main() {\n"
        );
        for event in [text("}"), Event::EndPreformatted, Event::EndDocument] {
            writer.handle_event(event).unwrap();
        }
        writer.finish().unwrap();
        assert_eq!(
            out.contents(),
            "<html><body><pre><code class=\"language-rust\">fn main() {\n}</code></pre></body></html>"
        );
    }

    #[test]
    fn write_failure_at_start_preformatted_propagates_error() {
        // StartDocument takes six writes (`<`, `html`, `>`, then `<`, `body`, `>`);
        // the first write of `<pre>` fails.
        let mut writer = HtmlWriter::new(FailingWriter::new(6));
        writer.handle_event(start_document()).unwrap();
        match writer.handle_event(start_pre(None)) {
            Err(Error::Io { source }) => {
                assert_eq!(source.kind(), io::ErrorKind::Other);
                assert_eq!(source.to_string(), "simulated write failure");
            }
            other => panic!("expected Error::Io, got: {other:?}"),
        }
    }
}
