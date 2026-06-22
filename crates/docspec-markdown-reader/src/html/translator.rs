//! HTML token stream → `DocSpec` event translator.

use super::tags::{tag_intent, TagIntent};
use crate::html::tag_intent_to_style_kind;
use docspec_core::{Event, StyleStack};
use html5gum::{Token, Tokenizer};

/// Tokenize an HTML fragment (from pulldown-cmark's `Event::Html` or `Event::InlineHtml` payload).
///
/// Returns an iterator over `html5gum::Token` items. Streaming: html5gum maintains a bounded
/// internal buffer; we do NOT collect tokens into a Vec.
///
/// # Example
///
/// ```ignore
/// let tokens: Vec<_> = tokenize_fragment("<b>hello</b>")
///     .collect::<Result<Vec<_>, _>>()
///     .unwrap();
/// assert_eq!(tokens.len(), 3); // StartTag, String, EndTag
/// ```
pub(crate) fn tokenize_fragment(
    input: &str,
) -> impl Iterator<Item = Result<Token, core::convert::Infallible>> + '_ {
    Tokenizer::new(input)
}

/// Classifies an HTML start-tag byte slice into its semantic intent.
///
/// Delegates to [`tag_intent`] and returns `None` for
/// [`TagIntent::Ignored`] tags. Later tasks extend this function into a
/// full event-emitting translator.
pub(crate) fn classify_start_tag(name: &[u8]) -> Option<TagIntent> {
    match tag_intent(name) {
        TagIntent::Ignored => None,
        intent => Some(intent),
    }
}

/// State for accumulating a multi-line HTML heading whose tags and content
/// arrive as separate `Event::Html` payloads inside a single `HtmlBlock`.
///
/// Streaming-safe: content is forwarded to the event queue as soon as it
/// arrives; only the open-heading "level" is buffered (a single u8).
#[derive(Default)]
pub(crate) struct BlockHeadingAccumulator {
    /// `Some(N)` when `StartHeading { level: N }` has been emitted but
    /// `EndHeading` has not yet been emitted.
    open_level: Option<u8>,
    nested_ignored_heading_depth: usize,
    heading_text_emitted: bool,
}

impl BlockHeadingAccumulator {
    /// Called when a `StartTag` for h1..h6 is seen in an Html payload.
    ///
    /// Returns the `StartHeading` event to emit, or `None` if a heading is
    /// already open (malformed nested heading — silently ignored).
    pub(crate) fn open(&mut self, level: u8) -> Option<Event> {
        if self.open_level.is_some() {
            return None; // nested heading: silently ignored
        }
        self.open_level = Some(level);
        self.nested_ignored_heading_depth = 0;
        self.heading_text_emitted = false;
        Some(Event::StartHeading { id: None, level })
    }

    /// Called when an `EndTag` for h1..h6 is seen in an Html payload.
    ///
    /// Returns `Event::EndHeading` if a heading was open, or `None` if no
    /// heading was open (mismatched close — silently ignored).
    pub(crate) fn close(&mut self) -> Option<Event> {
        let was_open = self.open_level.take().is_some();
        self.nested_ignored_heading_depth = 0;
        self.heading_text_emitted = false;

        was_open.then_some(Event::EndHeading)
    }

    /// Called by the integration layer when the `HtmlBlock` end-tag is observed.
    ///
    /// Auto-closes an unclosed heading, returning `Event::EndHeading` if a
    /// heading was still open, or `None` if the block ended cleanly.
    pub(crate) fn finish_block(&mut self) -> Option<Event> {
        self.close()
    }

    /// Returns `true` if a heading is currently open inside this accumulator.
    pub(crate) fn is_open(&self) -> bool {
        self.open_level.is_some()
    }

    fn enter_nested_ignored_heading(&mut self) {
        self.nested_ignored_heading_depth = self.nested_ignored_heading_depth.saturating_add(1);
    }

    fn exit_nested_ignored_heading(&mut self) {
        self.nested_ignored_heading_depth = self.nested_ignored_heading_depth.saturating_sub(1);
    }

    fn is_inside_nested_ignored_heading(&self) -> bool {
        self.nested_ignored_heading_depth > 0
    }

    fn has_heading_text(&self) -> bool {
        self.heading_text_emitted
    }

    fn note_heading_text(&mut self) {
        self.heading_text_emitted = true;
    }
}

/// Context in which a tag was encountered. Used to gate block-only events
/// (e.g., `<hr>` → `ThematicBreak`) from being emitted mid-paragraph.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HtmlContext {
    /// Tag came from `Event::InlineHtml` — must NOT emit block-level events.
    Inline,
    /// Tag came from `Event::Html` (inside `HtmlBlock`) — block-level events allowed.
    Block,
}

/// Translate a void element (self-closing or implicit-close tag like `<br>`, `<hr>`)
/// to zero or one `DocSpec` event. Returns `None` for out-of-context emissions
/// (e.g., `<hr>` from Inline context).
pub(crate) fn translate_void(intent: &TagIntent, context: HtmlContext) -> Option<Event> {
    match intent {
        TagIntent::LineBreak => Some(Event::LineBreak),
        TagIntent::ThematicBreak => match context {
            HtmlContext::Block => Some(Event::ThematicBreak { id: None }),
            HtmlContext::Inline => None,
        },
        _ => None,
    }
}

/// Translate a single `Event::InlineHtml` fragment (one tag from pulldown-cmark)
/// into zero or more `DocSpec` events.
///
/// `stack` is the caller-owned unified inline-style stack (carries state across
/// consecutive `InlineHtml` events and across markdown-emitted styles in the same
/// paragraph). Same-tag idempotency works because both sources share this stack.
///
/// When `in_preformatted` is true, inline style events are suppressed (Rule 11).
pub(crate) fn translate_inline(
    fragment: &str,
    stack: &mut StyleStack,
    in_preformatted: bool,
) -> Vec<Event> {
    let mut out = Vec::new();
    for token_result in tokenize_fragment(fragment) {
        let token = match token_result {
            Ok(token) => token,
            Err(error) => match error {},
        };
        match token {
            Token::StartTag(tag) => {
                let intent = tag_intent(&tag.name);
                match &intent {
                    TagIntent::Bold
                    | TagIntent::Italic
                    | TagIntent::Underline
                    | TagIntent::Strikethrough
                    | TagIntent::Code
                    | TagIntent::Subscript
                    | TagIntent::Superscript
                    | TagIntent::Mark => {
                        // LOAD-BEARING: Rule 11 (event.rs:24-54) — StartTextStyle MUST NOT nest inside Preformatted.
                        if !in_preformatted {
                            if let Some(kind) = tag_intent_to_style_kind(&intent) {
                                out.extend(stack.open(kind));
                            }
                        }
                    }
                    TagIntent::LineBreak | TagIntent::ThematicBreak => {
                        if let Some(event) = translate_void(&intent, HtmlContext::Inline) {
                            out.push(event);
                        }
                    }
                    TagIntent::Heading(_) | TagIntent::Ignored => {}
                }
            }
            Token::EndTag(tag) => {
                let intent = tag_intent(&tag.name);
                match &intent {
                    TagIntent::Bold
                    | TagIntent::Italic
                    | TagIntent::Underline
                    | TagIntent::Strikethrough
                    | TagIntent::Code
                    | TagIntent::Subscript
                    | TagIntent::Superscript
                    | TagIntent::Mark => {
                        // LOAD-BEARING: Rule 11 (event.rs:24-54) — EndTextStyle MUST NOT emit inside Preformatted.
                        if !in_preformatted {
                            if let Some(kind) = tag_intent_to_style_kind(&intent) {
                                out.extend(stack.close(&kind));
                            }
                        }
                    }
                    TagIntent::LineBreak
                    | TagIntent::ThematicBreak
                    | TagIntent::Heading(_)
                    | TagIntent::Ignored => {}
                }
            }
            Token::String(text) => {
                let content = String::from_utf8_lossy(&text.0).into_owned();
                out.push(Event::Text { content });
            }
            Token::Comment(_) | Token::Doctype(_) | Token::Error(_) => {}
        }
    }
    out
}

/// Translate a single `Event::Html` fragment (one line of block HTML inside `HtmlBlock`)
/// into zero or more `DocSpec` events.
///
/// `heading_acc` accumulates an open heading across multiple HTML payloads within the same
/// `HtmlBlock`. `inline_stack` tracks inline styles inside heading content. When
/// `in_preformatted` is true, inline style events inside headings are suppressed (Rule 11).
pub(crate) fn translate_block(
    fragment: &str,
    heading_acc: &mut BlockHeadingAccumulator,
    inline_stack: &mut StyleStack,
    in_preformatted: bool,
) -> Vec<Event> {
    let mut out = Vec::new();
    for token_result in tokenize_fragment(fragment) {
        let token = match token_result {
            Ok(token) => token,
            Err(error) => match error {},
        };
        match token {
            Token::StartTag(tag) => {
                let intent = tag_intent(&tag.name);
                match &intent {
                    TagIntent::Heading(level) => {
                        if heading_acc.is_open() {
                            heading_acc.enter_nested_ignored_heading();
                            continue;
                        }
                        if let Some(event) = heading_acc.open(*level) {
                            out.push(event);
                        }
                    }
                    TagIntent::Bold
                    | TagIntent::Italic
                    | TagIntent::Underline
                    | TagIntent::Strikethrough
                    | TagIntent::Code
                    | TagIntent::Subscript
                    | TagIntent::Superscript
                    | TagIntent::Mark => {
                        // LOAD-BEARING: Rule 11 (event.rs:24-54) — StartTextStyle MUST NOT nest inside Preformatted.
                        if heading_acc.is_open()
                            && !heading_acc.is_inside_nested_ignored_heading()
                            && !in_preformatted
                        {
                            if let Some(kind) = tag_intent_to_style_kind(&intent) {
                                out.extend(inline_stack.open(kind));
                            }
                        }
                    }
                    TagIntent::LineBreak | TagIntent::ThematicBreak => {
                        if let Some(event) = translate_void(&intent, HtmlContext::Block) {
                            out.push(event);
                        }
                    }
                    TagIntent::Ignored => {}
                }
            }
            Token::EndTag(tag) => {
                let intent = tag_intent(&tag.name);
                match &intent {
                    TagIntent::Heading(_) => {
                        if heading_acc.is_inside_nested_ignored_heading() {
                            heading_acc.exit_nested_ignored_heading();
                        } else {
                            out.extend(inline_stack.close_all());
                            if let Some(event) = heading_acc.close() {
                                out.push(event);
                            }
                        }
                    }
                    TagIntent::Bold
                    | TagIntent::Italic
                    | TagIntent::Underline
                    | TagIntent::Strikethrough
                    | TagIntent::Code
                    | TagIntent::Subscript
                    | TagIntent::Superscript
                    | TagIntent::Mark => {
                        // LOAD-BEARING: Rule 11 (event.rs:24-54) — EndTextStyle MUST NOT emit inside Preformatted.
                        if heading_acc.is_open()
                            && !heading_acc.is_inside_nested_ignored_heading()
                            && !in_preformatted
                        {
                            if let Some(kind) = tag_intent_to_style_kind(&intent) {
                                out.extend(inline_stack.close(&kind));
                            }
                        }
                    }
                    TagIntent::LineBreak | TagIntent::ThematicBreak | TagIntent::Ignored => {}
                }
            }
            Token::String(text) => {
                if heading_acc.is_open() && !heading_acc.is_inside_nested_ignored_heading() {
                    let content = String::from_utf8_lossy(&text.0).into_owned();
                    if !heading_acc.has_heading_text() && content.trim().is_empty() {
                        continue;
                    }
                    heading_acc.note_heading_text();
                    out.extend(inline_stack.note_text());
                    out.push(Event::Text { content });
                }
            }
            Token::Comment(_) | Token::Doctype(_) | Token::Error(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::doc_markdown,
        clippy::indexing_slicing,
        clippy::panic,
        clippy::single_match_else
    )]
    use super::*;
    use crate::html::MARK_COLOR;
    use docspec_core::TextStyleKind;

    fn start(kind: TextStyleKind) -> Event {
        Event::StartTextStyle { kind, id: None }
    }

    // ── tokenize_fragment tests ──────────────────────────────────────────────

    #[test]
    fn tokenize_empty() {
        let tokens: Vec<_> = tokenize_fragment("")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens.len(), 0, "empty input should yield no tokens");
    }

    #[test]
    fn tokenize_start_tag() {
        let tokens: Vec<_> = tokenize_fragment("<b>")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens.len(), 1, "should have exactly one token");
        match &tokens[0] {
            Token::StartTag(tag) => {
                assert_eq!(tag.name, b"b", "tag name should be lowercase 'b'");
            }
            other => panic!("expected StartTag, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_end_tag() {
        let tokens: Vec<_> = tokenize_fragment("</b>")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens.len(), 1, "should have exactly one token");
        match &tokens[0] {
            Token::EndTag(tag) => {
                assert_eq!(tag.name, b"b", "tag name should be lowercase 'b'");
            }
            other => panic!("expected EndTag, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_self_closing() {
        let tokens: Vec<_> = tokenize_fragment("<br/>")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens.len(), 1, "should have exactly one token");
        match &tokens[0] {
            Token::StartTag(tag) => {
                assert_eq!(tag.name, b"br", "tag name should be 'br'");
                assert!(tag.self_closing, "br should be marked self-closing");
            }
            other => panic!("expected StartTag with self_closing=true, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_text_inside_tag() {
        let tokens: Vec<_> = tokenize_fragment("<b>hello</b>")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens.len(), 3, "should have StartTag, String, EndTag");

        match &tokens[0] {
            Token::StartTag(tag) => assert_eq!(tag.name, b"b"),
            other => panic!("expected StartTag, got {other:?}"),
        }

        match &tokens[1] {
            Token::String(s) => {
                let text = String::from_utf8_lossy(&s.0);
                assert_eq!(text, "hello", "text content should be 'hello'");
            }
            other => panic!("expected String, got {other:?}"),
        }

        match &tokens[2] {
            Token::EndTag(tag) => assert_eq!(tag.name, b"b"),
            other => panic!("expected EndTag, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_uppercase_normalized_to_lowercase() {
        let tokens: Vec<_> = tokenize_fragment("<B>X</B>")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens.len(), 3, "should have StartTag, String, EndTag");

        match &tokens[0] {
            Token::StartTag(tag) => {
                assert_eq!(
                    tag.name, b"b",
                    "uppercase B should be normalized to lowercase b"
                );
            }
            other => panic!("expected StartTag, got {other:?}"),
        }

        match &tokens[2] {
            Token::EndTag(tag) => {
                assert_eq!(
                    tag.name, b"b",
                    "uppercase B should be normalized to lowercase b"
                );
            }
            other => panic!("expected EndTag, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_handles_entities() {
        let tokens: Vec<_> = tokenize_fragment("<b>&amp;</b>")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(tokens.len(), 3, "should have StartTag, String, EndTag");

        match &tokens[1] {
            Token::String(s) => {
                let text = String::from_utf8_lossy(&s.0);
                assert_eq!(text, "&", "entity &amp; should be decoded to &");
            }
            other => panic!("expected String, got {other:?}"),
        }
    }

    #[test]
    fn tokenize_malformed_no_panic() {
        let _count = tokenize_fragment("<b oops").count();
    }

    // ── BlockHeadingAccumulator tests ────────────────────────────────────────

    /// open(1) returns StartHeading{level:1}; close() returns EndHeading.
    #[test]
    fn block_heading_single_line_open_close() {
        let mut acc = BlockHeadingAccumulator::default();
        let start = acc.open(1);
        let end = acc.close();
        assert_eq!(
            start,
            Some(Event::StartHeading { id: None, level: 1 }),
            "open(1) should emit StartHeading level 1"
        );
        assert_eq!(
            end,
            Some(Event::EndHeading),
            "close() should emit EndHeading"
        );
    }

    /// open() and close() arriving in separate calls — as in multi-line HTML blocks.
    #[test]
    fn block_heading_multi_line_open_then_close_separately() {
        let mut acc = BlockHeadingAccumulator::default();
        let start = acc.open(3);
        assert_eq!(
            start,
            Some(Event::StartHeading { id: None, level: 3 }),
            "open(3) should return StartHeading level 3"
        );
        assert!(acc.is_open(), "heading should be open after open()");
        let end = acc.close();
        assert_eq!(
            end,
            Some(Event::EndHeading),
            "close() should return EndHeading"
        );
        assert!(!acc.is_open(), "heading should be closed after close()");
    }

    /// A second open() while one is already open is silently ignored.
    #[test]
    fn block_heading_nested_heading_ignored() {
        let mut acc = BlockHeadingAccumulator::default();
        let first = acc.open(1);
        let second = acc.open(2);
        assert_eq!(
            first,
            Some(Event::StartHeading { id: None, level: 1 }),
            "first open should succeed"
        );
        assert_eq!(
            second, None,
            "nested heading open should be silently ignored"
        );
        // The original heading level 1 is still tracked
        let end = acc.close();
        assert_eq!(
            end,
            Some(Event::EndHeading),
            "close should end the first heading"
        );
    }

    /// close() with no heading open returns None without panicking.
    #[test]
    fn block_heading_close_without_open() {
        let mut acc = BlockHeadingAccumulator::default();
        let result = acc.close();
        assert_eq!(
            result, None,
            "close() with no open heading should return None"
        );
    }

    /// finish_block() auto-closes an unclosed heading.
    #[test]
    fn block_heading_finish_block_auto_closes() {
        let mut acc = BlockHeadingAccumulator::default();
        acc.open(2);
        let result = acc.finish_block();
        assert_eq!(
            result,
            Some(Event::EndHeading),
            "finish_block() should auto-close an open heading"
        );
        assert!(
            !acc.is_open(),
            "heading should be closed after finish_block()"
        );
    }

    /// finish_block() with no open heading returns None.
    #[test]
    fn block_heading_finish_block_when_clean() {
        let mut acc = BlockHeadingAccumulator::default();
        let result = acc.finish_block();
        assert_eq!(
            result, None,
            "finish_block() with no open heading should return None"
        );
    }

    // ── translate_void tests ─────────────────────────────────────────────────

    /// LineBreak intent in Inline context emits LineBreak.
    #[test]
    fn translate_void_br_in_inline_emits_linebreak() {
        let result = translate_void(&TagIntent::LineBreak, HtmlContext::Inline);
        assert_eq!(
            result,
            Some(Event::LineBreak),
            "LineBreak in Inline context should emit LineBreak"
        );
    }

    /// LineBreak intent in Block context emits LineBreak.
    #[test]
    fn translate_void_br_in_block_emits_linebreak() {
        let result = translate_void(&TagIntent::LineBreak, HtmlContext::Block);
        assert_eq!(
            result,
            Some(Event::LineBreak),
            "LineBreak in Block context should emit LineBreak"
        );
    }

    /// ThematicBreak intent in Block context emits ThematicBreak.
    #[test]
    fn translate_void_hr_in_block_emits_thematic_break() {
        let result = translate_void(&TagIntent::ThematicBreak, HtmlContext::Block);
        assert_eq!(
            result,
            Some(Event::ThematicBreak { id: None }),
            "ThematicBreak in Block context should emit ThematicBreak"
        );
    }

    /// ThematicBreak intent in Inline context returns None.
    #[test]
    fn translate_void_hr_in_inline_returns_none() {
        let result = translate_void(&TagIntent::ThematicBreak, HtmlContext::Inline);
        assert_eq!(
            result, None,
            "ThematicBreak in Inline context should return None"
        );
    }

    /// Non-void intent (Bold) returns None.
    #[test]
    fn translate_void_non_void_intent_returns_none() {
        let result = translate_void(&TagIntent::Bold, HtmlContext::Block);
        assert_eq!(
            result, None,
            "Bold intent should return None (not a void element)"
        );
    }

    /// Heading intent returns None.
    #[test]
    fn translate_void_heading_intent_returns_none() {
        let result = translate_void(&TagIntent::Heading(2), HtmlContext::Block);
        assert_eq!(
            result, None,
            "Heading intent should return None (not a void element)"
        );
    }

    // ── translate_inline tests ───────────────────────────────────────────────

    #[test]
    fn translate_inline_bold_open_close_with_text() {
        let mut stack = StyleStack::default();

        assert_eq!(translate_inline("<b>", &mut stack, false), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Bold)]);
        assert_eq!(
            translate_inline("</b>", &mut stack, false),
            vec![Event::EndTextStyle]
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn translate_inline_italic_open_close() {
        let mut stack = StyleStack::default();

        assert_eq!(translate_inline("<em>", &mut stack, false), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Italic)]);
        assert_eq!(
            translate_inline("</em>", &mut stack, false),
            vec![Event::EndTextStyle]
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn translate_inline_bold_no_intervening_text() {
        let mut stack = StyleStack::default();

        assert_eq!(translate_inline("<b>", &mut stack, false), Vec::new());
        assert_eq!(translate_inline("</b>", &mut stack, false), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn translate_inline_hr_dropped_in_inline_context() {
        let mut stack = StyleStack::default();

        assert_eq!(translate_inline("<hr>", &mut stack, false), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn translate_inline_heading_dropped_in_inline_context() {
        let mut stack = StyleStack::default();

        assert_eq!(translate_inline("<h1>", &mut stack, false), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn translate_inline_br_emits_linebreak() {
        let mut stack = StyleStack::default();

        assert_eq!(
            translate_inline("<br>", &mut stack, false),
            vec![Event::LineBreak]
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn translate_inline_br_self_closing() {
        let mut stack = StyleStack::default();

        assert_eq!(
            translate_inline("<br/>", &mut stack, false),
            vec![Event::LineBreak]
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn translate_inline_uppercase_tag_normalized() {
        let mut stack = StyleStack::default();

        assert_eq!(translate_inline("<B>", &mut stack, false), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Bold)]);
        assert_eq!(
            translate_inline("</B>", &mut stack, false),
            vec![Event::EndTextStyle]
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn translate_inline_unknown_tag_silently_dropped() {
        let mut stack = StyleStack::default();

        assert_eq!(translate_inline("<div>", &mut stack, false), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn translate_inline_malformed_no_panic() {
        let mut stack = StyleStack::default();

        assert_eq!(translate_inline("<b oops", &mut stack, false), Vec::new());
    }

    #[test]
    fn translate_inline_mark_uses_constant_color() {
        let mut stack = StyleStack::default();

        assert_eq!(translate_inline("<mark>", &mut stack, false), Vec::new());
        assert_eq!(
            stack.note_text(),
            vec![start(TextStyleKind::Mark(MARK_COLOR))]
        );
        assert_eq!(
            translate_inline("</mark>", &mut stack, false),
            vec![Event::EndTextStyle]
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn translate_inline_preformatted_suppresses_styles() {
        let mut stack = StyleStack::default();

        assert_eq!(translate_inline("<b>", &mut stack, true), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn translate_inline_styles_suppressed_in_preformatted() {
        let mut stack = StyleStack::default();
        let events = translate_inline("<b>text</b>", &mut stack, true);
        assert_eq!(
            events,
            vec![Event::Text {
                content: "text".to_owned()
            }],
            "open and close style tags suppressed; text preserved"
        );
    }

    #[test]
    fn translate_inline_text_passthrough_in_preformatted() {
        let mut stack = StyleStack::default();
        let events = translate_inline("<em>hello world</em>", &mut stack, true);
        assert_eq!(
            events,
            vec![Event::Text {
                content: "hello world".to_owned()
            }],
            "text inside a style tag is preserved even when in_preformatted=true"
        );
    }

    #[test]
    fn translate_inline_linebreak_emitted_in_preformatted() {
        let mut stack = StyleStack::default();
        let events = translate_inline("<br>", &mut stack, true);
        assert_eq!(
            events,
            vec![Event::LineBreak],
            "LineBreak is a void element unaffected by Rule 11; emitted even in preformatted"
        );
    }

    #[test]
    fn translate_inline_stack_state_unchanged_when_suppressed() {
        let mut stack = StyleStack::default();
        translate_inline("<b>", &mut stack, true);
        assert!(
            stack.is_empty(),
            "stack must not be modified when in_preformatted=true"
        );
    }

    #[test]
    fn translate_inline_non_preformatted_still_emits_styles() {
        let mut stack = StyleStack::default();
        let open_events = translate_inline("<b>", &mut stack, false);
        assert_eq!(
            open_events,
            Vec::new(),
            "open returns nothing (deferred until text)"
        );
        let start_events = stack.note_text();
        assert_eq!(
            start_events,
            vec![start(TextStyleKind::Bold)],
            "regression: normal mode (in_preformatted=false) still defers and emits StartTextStyle"
        );
        let close_events = translate_inline("</b>", &mut stack, false);
        assert_eq!(
            close_events,
            vec![Event::EndTextStyle],
            "regression: close emits EndTextStyle"
        );
        assert!(stack.is_empty());
    }

    // ── translate_block tests ────────────────────────────────────────────────

    fn translate_block_with_default_state(fragment: &str) -> Vec<Event> {
        let mut heading_acc = BlockHeadingAccumulator::default();
        let mut inline_stack = StyleStack::default();

        translate_block(fragment, &mut heading_acc, &mut inline_stack, false)
    }

    #[test]
    fn translate_block_single_line_h1() {
        assert_eq!(
            translate_block_with_default_state("<h1>Title</h1>"),
            vec![
                Event::StartHeading { id: None, level: 1 },
                Event::Text {
                    content: "Title".to_owned()
                },
                Event::EndHeading
            ]
        );
    }

    #[test]
    fn translate_block_single_line_h6() {
        assert_eq!(
            translate_block_with_default_state("<h6>Title</h6>"),
            vec![
                Event::StartHeading { id: None, level: 6 },
                Event::Text {
                    content: "Title".to_owned()
                },
                Event::EndHeading
            ]
        );
    }

    #[test]
    fn translate_block_h7_dropped() {
        assert_eq!(translate_block_with_default_state("<h7>X</h7>"), Vec::new());
    }

    #[test]
    fn translate_block_multi_line_open_then_close() {
        let mut heading_acc = BlockHeadingAccumulator::default();
        let mut inline_stack = StyleStack::default();

        assert_eq!(
            translate_block("<h1>\n", &mut heading_acc, &mut inline_stack, false),
            vec![Event::StartHeading { id: None, level: 1 }]
        );
        assert_eq!(
            translate_block("  Title\n", &mut heading_acc, &mut inline_stack, false),
            vec![Event::Text {
                content: "  Title\n".to_owned()
            }]
        );
        assert_eq!(
            translate_block("</h1>\n", &mut heading_acc, &mut inline_stack, false),
            vec![Event::EndHeading]
        );
    }

    #[test]
    fn translate_block_nested_inline_inside_heading() {
        assert_eq!(
            translate_block_with_default_state("<h1><b>Bold Title</b></h1>"),
            vec![
                Event::StartHeading { id: None, level: 1 },
                start(TextStyleKind::Bold),
                Event::Text {
                    content: "Bold Title".to_owned()
                },
                Event::EndTextStyle,
                Event::EndHeading
            ]
        );
    }

    #[test]
    fn translate_block_inline_styles_auto_closed_on_heading_end() {
        assert_eq!(
            translate_block_with_default_state("<h1><b>oops</h1>"),
            vec![
                Event::StartHeading { id: None, level: 1 },
                start(TextStyleKind::Bold),
                Event::Text {
                    content: "oops".to_owned()
                },
                Event::EndTextStyle,
                Event::EndHeading
            ]
        );
    }

    #[test]
    fn translate_block_hr_in_block_context_emits_thematic_break() {
        assert_eq!(
            translate_block_with_default_state("<hr>"),
            vec![Event::ThematicBreak { id: None }]
        );
    }

    #[test]
    fn translate_block_br_in_heading_emits_linebreak() {
        assert_eq!(
            translate_block_with_default_state("<h1>line1<br/>line2</h1>"),
            vec![
                Event::StartHeading { id: None, level: 1 },
                Event::Text {
                    content: "line1".to_owned()
                },
                Event::LineBreak,
                Event::Text {
                    content: "line2".to_owned()
                },
                Event::EndHeading
            ]
        );
    }

    #[test]
    fn translate_block_text_outside_heading_dropped() {
        assert_eq!(translate_block_with_default_state("some text"), Vec::new());
    }

    #[test]
    fn translate_block_out_of_scope_tag_dropped() {
        assert_eq!(
            translate_block_with_default_state("<table>x</table>"),
            Vec::new()
        );
    }

    #[test]
    fn translate_block_malformed_no_panic() {
        let mut heading_acc = BlockHeadingAccumulator::default();
        let mut inline_stack = StyleStack::default();

        let _events = translate_block("<h1 oops", &mut heading_acc, &mut inline_stack, false);
    }

    #[test]
    fn translate_block_nested_heading_inner_ignored() {
        assert_eq!(
            translate_block_with_default_state("<h1>outer<h2>inner</h2></h1>"),
            vec![
                Event::StartHeading { id: None, level: 1 },
                Event::Text {
                    content: "outer".to_owned()
                },
                Event::EndHeading
            ]
        );
    }

    #[test]
    fn translate_block_suppresses_inline_in_pre() {
        let mut heading_acc = BlockHeadingAccumulator::default();
        let mut inline_stack = StyleStack::default();
        let events = translate_block(
            "<h1><b>text</b></h1>",
            &mut heading_acc,
            &mut inline_stack,
            true,
        );
        assert_eq!(
            events,
            vec![
                Event::StartHeading { id: None, level: 1 },
                Event::Text {
                    content: "text".to_owned()
                },
                Event::EndHeading,
            ],
            "Rule 11: Bold events suppressed inside preformatted; text and heading structure preserved"
        );
    }
}
