//! Owned `MarkdownReader` wrapper using `self_cell` for lifetime-erased storage.

use self_cell::self_cell;

use docspec_core::{Event, EventSource, Result};

/// Type alias required by `self_cell!` macro — the macro appends a lifetime parameter
/// to the dependent type identifier, so we cannot use a fully-qualified path with
/// embedded generics directly.
type InnerMarkdown<'a> = docspec_markdown_reader::MarkdownReader<'a>;

self_cell!(
    /// Owned Markdown reader: holds the input `String` and a `MarkdownReader` that
    /// borrows from it. Constructed via the macro-generated
    /// `MarkdownReaderOwned::new(owner, builder)`.
    pub(crate) struct MarkdownReaderOwned {
        owner: String,
        #[covariant]
        dependent: InnerMarkdown,
    }
);

impl EventSource for MarkdownReaderOwned {
    /// Delegates `next_event` to the inner `MarkdownReader`.
    ///
    /// Uses `with_dependent_mut` — the canonical `self_cell` 1.x API for mutable
    /// access to the dependent value.
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        self.with_dependent_mut(|_owner, reader| reader.next_event())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use docspec_core::Event;

    #[test]
    fn roundtrips_a_heading() {
        let mut reader = MarkdownReaderOwned::new("# Hello".to_string(), |s| {
            docspec_markdown_reader::MarkdownReader::new(s)
        });
        let mut events = Vec::new();
        while let Some(event) = reader.next_event().unwrap() {
            events.push(event);
        }
        // Should contain StartHeading, Text("Hello"), EndHeading, EndDocument
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartHeading { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Text { content, .. } if content == "Hello")));
        assert!(events.iter().any(|e| matches!(e, Event::EndHeading)));
    }

    #[test]
    fn empty_string_emits_only_document_envelope() {
        let mut reader = MarkdownReaderOwned::new(String::new(), |s| {
            docspec_markdown_reader::MarkdownReader::new(s)
        });
        let mut events = Vec::new();
        while let Some(event) = reader.next_event().unwrap() {
            events.push(event);
        }
        // Empty markdown: only StartDocument + EndDocument
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartDocument { .. })));
        assert!(events.iter().any(|e| matches!(e, Event::EndDocument)));
        // No headings, paragraphs, or text
        assert!(!events
            .iter()
            .any(|e| matches!(e, Event::StartHeading { .. })));
        assert!(!events
            .iter()
            .any(|e| matches!(e, Event::StartParagraph { .. })));
    }

    #[test]
    fn wrapper_does_not_strip_bom_internally() {
        // The wrapper must NOT strip the BOM — that is T7's responsibility.
        // With a BOM prefix, pulldown-cmark may or may not emit a heading.
        // We just verify the wrapper doesn't panic and produces some events.
        let mut reader = MarkdownReaderOwned::new("\u{FEFF}# Hello".to_string(), |s| {
            docspec_markdown_reader::MarkdownReader::new(s)
        });
        let mut events = Vec::new();
        while let Some(event) = reader.next_event().unwrap() {
            events.push(event);
        }
        // Must produce at least StartDocument + EndDocument without panicking
        assert!(!events.is_empty());
    }
}
