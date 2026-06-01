//! Unit tests for `parse_html_fragment`.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::unwrap_used)]

mod helpers {
    #![allow(clippy::single_call_fn)]

    use docspec_core::{Event, TextStyle};

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
    use docspec_core::Event;
    use docspec_html_reader::parse_html_fragment;
    use std::collections::VecDeque;

    fn run(html: &str) -> Vec<Event> {
        let mut q = VecDeque::new();
        parse_html_fragment(html, &mut q);
        q.drain(..).collect()
    }

    #[test]
    fn single_p_emits_paragraph() {
        let events = run("<p>Hello world</p>");
        assert_eq!(
            events,
            vec![
                helpers::start_paragraph(),
                helpers::text("Hello world"),
                Event::EndParagraph,
            ]
        );
    }

    #[test]
    fn empty_p_emits_empty_paragraph() {
        let events = run("<p></p>");
        assert_eq!(
            events,
            vec![helpers::start_paragraph(), Event::EndParagraph,]
        );
    }

    #[test]
    fn attributes_on_p_are_ignored() {
        let events = run("<p class=\"foo\" id=\"bar\">text</p>");
        assert_eq!(
            events,
            vec![
                helpers::start_paragraph(),
                helpers::text("text"),
                Event::EndParagraph,
            ]
        );
    }

    #[test]
    fn uppercase_p_tag_recognized_case_insensitive() {
        let events = run("<P>Hi</P>");
        assert_eq!(
            events,
            vec![
                helpers::start_paragraph(),
                helpers::text("Hi"),
                Event::EndParagraph,
            ]
        );
    }

    #[test]
    fn multiple_p_blocks_emit_sequentially() {
        let events = run("<p>A</p><p>B</p>");
        assert_eq!(
            events,
            vec![
                helpers::start_paragraph(),
                helpers::text("A"),
                Event::EndParagraph,
                helpers::start_paragraph(),
                helpers::text("B"),
                Event::EndParagraph,
            ]
        );
    }

    #[test]
    fn non_p_tags_silently_dropped() {
        let events = run("<div>x</div>");
        assert_eq!(events, vec![]);
    }

    #[test]
    fn inner_tags_stripped_text_preserved() {
        let events = run("<p>Hi <em>there</em></p>");
        assert_eq!(
            events,
            vec![
                helpers::start_paragraph(),
                helpers::text("Hi there"),
                Event::EndParagraph,
            ]
        );
    }

    #[test]
    fn unclosed_p_gracefully_closed_at_eof() {
        let events = run("<p>oops");
        assert_eq!(
            events,
            vec![
                helpers::start_paragraph(),
                helpers::text("oops"),
                Event::EndParagraph,
            ]
        );
    }

    #[test]
    fn nested_p_implicitly_closes_outer() {
        let events = run("<p>outer<p>inner</p></p>");
        assert_eq!(
            events,
            vec![
                helpers::start_paragraph(),
                helpers::text("outer"),
                Event::EndParagraph,
                helpers::start_paragraph(),
                helpers::text("inner"),
                Event::EndParagraph,
            ]
        );
    }

    #[test]
    fn text_outside_p_is_dropped() {
        let events = run("loose text");
        assert_eq!(events, vec![]);
    }

    #[test]
    fn html_entities_are_decoded() {
        let events = run("<p>&amp;</p>");
        assert_eq!(
            events,
            vec![
                helpers::start_paragraph(),
                helpers::text("&"),
                Event::EndParagraph,
            ]
        );
    }

    #[test]
    fn comments_inside_paragraph_are_dropped() {
        let events = run("<p>before<!-- note -->after</p>");
        assert_eq!(
            events,
            vec![
                helpers::start_paragraph(),
                helpers::text("beforeafter"),
                Event::EndParagraph,
            ]
        );
    }

    #[test]
    fn doctype_outside_paragraph_is_dropped() {
        let events = run("<!DOCTYPE html><p>x</p>");
        assert_eq!(
            events,
            vec![
                helpers::start_paragraph(),
                helpers::text("x"),
                Event::EndParagraph,
            ]
        );
    }
}
