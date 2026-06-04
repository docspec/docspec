//! Owned `HtmlReader` wrapper using `self_cell` for lifetime-erased storage.

use self_cell::self_cell;

use docspec_core::{Event, EventSource, Result};

/// Type alias required by `self_cell!` macro — the macro appends a lifetime parameter
/// to the dependent type identifier, so we cannot use a fully-qualified path with
/// embedded generics directly.
type InnerHtml<'a> = docspec_html_reader::HtmlReader<'a>;

self_cell!(
    /// Owned HTML reader: holds the input `String` and an `HtmlReader` that
    /// borrows from it. Constructed via the macro-generated
    /// `HtmlReaderOwned::new(owner, builder)`.
    pub(crate) struct HtmlReaderOwned {
        owner: String,
        #[not_covariant]
        dependent: InnerHtml,
    }
);

impl EventSource for HtmlReaderOwned {
    /// Delegates `next_event` to the inner `HtmlReader`.
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
    fn roundtrips_a_paragraph() {
        let mut reader = HtmlReaderOwned::new("<p>hello</p>".to_string(), |s| {
            docspec_html_reader::HtmlReader::new(s)
        });
        let mut events = Vec::new();
        while let Some(event) = reader.next_event().unwrap() {
            events.push(event);
        }
        // Should contain StartParagraph, Text("hello"), EndParagraph, EndDocument
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartParagraph { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Text { content, .. } if content == "hello")));
        assert!(events.iter().any(|e| matches!(e, Event::EndParagraph)));
    }

    #[test]
    fn empty_html_emits_only_document_envelope() {
        let mut reader =
            HtmlReaderOwned::new(String::new(), |s| docspec_html_reader::HtmlReader::new(s));
        let mut events = Vec::new();
        while let Some(event) = reader.next_event().unwrap() {
            events.push(event);
        }
        // Empty HTML: only StartDocument + EndDocument
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::StartDocument { .. })));
        assert!(events.iter().any(|e| matches!(e, Event::EndDocument)));
        // No paragraphs or text
        assert!(!events
            .iter()
            .any(|e| matches!(e, Event::StartParagraph { .. })));
    }

    #[test]
    fn wrapper_does_not_strip_bom_internally() {
        // The wrapper must NOT strip the BOM — that is T7's responsibility.
        let mut reader = HtmlReaderOwned::new("\u{FEFF}<p>hello</p>".to_string(), |s| {
            docspec_html_reader::HtmlReader::new(s)
        });
        let mut events = Vec::new();
        while let Some(event) = reader.next_event().unwrap() {
            events.push(event);
        }
        // Must produce at least StartDocument + EndDocument without panicking
        assert!(!events.is_empty());
    }
}
