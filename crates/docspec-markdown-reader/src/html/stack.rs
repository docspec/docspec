//! Open-styles stack with overlap normalization.

use docspec_core::{Event, TextStyleKind};

use crate::html::tags::TagIntent;
use crate::html::{MARK_COLOR, MAX_HTML_STYLE_DEPTH};

/// One currently-open inline style and whether it has protected text content.
pub(crate) struct StyleFrame {
    /// The tag intent represented by this style frame.
    intent: TagIntent,
    /// Whether a text event has been emitted since this frame opened.
    text_emitted: bool,
}

/// Stack of open inline styles with deferred starts for non-empty spans.
#[derive(Default)]
pub(crate) struct StyleStack {
    frames: Vec<StyleFrame>,
    deferred_starts: Vec<Event>,
}

impl StyleStack {
    /// Open an inline style if it is not already active and depth allows it.
    pub(crate) fn open(&mut self, intent: TagIntent) -> Vec<Event> {
        if self.frames.iter().any(|frame| frame.intent == intent)
            || self.frames.len() >= MAX_HTML_STYLE_DEPTH
        {
            return Vec::new();
        }

        let start = intent_to_start(&intent);
        self.frames.push(StyleFrame {
            intent,
            text_emitted: false,
        });
        self.deferred_starts.push(start);
        Vec::new()
    }

    /// Close an inline style, normalizing overlaps via close-and-reopen.
    pub(crate) fn close(&mut self, intent: TagIntent) -> Vec<Event> {
        let target = [intent];
        let Some(position) = self.frames.iter().rposition(|frame| {
            target
                .first()
                .is_some_and(|target_intent| frame.intent == *target_intent)
        }) else {
            return Vec::new();
        };
        let Some(after_position) = position.checked_add(1) else {
            return Vec::new();
        };

        if after_position == self.frames.len() {
            let Some(frame) = self.frames.pop() else {
                return Vec::new();
            };
            self.rebuild_deferred_starts();
            return if frame.text_emitted {
                vec![intent_to_end(&frame.intent)]
            } else {
                Vec::new()
            };
        }

        let mut emitted = Vec::new();
        let mut above = self.frames.split_off(after_position);
        for frame in above.iter().rev() {
            if frame.text_emitted {
                emitted.push(intent_to_end(&frame.intent));
            }
        }

        let Some(matched) = self.frames.pop() else {
            self.rebuild_deferred_starts();
            return emitted;
        };
        if matched.text_emitted {
            emitted.push(intent_to_end(&matched.intent));
        }

        for frame in above.drain(..) {
            self.frames.push(StyleFrame {
                intent: frame.intent,
                text_emitted: false,
            });
        }
        self.rebuild_deferred_starts();

        emitted
    }

    /// Mark active styles as having text and release deferred starts before it.
    pub(crate) fn note_text(&mut self) -> Vec<Event> {
        for frame in &mut self.frames {
            frame.text_emitted = true;
        }
        self.deferred_starts.drain(..).collect()
    }

    /// Close every active style from top to bottom, suppressing empty spans.
    pub(crate) fn close_all(&mut self) -> Vec<Event> {
        let mut emitted = Vec::new();
        for frame in self.frames.iter().rev() {
            if frame.text_emitted {
                emitted.push(intent_to_end(&frame.intent));
            }
        }
        self.frames.clear();
        self.deferred_starts.clear();
        emitted
    }

    /// Return true when no open or deferred style state remains.
    pub(crate) fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.deferred_starts.is_empty()
    }

    fn rebuild_deferred_starts(&mut self) {
        self.deferred_starts = self
            .frames
            .iter()
            .filter(|frame| !frame.text_emitted)
            .map(|frame| intent_to_start(&frame.intent))
            .collect();
    }
}

fn intent_to_start(intent: &TagIntent) -> Event {
    let kind = match intent {
        TagIntent::Bold => TextStyleKind::Bold,
        TagIntent::Italic => TextStyleKind::Italic,
        TagIntent::Underline => TextStyleKind::Underline,
        TagIntent::Strikethrough => TextStyleKind::Strikethrough,
        TagIntent::Code
        | TagIntent::LineBreak
        | TagIntent::ThematicBreak
        | TagIntent::Heading(_)
        | TagIntent::Ignored => TextStyleKind::Code,
        TagIntent::Subscript => TextStyleKind::Subscript,
        TagIntent::Superscript => TextStyleKind::Superscript,
        TagIntent::Mark => TextStyleKind::Mark(MARK_COLOR),
    };

    Event::StartTextStyle { kind, id: None }
}

fn intent_to_end(_intent: &TagIntent) -> Event {
    Event::EndTextStyle
}

#[cfg(test)]
mod tests {
    #![allow(clippy::as_conversions, clippy::cast_possible_truncation)]
    use super::*;
    use docspec_core::Color;

    fn start(kind: TextStyleKind) -> Event {
        Event::StartTextStyle { kind, id: None }
    }

    #[test]
    fn open_then_close_with_text() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TagIntent::Bold), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Bold)]);
        assert_eq!(stack.close(TagIntent::Bold), vec![Event::EndTextStyle]);
        assert!(stack.is_empty());
    }

    #[test]
    fn open_then_close_without_text() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TagIntent::Italic), Vec::new());
        assert_eq!(stack.close(TagIntent::Italic), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn same_tag_nesting_idempotent() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TagIntent::Bold), Vec::new());
        assert_eq!(stack.open(TagIntent::Bold), Vec::new());
        assert_eq!(stack.frames.len(), 1);
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Bold)]);
        assert_eq!(stack.close(TagIntent::Bold), vec![Event::EndTextStyle]);
        assert_eq!(stack.close(TagIntent::Bold), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn rule_9_mismatched_closers() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TagIntent::Bold), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Bold)]);
        assert_eq!(stack.open(TagIntent::Italic), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Italic)]);

        assert_eq!(
            stack.close(TagIntent::Bold),
            vec![Event::EndTextStyle, Event::EndTextStyle]
        );
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Italic)]);
        assert_eq!(stack.close(TagIntent::Italic), vec![Event::EndTextStyle]);
        assert!(stack.is_empty());
    }

    #[test]
    fn rule_9_mismatched_closers_no_extra_text() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TagIntent::Bold), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Bold)]);
        assert_eq!(stack.open(TagIntent::Italic), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Italic)]);
        assert_eq!(
            stack.close(TagIntent::Bold),
            vec![Event::EndTextStyle, Event::EndTextStyle]
        );

        assert_eq!(stack.close(TagIntent::Italic), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn depth_bound() {
        let mut stack = StyleStack::default();

        for level in 0..MAX_HTML_STYLE_DEPTH {
            assert_eq!(stack.open(TagIntent::Heading(level as u8)), Vec::new());
        }
        assert_eq!(stack.open(TagIntent::LineBreak), Vec::new());

        assert_eq!(stack.frames.len(), MAX_HTML_STYLE_DEPTH);
        assert_eq!(stack.deferred_starts.len(), MAX_HTML_STYLE_DEPTH);
    }

    #[test]
    fn close_unmatched() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.close(TagIntent::Bold), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn close_all_with_open_frames() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TagIntent::Bold), Vec::new());
        assert_eq!(stack.open(TagIntent::Italic), Vec::new());
        assert_eq!(
            stack.note_text(),
            vec![start(TextStyleKind::Bold), start(TextStyleKind::Italic)]
        );
        assert_eq!(
            stack.close_all(),
            vec![Event::EndTextStyle, Event::EndTextStyle]
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn close_all_with_deferred() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TagIntent::Bold), Vec::new());
        assert_eq!(stack.open(TagIntent::Italic), Vec::new());
        assert_eq!(stack.close_all(), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn mark_uses_constant_color() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TagIntent::Mark), Vec::new());
        assert_eq!(
            stack.note_text(),
            vec![start(TextStyleKind::Mark(Color::Rgb {
                r: 255,
                g: 255,
                b: 0
            }))]
        );
        assert_eq!(stack.close(TagIntent::Mark), vec![Event::EndTextStyle]);
    }

    #[test]
    fn adversarial_repeated_open_close_is_bounded() {
        let mut stack = StyleStack::default();

        for _ in 0..10_000 {
            assert_eq!(stack.open(TagIntent::Bold), Vec::new());
        }
        assert_eq!(stack.frames.len(), 1);

        for _ in 0..10_000 {
            let _ = stack.close(TagIntent::Bold);
        }
        assert!(stack.is_empty());
    }
}
