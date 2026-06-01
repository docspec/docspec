//! Internal HTML fragment parser backed by `html5gum`'s WHATWG-compliant tokenizer.
//! It recognizes `<p>` start/end tags and silently drops every other tag, comment,
//! doctype, and tokenizer error.

use alloc::collections::VecDeque;
use alloc::string::String;

use docspec_core::{Event, TextStyle};
use html5gum::{Token, Tokenizer};

/// Parses an HTML fragment and appends paragraph events to `queue`.
///
/// The `html` parameter is tokenized once with `html5gum`, and `queue` receives
/// `StartParagraph`, `Text`, and `EndParagraph` events for recognized `<p>` elements.
/// Parsing is lenient: stray closing tags are dropped, nested opening paragraph tags
/// implicitly close the current paragraph, tokenizer errors are skipped, and an
/// unclosed paragraph is closed at end-of-input.
///
/// HTML entities in paragraph text are decoded by the tokenizer. Non-`<p>` tags,
/// comments, doctypes, and text outside paragraphs are ignored.
///
/// # Examples
///
/// ```
/// use std::collections::VecDeque;
/// use docspec_html_reader::parse_html_fragment;
/// use docspec_core::{Event, TextStyle};
///
/// let mut queue = VecDeque::new();
/// parse_html_fragment("<p>Hello world</p>", &mut queue);
/// let events: Vec<Event> = queue.drain(..).collect();
/// assert_eq!(events, vec![
///     Event::StartParagraph { alignment: None, id: None },
///     Event::Text { content: "Hello world".to_string(), style: TextStyle::default() },
///     Event::EndParagraph,
/// ]);
/// ```
#[inline]
pub fn parse_html_fragment(html: &str, queue: &mut VecDeque<Event>) {
    let mut paragraph_text: Option<String> = None;

    for token in Tokenizer::new(html).flatten() {
        match token {
            Token::StartTag(tag) if tag.name.as_slice() == b"p" => {
                close_paragraph(&mut paragraph_text, queue);
                queue.push_back(Event::StartParagraph {
                    alignment: None,
                    id: None,
                });
                paragraph_text = Some(String::new());
            }
            Token::EndTag(tag) if tag.name.as_slice() == b"p" => {
                close_paragraph(&mut paragraph_text, queue);
            }
            Token::String(content) => {
                if let Some(buf) = paragraph_text.as_mut() {
                    buf.push_str(&String::from_utf8_lossy(content.as_slice()));
                }
            }
            Token::StartTag(_)
            | Token::EndTag(_)
            | Token::Comment(_)
            | Token::Doctype(_)
            | Token::Error(_) => {}
        }
    }

    close_paragraph(&mut paragraph_text, queue);
}

#[inline]
fn close_paragraph(state: &mut Option<String>, queue: &mut VecDeque<Event>) {
    let Some(buf) = state.take() else {
        return;
    };

    if !buf.is_empty() {
        queue.push_back(Event::Text {
            content: buf,
            style: TextStyle::default(),
        });
    }
    queue.push_back(Event::EndParagraph);
}
