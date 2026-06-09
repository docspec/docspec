#![forbid(unsafe_code)]

//! Streaming HTML5 writer for `DocSpec` events.

use docspec_core::{Event, EventSink, Result};
use html5ever::serialize::{HtmlSerializer, SerializeOpts, Serializer as _};
use html5ever::{local_name, ns, LocalName, QualName};
use std::io::Write;

/// A streaming HTML5 writer for `DocSpec` events.
///
/// Writes HTML5 markup directly to the underlying `Write` as events arrive.
/// Implements [`EventSink`] for integration with the `DocSpec` pipeline.
///
/// # Type Parameters
///
/// * `W` - Any type implementing [`Write`]
pub struct HtmlWriter<W: Write> {
    finished: bool,
    in_paragraph: bool,
    serializer: HtmlSerializer<W>,
    started: bool,
}

impl<W: Write> HtmlWriter<W> {
    fn close(&mut self, local: LocalName) -> Result<()> {
        let name = QualName::new(None, ns!(html), local);
        self.serializer.end_elem(name)?;
        Ok(())
    }

    /// Creates a new `HtmlWriter` that writes to the given writer.
    #[inline]
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self {
            serializer: HtmlSerializer::new(writer, SerializeOpts::default()),
            started: false,
            finished: false,
            in_paragraph: false,
        }
    }

    fn open(&mut self, local: LocalName) -> Result<()> {
        let name = QualName::new(None, ns!(html), local);
        self.serializer
            .start_elem(name, core::iter::empty::<(&QualName, &str)>())?;
        Ok(())
    }
}

impl<W: Write> EventSink for HtmlWriter<W> {
    #[inline]
    fn finish(mut self) -> Result<()> {
        self.serializer.writer.flush()?;
        Ok(())
    }

    #[inline]
    fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::StartDocument { .. } => {
                if !self.started && !self.finished {
                    self.open(local_name!("html"))?;
                    self.open(local_name!("body"))?;
                    self.started = true;
                }
            }
            Event::EndDocument => {
                if self.started && !self.finished {
                    if self.in_paragraph {
                        self.close(local_name!("p"))?;
                        self.in_paragraph = false;
                    }
                    self.close(local_name!("body"))?;
                    self.close(local_name!("html"))?;
                    self.finished = true;
                }
            }
            Event::StartParagraph { .. } => {
                if self.started && !self.finished && !self.in_paragraph {
                    self.open(local_name!("p"))?;
                    self.in_paragraph = true;
                }
            }
            Event::EndParagraph => {
                if self.in_paragraph {
                    self.close(local_name!("p"))?;
                    self.in_paragraph = false;
                }
            }
            Event::Text { content } if self.in_paragraph => {
                self.serializer.write_text(&content)?;
            }
            _ => {
                // HTML writer drops all inline styles (StartTextStyle, EndTextStyle) per documented contract
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic_in_result_fn, clippy::unwrap_used)]

    use super::HtmlWriter;
    use docspec_core::{Event, EventSink as _, Result};

    fn assert_output(events: impl IntoIterator<Item = Event>, expected: &str) {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = HtmlWriter::new(&mut buf);
        for e in events {
            let _r = writer.handle_event(e);
        }
        let _r = writer.finish();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, expected);
    }

    #[test]
    fn autoclose_paragraph_on_enddocument() {
        assert_output(
            [
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "oops".to_string(),
                },
                Event::EndDocument,
            ],
            "<html><body><p>oops</p></body></html>",
        );
    }

    #[test]
    fn double_start_document_is_noop() {
        assert_output(
            [
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::EndDocument,
            ],
            "<html><body></body></html>",
        );
    }

    #[test]
    fn empty_document_exact_output() {
        assert_output(
            [
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::EndDocument,
            ],
            "<html><body></body></html>",
        );
    }

    #[test]
    fn end_paragraph_without_start() {
        assert_output(
            [
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ],
            "<html><body></body></html>",
        );
    }

    #[test]
    fn escapes_special_chars() {
        assert_output(
            [
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "a & b < c > d".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ],
            "<html><body><p>a &amp; b &lt; c &gt; d</p></body></html>",
        );
    }

    #[test]
    fn finish_after_normal_document_succeeds() -> Result<()> {
        let mut buf: Vec<u8> = Vec::new();
        let mut writer = HtmlWriter::new(&mut buf);
        writer.handle_event(Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        })?;
        writer.handle_event(Event::StartParagraph {
            alignment: None,
            id: None,
        })?;
        writer.handle_event(Event::Text {
            content: "hello".to_string(),
        })?;
        writer.handle_event(Event::EndParagraph)?;
        writer.handle_event(Event::EndDocument)?;
        writer.finish()
    }

    #[test]
    fn ignored_events_no_effect() {
        assert_output(
            [
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartHeading { level: 1, id: None },
                Event::EndHeading,
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "x".to_string(),
                },
                Event::EndParagraph,
                Event::ThematicBreak { id: None },
                Event::EndDocument,
            ],
            "<html><body><p>x</p></body></html>",
        );
    }

    #[test]
    fn paragraph_with_text() {
        assert_output(
            [
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "hello".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ],
            "<html><body><p>hello</p></body></html>",
        );
    }

    #[test]
    fn start_paragraph_while_in_paragraph() {
        assert_output(
            [
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::Text {
                    content: "x".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ],
            "<html><body><p>x</p></body></html>",
        );
    }

    #[test]
    fn text_outside_paragraph_ignored() {
        assert_output(
            [
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::Text {
                    content: "ignored".to_string(),
                },
                Event::EndDocument,
            ],
            "<html><body></body></html>",
        );
    }
}
