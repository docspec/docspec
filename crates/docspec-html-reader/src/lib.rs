//! HTML to `DocSpec` event stream reader.
//!
//! This crate provides an [`HtmlReader`] that implements [`EventSource`] to convert
//! HTML documents into the `DocSpec` event stream format. It uses `html5gum`
//! to parse HTML5-compliant markup and emits typed events representing document
//! structure.
//!
//! # Quick Start
//!
//! ```
//! use docspec_html_reader::{HtmlReader, EventSource};
//!
//! let html = "<p>Hello world</p>";
//! let mut reader = HtmlReader::new(html);
//!
//! while let Some(event) = reader.next_event()? {
//!     println!("{event:?}");
//! }
//! # Ok::<(), docspec_core::Error>(())
//! ```
//!
//! # Supported Elements
//!
//! - Paragraphs → `StartParagraph` / `EndParagraph`
//!
//! # Unsupported Elements
//!
//! All other HTML elements are silently ignored. Text content inside inline
//! elements (e.g., `<strong>`, `<em>`) is preserved as `Text` events, but
//! the formatting structure is dropped.

extern crate alloc;

use alloc::collections::VecDeque;

pub use docspec_core::EventSource;
use docspec_core::{Event, Result};
use html5gum::{StringReader, Tokenizer};

/// Document processing phase.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// `EndDocument` has been emitted.
    Finished,
    /// `StartDocument` not yet emitted.
    NotStarted,
    /// Processing events between `StartDocument` and `EndDocument`.
    Running,
}

/// A streaming HTML reader that implements [`EventSource`].
///
/// `HtmlReader` parses HTML using `html5gum` and emits `DocSpec` events
/// one at a time. Only `<p>` paragraph elements are recognized; all other
/// elements are silently ignored.
///
/// # Example
///
/// ```
/// use docspec_html_reader::{HtmlReader, EventSource};
///
/// let mut reader = HtmlReader::new("<p>hello</p>");
/// while let Some(event) = reader.next_event()? {
///     // Process events...
/// }
/// # Ok::<(), docspec_core::Error>(())
/// ```
pub struct HtmlReader<'a> {
    /// Whether the reader is currently inside a `<p>` element.
    in_paragraph: bool,
    /// Document processing phase.
    phase: Phase,
    /// Queue of `DocSpec` events to emit.
    queue: VecDeque<Event>,
    /// The html5gum tokenizer iterator.
    tokens: Tokenizer<StringReader<'a>>,
}

impl<'a> HtmlReader<'a> {
    /// Pops the front event from the queue, if any.
    fn drain_queue(&mut self) -> Option<Event> {
        self.queue.pop_front()
    }

    /// Translates an end tag token into queued events.
    fn handle_end_tag(&mut self, tag: &html5gum::EndTag<()>) {
        if &*tag.name == b"p" && self.in_paragraph {
            self.queue.push_back(Event::EndParagraph);
            self.in_paragraph = false;
        }
        // Orphan </p> or non-p end tags: silently ignore
    }

    /// Handles end-of-input: auto-closes any open paragraph and emits `EndDocument`.
    fn handle_eof(&mut self) {
        if self.in_paragraph {
            self.queue.push_back(Event::EndParagraph);
            self.in_paragraph = false;
        }
        self.queue.push_back(Event::EndDocument);
        self.phase = Phase::Finished;
    }

    /// Translates a start tag token into queued events.
    fn handle_start_tag(&mut self, tag: &html5gum::StartTag<()>) {
        if &*tag.name != b"p" || self.in_paragraph {
            // Nested <p> while already in paragraph: silently ignore (including self-closing nested)
            // All other tags: silently ignore
            return;
        }

        self.queue.push_back(Event::StartParagraph {
            alignment: None,
            id: None,
        });
        self.in_paragraph = true;
        if tag.self_closing {
            self.queue.push_back(Event::EndParagraph);
            self.in_paragraph = false;
        }
    }

    /// Translates a text token into a queued event.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the text bytes are not valid UTF-8.
    fn handle_text(&mut self, text_bytes: &[u8]) -> Result<()> {
        if self.in_paragraph {
            let text =
                core::str::from_utf8(text_bytes).map_err(|e| docspec_core::Error::Parse {
                    message: format!("invalid UTF-8 in HTML text: {e}"),
                    position: None,
                })?;
            self.queue.push_back(Event::Text {
                content: text.to_string(),
            });
        }
        Ok(())
    }

    /// Creates a new `HtmlReader` from the given HTML string.
    ///
    /// The reader will emit `StartDocument` as its first event and `EndDocument`
    /// as its last event, with the parsed content events in between.
    ///
    /// # Example
    ///
    /// ```
    /// use docspec_html_reader::HtmlReader;
    ///
    /// let reader = HtmlReader::new("<p>Hello World</p>");
    /// ```
    #[inline]
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            in_paragraph: false,
            phase: Phase::NotStarted,
            queue: VecDeque::new(),
            tokens: Tokenizer::new(input),
        }
    }
}

impl EventSource for HtmlReader<'_> {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        loop {
            if let Some(event) = self.drain_queue() {
                return Ok(Some(event));
            }

            match self.phase {
                Phase::NotStarted => {
                    self.phase = Phase::Running;
                    self.queue.push_back(Event::StartDocument {
                        id: None,
                        language: None,
                        metadata: None,
                    });
                }
                Phase::Finished => {
                    return Ok(None);
                }
                Phase::Running => {
                    let Some(result) = self.tokens.next() else {
                        self.handle_eof();
                        continue;
                    };
                    match result {
                        Ok(token) => match token {
                            html5gum::Token::StartTag(tag) => {
                                self.handle_start_tag(&tag);
                            }
                            html5gum::Token::EndTag(tag) => {
                                self.handle_end_tag(&tag);
                            }
                            html5gum::Token::String(spanned) => {
                                self.handle_text(&spanned.value.0)?;
                            }
                            html5gum::Token::Comment(_) | html5gum::Token::Doctype(_) => {
                                // Silently ignore
                            }
                            html5gum::Token::Error(spanned) => {
                                return Err(docspec_core::Error::Parse {
                                    message: format!("html5gum: {:?}", spanned.value),
                                    position: None,
                                });
                            }
                        },
                        Err(infallible) => match infallible {},
                    }
                }
            }
        }
    }
}
