//! Stack of open inline text-style spans with deferred starts and
//! overlap normalization.
//!
//! Readers use [`StyleStack`] to translate format-specific inline-style
//! information (HTML tags, OOXML run properties, etc.) into the well-formed
//! [`Event::StartTextStyle`] / [`Event::EndTextStyle`] sequence required by
//! the event protocol. The stack preserves four invariants from the
//! [`event`](crate::event) module docs:
//!
//! - **Rule 9** — `StartTextStyle` spans nest but never overlap. The stack
//!   normalizes overlaps via close-and-reopen.
//! - **Rule 10** — All open style spans close before enclosing block-end
//!   events. Callers invoke [`StyleStack::close_all`] at block boundaries.
//! - **Rule 13** — Empty `StartTextStyle` spans are never emitted. Starts
//!   are deferred at [`StyleStack::open`] and only released by
//!   [`StyleStack::note_text`] when text is about to be written.
//! - The stack is bounded by [`MAX_STYLE_DEPTH`]; opens beyond that depth
//!   are silently ignored so malformed or adversarial input cannot inflate
//!   reader memory.
//!
//! The stack is keyed on [`TextStyleKind`]. Colour-bearing variants such
//! as [`TextStyleKind::Mark`] and [`TextStyleKind::TextColor`] participate
//! in equality by colour, so a run that flips highlight colour produces
//! two distinct style spans, as expected.

use alloc::vec::Vec;

use crate::{Event, TextStyleKind};

/// Maximum number of simultaneously-open inline-style spans.
///
/// Beyond this depth, additional [`StyleStack::open`] calls are silently
/// ignored. This bounds reader memory under adversarial or pathologically
/// nested input. The limit matches the historical HTML-style stack used
/// in the markdown reader.
pub const MAX_STYLE_DEPTH: usize = 32;

/// One open inline style and whether text has been emitted since it opened.
#[derive(Debug, Clone)]
struct StyleFrame {
    kind: TextStyleKind,
    text_emitted: bool,
}

/// Stack of open inline-style spans with deferred starts and overlap
/// normalization.
///
/// Construct with [`StyleStack::default`]. The typical reader loop is:
///
/// 1. On encountering an open-style marker, call [`StyleStack::open`] and
///    enqueue the returned events.
/// 2. Before emitting any [`Event::Text`], call [`StyleStack::note_text`]
///    and enqueue the returned events (deferred `StartTextStyle` events
///    are released here).
/// 3. On encountering a close-style marker, call [`StyleStack::close`].
/// 4. At a block boundary, call [`StyleStack::close_all`] to auto-close
///    any spans still open.
///
/// All four methods return the events the reader should emit, in order,
/// for the current step. The stack itself never emits directly.
#[derive(Debug, Clone, Default)]
pub struct StyleStack {
    frames: Vec<StyleFrame>,
    deferred_starts: Vec<Event>,
}

impl StyleStack {
    /// Opens an inline style if it is not already active and depth allows it.
    ///
    /// The corresponding [`Event::StartTextStyle`] is **deferred** — it is
    /// not returned here. It is released by the next [`StyleStack::note_text`]
    /// call, so an empty styled span (open immediately followed by close
    /// with no intervening text) emits no events. This implements Rule 13.
    ///
    /// Opens are idempotent on `kind`: if a frame with an equal kind is
    /// already on the stack, this call is a no-op and returns an empty
    /// vector. Colour-bearing variants compare by colour, so opening
    /// `Mark(red)` while `Mark(blue)` is open does push a new frame.
    ///
    /// Returns an empty vector in all cases; the signature returns
    /// `Vec<Event>` to match [`StyleStack::close`] and
    /// [`StyleStack::note_text`] for caller uniformity.
    #[inline]
    pub fn open(&mut self, kind: TextStyleKind) -> Vec<Event> {
        if self.frames.iter().any(|frame| frame.kind == kind)
            || self.frames.len() >= MAX_STYLE_DEPTH
        {
            return Vec::new();
        }

        let start = Event::StartTextStyle {
            kind: kind.clone(),
            id: None,
        };
        self.frames.push(StyleFrame {
            kind,
            text_emitted: false,
        });
        self.deferred_starts.push(start);
        Vec::new()
    }

    /// Closes an inline style, normalizing overlaps via close-and-reopen.
    ///
    /// If `kind` is not currently open, the call is a no-op and returns
    /// an empty vector. When the matching frame is below other open
    /// frames in the stack (an overlap such as `<b><i>x</b></i>`), the
    /// frames above it are closed in LIFO order, the target frame is
    /// closed, and the previously-above frames are re-opened with their
    /// starts deferred again. This is the close-and-reopen pattern that
    /// satisfies Rule 9.
    ///
    /// Returns the [`Event::EndTextStyle`] events that the caller should
    /// emit, in order. A frame whose `text_emitted` flag is `false`
    /// contributes no event — its deferred start was never released, so
    /// no matching end is needed (Rule 13).
    #[inline]
    pub fn close(&mut self, kind: &TextStyleKind) -> Vec<Event> {
        let Some(position) = self.frames.iter().rposition(|frame| frame.kind == *kind) else {
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
                alloc::vec![Event::EndTextStyle]
            } else {
                Vec::new()
            };
        }

        let mut emitted = Vec::new();
        let mut above = self.frames.split_off(after_position);
        for frame in above.iter().rev() {
            if frame.text_emitted {
                emitted.push(Event::EndTextStyle);
            }
        }

        let Some(matched) = self.frames.pop() else {
            self.rebuild_deferred_starts();
            return emitted;
        };
        if matched.text_emitted {
            emitted.push(Event::EndTextStyle);
        }

        for frame in above.drain(..) {
            self.frames.push(StyleFrame {
                kind: frame.kind,
                text_emitted: false,
            });
        }
        self.rebuild_deferred_starts();

        emitted
    }

    /// Marks every open frame as having emitted text and releases all
    /// deferred [`Event::StartTextStyle`] events.
    ///
    /// Callers invoke this immediately before emitting an
    /// [`Event::Text`], [`Event::LineBreak`], or any other event that
    /// constitutes "content under the currently open styles". The
    /// returned events must be enqueued in order, before the content
    /// event.
    #[inline]
    pub fn note_text(&mut self) -> Vec<Event> {
        for frame in &mut self.frames {
            frame.text_emitted = true;
        }
        self.deferred_starts.drain(..).collect()
    }

    /// Closes every active style from innermost to outermost, suppressing
    /// frames whose deferred start was never released.
    ///
    /// Used at block boundaries (paragraph end, heading end, run end in
    /// OOXML, etc.) to satisfy Rule 10. After this call the stack is
    /// empty.
    #[inline]
    pub fn close_all(&mut self) -> Vec<Event> {
        let mut emitted = Vec::new();
        for frame in self.frames.iter().rev() {
            if frame.text_emitted {
                emitted.push(Event::EndTextStyle);
            }
        }
        self.frames.clear();
        self.deferred_starts.clear();
        emitted
    }

    /// Returns `true` when no open frames and no deferred starts remain.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.deferred_starts.is_empty()
    }

    fn rebuild_deferred_starts(&mut self) {
        self.deferred_starts = self
            .frames
            .iter()
            .filter(|frame| !frame.text_emitted)
            .map(|frame| Event::StartTextStyle {
                kind: frame.kind.clone(),
                id: None,
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;
    use alloc::vec;

    fn start(kind: TextStyleKind) -> Event {
        Event::StartTextStyle { kind, id: None }
    }

    fn yellow() -> Color {
        Color::Rgb {
            r: 255,
            g: 255,
            b: 0,
        }
    }

    // LOAD-BEARING: changing this constant is a behavior-visible change for
    // every reader that uses StyleStack (markdown HTML translation, docx
    // run-property parsing, etc.).
    #[test]
    fn max_style_depth_is_32() {
        assert_eq!(MAX_STYLE_DEPTH, 32);
    }

    fn blue() -> Color {
        Color::Rgb { r: 0, g: 0, b: 255 }
    }

    fn red() -> Color {
        Color::Rgb { r: 255, g: 0, b: 0 }
    }

    #[test]
    fn open_then_close_with_text() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TextStyleKind::Bold), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Bold)]);
        assert_eq!(stack.close(&TextStyleKind::Bold), vec![Event::EndTextStyle]);
        assert!(stack.is_empty());
    }

    #[test]
    fn open_then_close_without_text_emits_nothing() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TextStyleKind::Italic), Vec::new());
        assert_eq!(stack.close(&TextStyleKind::Italic), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn same_kind_nesting_idempotent() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TextStyleKind::Bold), Vec::new());
        assert_eq!(stack.open(TextStyleKind::Bold), Vec::new());
        assert_eq!(stack.frames.len(), 1);
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Bold)]);
        assert_eq!(stack.close(&TextStyleKind::Bold), vec![Event::EndTextStyle]);
        assert_eq!(stack.close(&TextStyleKind::Bold), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn rule_9_mismatched_closers_with_text() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TextStyleKind::Bold), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Bold)]);
        assert_eq!(stack.open(TextStyleKind::Italic), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Italic)]);

        assert_eq!(
            stack.close(&TextStyleKind::Bold),
            vec![Event::EndTextStyle, Event::EndTextStyle]
        );
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Italic)]);
        assert_eq!(
            stack.close(&TextStyleKind::Italic),
            vec![Event::EndTextStyle]
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn rule_9_mismatched_closers_no_extra_text() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TextStyleKind::Bold), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Bold)]);
        assert_eq!(stack.open(TextStyleKind::Italic), Vec::new());
        assert_eq!(stack.note_text(), vec![start(TextStyleKind::Italic)]);
        assert_eq!(
            stack.close(&TextStyleKind::Bold),
            vec![Event::EndTextStyle, Event::EndTextStyle]
        );

        assert_eq!(stack.close(&TextStyleKind::Italic), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn depth_bound() {
        let mut stack = StyleStack::default();

        // Mark with distinct colours produces distinct kinds, so we can
        // fill the stack with MAX_STYLE_DEPTH frames using one variant.
        for level in 0..MAX_STYLE_DEPTH {
            let level_u8 = u8::try_from(level).unwrap_or(u8::MAX);
            assert_eq!(
                stack.open(TextStyleKind::Mark(Color::Rgb {
                    r: level_u8,
                    g: 0,
                    b: 0,
                })),
                Vec::new()
            );
        }
        // One more open beyond MAX_STYLE_DEPTH is silently ignored.
        assert_eq!(stack.open(TextStyleKind::Bold), Vec::new());

        assert_eq!(stack.frames.len(), MAX_STYLE_DEPTH);
        assert_eq!(stack.deferred_starts.len(), MAX_STYLE_DEPTH);
    }

    #[test]
    fn close_unmatched() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.close(&TextStyleKind::Bold), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn close_all_with_open_frames() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TextStyleKind::Bold), Vec::new());
        assert_eq!(stack.open(TextStyleKind::Italic), Vec::new());
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
    fn close_all_with_deferred_only_emits_nothing() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TextStyleKind::Bold), Vec::new());
        assert_eq!(stack.open(TextStyleKind::Italic), Vec::new());
        assert_eq!(stack.close_all(), Vec::new());
        assert!(stack.is_empty());
    }

    #[test]
    fn mark_with_arbitrary_color_round_trips() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TextStyleKind::Mark(yellow())), Vec::new());
        assert_eq!(
            stack.note_text(),
            vec![start(TextStyleKind::Mark(yellow()))]
        );
        assert_eq!(
            stack.close(&TextStyleKind::Mark(yellow())),
            vec![Event::EndTextStyle]
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn distinct_mark_colors_are_distinct_kinds() {
        // Opening Mark(red) when Mark(blue) is already open must push
        // a second frame, because PartialEq on TextStyleKind compares
        // colour. This is the docx use case: adjacent runs with
        // different highlight colours each get their own span.
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TextStyleKind::Mark(blue())), Vec::new());
        assert_eq!(stack.open(TextStyleKind::Mark(red())), Vec::new());
        assert_eq!(stack.frames.len(), 2);
        assert_eq!(
            stack.note_text(),
            vec![
                start(TextStyleKind::Mark(blue())),
                start(TextStyleKind::Mark(red()))
            ]
        );
        assert_eq!(
            stack.close(&TextStyleKind::Mark(red())),
            vec![Event::EndTextStyle]
        );
        assert_eq!(
            stack.close(&TextStyleKind::Mark(blue())),
            vec![Event::EndTextStyle]
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn text_color_round_trips() {
        let mut stack = StyleStack::default();

        assert_eq!(stack.open(TextStyleKind::TextColor(red())), Vec::new());
        assert_eq!(
            stack.note_text(),
            vec![start(TextStyleKind::TextColor(red()))]
        );
        assert_eq!(
            stack.close(&TextStyleKind::TextColor(red())),
            vec![Event::EndTextStyle]
        );
        assert!(stack.is_empty());
    }

    #[test]
    fn adversarial_repeated_open_close_is_bounded() {
        let mut stack = StyleStack::default();

        for _ in 0..10_000 {
            assert_eq!(stack.open(TextStyleKind::Bold), Vec::new());
        }
        assert_eq!(stack.frames.len(), 1);

        for _ in 0..10_000 {
            let _events = stack.close(&TextStyleKind::Bold);
        }
        assert!(stack.is_empty());
    }
}
