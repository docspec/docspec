#![forbid(unsafe_code)]

//! Streaming HTML5 writer for `DocSpec` events.
//!
//! **Experimental.** This writer emits paragraphs, preformatted (code) blocks and
//! their text, and nothing else. Headings, lists, tables, links, images, text styles
//! and block quotes are all silently dropped, as are breaks outside a code block and
//! any `Text` event that arrives outside a paragraph or code block. A heading
//! therefore produces no output at all — not even its text. Do not rely on this
//! writer where fidelity matters.
//!
//! A preformatted block becomes `<pre><code>…</code></pre>`, with
//! `class="language-…"` on `<code>` when the block names a language. Its text is
//! escaped and written as it arrives, and `LineBreak` and `SoftBreak` inside it
//! each become a newline. Block ids are not written, for code blocks as for
//! paragraphs.

use docspec_core::{Event, EventSink, Result};
use html5ever::serialize::{HtmlSerializer, SerializeOpts, Serializer as _};
use html5ever::{local_name, ns, LocalName, QualName};
use std::io::Write;

/// Where the writer is in the document: exactly one of these at any time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Before `StartDocument`: nothing written yet.
    NotStarted,
    /// Inside `<body>`, between blocks.
    InBody,
    /// Inside a `<p>`.
    InParagraph,
    /// Inside the `<pre><code>` of a preformatted block.
    InPreformatted,
    /// After `EndDocument`: `</body></html>` written, every further event ignored.
    Finished,
}

/// A streaming HTML5 writer for `DocSpec` events.
///
/// Writes HTML5 markup directly to the underlying `Write` as events arrive.
/// Implements [`EventSink`] for integration with the `DocSpec` pipeline.
///
/// # Type Parameters
///
/// * `W` - Any type implementing [`Write`]
pub struct HtmlWriter<W: Write> {
    phase: Phase,
    serializer: HtmlSerializer<W>,
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
            phase: Phase::NotStarted,
            serializer: HtmlSerializer::new(writer, SerializeOpts::default()),
        }
    }

    fn open(&mut self, local: LocalName) -> Result<()> {
        let name = QualName::new(None, ns!(html), local);
        self.serializer
            .start_elem(name, core::iter::empty::<(&QualName, &str)>())?;
        Ok(())
    }

    /// Opens `<pre><code>`, with `class="language-{syntax}"` on `<code>` when
    /// `syntax` names a language.
    fn open_preformatted(&mut self, syntax: Option<&str>) -> Result<()> {
        self.open(local_name!("pre"))?;
        let class = syntax
            .filter(|language| !language.is_empty())
            .map(|language| format!("language-{language}"));
        let class_name = QualName::new(None, ns!(), local_name!("class"));
        let attrs = class.as_deref().map(|value| (&class_name, value));
        let code = QualName::new(None, ns!(html), local_name!("code"));
        self.serializer.start_elem(code, attrs.into_iter())?;
        self.phase = Phase::InPreformatted;
        Ok(())
    }

    /// Closes the `</code></pre>` opened by [`Self::open_preformatted`].
    fn close_preformatted(&mut self) -> Result<()> {
        self.close(local_name!("code"))?;
        self.close(local_name!("pre"))?;
        self.phase = Phase::InBody;
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
            Event::StartDocument { .. } if self.phase == Phase::NotStarted => {
                self.open(local_name!("html"))?;
                self.open(local_name!("body"))?;
                self.phase = Phase::InBody;
            }
            Event::EndDocument => {
                match self.phase {
                    Phase::NotStarted | Phase::Finished => return Ok(()),
                    Phase::InParagraph => self.close(local_name!("p"))?,
                    Phase::InPreformatted => self.close_preformatted()?,
                    Phase::InBody => {}
                }
                self.close(local_name!("body"))?;
                self.close(local_name!("html"))?;
                self.phase = Phase::Finished;
            }
            Event::StartParagraph { .. } if self.phase == Phase::InBody => {
                self.open(local_name!("p"))?;
                self.phase = Phase::InParagraph;
            }
            Event::EndParagraph if self.phase == Phase::InParagraph => {
                self.close(local_name!("p"))?;
                self.phase = Phase::InBody;
            }
            Event::StartPreformatted { syntax, .. } if self.phase == Phase::InBody => {
                self.open_preformatted(syntax.as_deref())?;
            }
            Event::EndPreformatted if self.phase == Phase::InPreformatted => {
                self.close_preformatted()?;
            }
            Event::Text { content }
                if matches!(self.phase, Phase::InParagraph | Phase::InPreformatted) =>
            {
                self.serializer.write_text(&content)?;
            }
            // Inside <pre> a newline is a line break; <br> would not be code text.
            Event::LineBreak | Event::SoftBreak if self.phase == Phase::InPreformatted => {
                self.serializer.write_text("\n")?;
            }
            _ => {
                // Every other event (headings, lists, tables, links, images, text styles,
                // breaks outside a code block, Text outside a paragraph or code block, and
                // a block start while another block is open) is dropped; see the crate docs.
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
        let writer = HtmlWriter::new(&mut buf);
        let _r = docspec_test_utils::try_drive(writer, events);
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

    #[test]
    fn orphan_end_document_produces_empty_output() {
        assert_output([Event::EndDocument], "");
    }
}
