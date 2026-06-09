//! Integration tests for `HtmlWriter`.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use core::cell::Cell;
use docspec_core::{Error, Event, EventSink as _, ImageSource, ListStyleType, TextStyleKind};
use docspec_html_writer::HtmlWriter;
use std::io::{self, Write};
use std::rc::Rc;

struct FailingWriter;

impl Write for FailingWriter {
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "fail"))
    }

    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "fail"))
    }
}

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
        let mut writer = HtmlWriter::new(FailingWriter);
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
}
