//! Event builders for terse test assertions.

use docspec_core::Event;

/// Returns a vanilla `StartDocument` event with no id, language, or metadata.
#[inline]
#[must_use]
pub fn start_document() -> Event {
    Event::StartDocument {
        id: None,
        language: None,
        metadata: None,
    }
}

/// Returns a vanilla `StartParagraph` event with no alignment or id.
#[inline]
#[must_use]
pub fn start_paragraph() -> Event {
    Event::StartParagraph {
        alignment: None,
        id: None,
    }
}

/// Returns a `Text` event with the given content.
#[inline]
#[must_use]
pub fn text(content: &str) -> Event {
    Event::Text {
        content: content.to_string(),
    }
}
