//! HTML to `DocSpec` event stream reader.
//!
//! This crate provides an HTML fragment parser that converts minimal HTML documents
//! (v1: `<p>` tags only) into the `DocSpec` event stream format. It emits `StartParagraph`,
//! `Text`, and `EndParagraph` events for each paragraph element, enabling streaming
//! conversion of HTML content without buffering the entire document.
//! Parsing is delegated to `html5gum`, a WHATWG-compliant HTML tokenizer.

extern crate alloc;

mod parser;

pub use parser::parse_html_fragment;

pub use docspec_core::EventSource;

use alloc::collections::VecDeque;
use docspec_core::{Event, Result};

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

/// Standalone HTML-to-DocSpec event stream reader.
///
/// Wraps [`parse_html_fragment`] with `StartDocument` / `EndDocument` boundaries,
/// implementing [`EventSource`] for use in any `DocSpec` pipeline.
///
/// Only `<p>` elements are recognized in v1; all other HTML is silently dropped.
///
/// # Quick Start
///
/// ```
/// use docspec_html_reader::{HtmlReader, EventSource};
/// use docspec_core::{Event, TextStyle};
///
/// let mut reader = HtmlReader::new("<p>Hi</p>");
/// let mut events = Vec::new();
/// while let Some(ev) = reader.next_event()? {
///     events.push(ev);
/// }
/// assert_eq!(events, vec![
///     Event::StartDocument { id: None, language: None, metadata: None },
///     Event::StartParagraph { alignment: None, id: None },
///     Event::Text { content: "Hi".to_string(), style: TextStyle::default() },
///     Event::EndParagraph,
///     Event::EndDocument,
/// ]);
/// # Ok::<(), docspec_core::Error>(())
/// ```
pub struct HtmlReader<'a> {
    /// Current processing phase.
    phase: Phase,
    /// Queue of pending events to emit.
    queue: VecDeque<Event>,
    /// The HTML source string.
    source: &'a str,
}

impl<'a> HtmlReader<'a> {
    /// Creates a new `HtmlReader` from the given HTML string.
    ///
    /// Parsing is lazy — no events are emitted until [`EventSource::next_event`] is called.
    #[inline]
    #[must_use]
    pub fn new(html: &'a str) -> Self {
        Self {
            phase: Phase::NotStarted,
            queue: VecDeque::new(),
            source: html,
        }
    }
}

impl docspec_core::EventSource for HtmlReader<'_> {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        if self.phase == Phase::NotStarted {
            self.phase = Phase::Running;
            return Ok(Some(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            }));
        }

        if self.phase == Phase::Running && self.queue.is_empty() {
            parser::parse_html_fragment(self.source, &mut self.queue);
            self.queue.push_back(Event::EndDocument);
            self.phase = Phase::Finished;
        }

        if self.phase == Phase::Finished && self.queue.is_empty() {
            return Ok(None);
        }

        Ok(self.queue.pop_front())
    }
}
