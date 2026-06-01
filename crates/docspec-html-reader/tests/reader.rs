//! Integration tests for `HtmlReader`.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

mod helpers {
    #![allow(clippy::single_call_fn)]

    use docspec_core::{Event, TextStyle};

    /// Returns a `StartDocument` event with all optional fields set to `None`.
    pub fn start_document() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    /// Returns a `StartParagraph` event with no alignment or id.
    pub fn start_paragraph() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    /// Returns a `Text` event with the given content and default style.
    pub fn text(content: &str) -> Event {
        Event::Text {
            content: content.to_string(),
            style: TextStyle::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::helpers;
    use docspec_core::{Event, EventSource as _};
    use docspec_html_reader::HtmlReader;

    fn collect_events(reader: &mut HtmlReader<'_>) -> Vec<Event> {
        let mut events = Vec::new();
        while let Some(ev) = reader.next_event().expect("event") {
            events.push(ev);
        }
        events
    }

    #[test]
    fn empty_input_yields_only_document_boundaries() {
        let mut reader = HtmlReader::new("");
        let events = collect_events(&mut reader);
        assert_eq!(events, vec![helpers::start_document(), Event::EndDocument,]);
    }

    #[test]
    fn single_p_wrapped_in_document() {
        let mut reader = HtmlReader::new("<p>Hi</p>");
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("Hi"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_p_html_yields_only_document_boundaries() {
        let mut reader = HtmlReader::new("<div><span>x</span></div>");
        let events = collect_events(&mut reader);
        assert_eq!(events, vec![helpers::start_document(), Event::EndDocument,]);
    }

    #[test]
    fn multiple_p_wrapped_in_document() {
        let mut reader = HtmlReader::new("<p>A</p><p>B</p>");
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                helpers::start_document(),
                helpers::start_paragraph(),
                helpers::text("A"),
                Event::EndParagraph,
                helpers::start_paragraph(),
                helpers::text("B"),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn next_event_returns_none_after_eof_idempotently() {
        let mut reader = HtmlReader::new("<p>x</p>");
        while reader.next_event().expect("event").is_some() {}
        assert_eq!(reader.next_event().expect("event 1"), None);
        assert_eq!(reader.next_event().expect("event 2"), None);
        assert_eq!(reader.next_event().expect("event 3"), None);
    }
}
