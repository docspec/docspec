#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use crate::{Event, EventSource, Result};

/// A streaming `EventSource` adapter that suppresses empty `Heading`, `BlockQuote`, and
/// `Paragraph` Start/End pairs from the wrapped source.
///
/// The adapter uses a **1-event look-back** (hold-1) algorithm. When it sees a candidate
/// skippable `Start*` event (`StartHeading`, `StartBlockQuote`, or `StartParagraph`), it
/// buffers that event and peeks the next event from the inner source. If the next event is
/// the matching `End*` (i.e., the block is empty), both events are dropped and the adapter
/// keeps draining iteratively until it finds an event to emit. If the next event is something
/// else, the buffered `Start*` is emitted immediately and the "something else" is stashed as
/// `pending` for the next call. Memory is O(1) — at most two `Event` values are held at any
/// time. Stack is O(1) — the implementation is a single `loop` with no recursion, so an
/// arbitrarily long run of empty blocks consumes constant stack regardless of input size.
///
/// # Skip Set
///
/// Exactly three Start/End pairs are matched:
///
/// - `Event::StartHeading { .. }` ↔ `Event::EndHeading`
/// - `Event::StartBlockQuote { .. }` ↔ `Event::EndBlockQuote`
/// - `Event::StartParagraph { .. }` ↔ `Event::EndParagraph`
///
/// No other variants are suppressed. Empty `StartTable`, `StartOrderedListItem`, and all
/// other containers pass through unchanged.
///
/// # Asymmetric API
///
/// `docspec-cli` and `docspec-http` apply this filter automatically by default.
/// Library users opt in by wrapping their `EventSource` explicitly:
/// `SkipEmptyBlocks::new(my_reader)`.
///
/// # Known Limitations
///
/// **No cascading**: an outer container that becomes empty *because* its inner contents were
/// filtered is preserved. Example: `StartBlockQuote → StartParagraph → EndParagraph → EndBlockQuote`
/// produces `StartBlockQuote → EndBlockQuote` (the inner empty paragraph pair is dropped, but the
/// outer block quote is preserved). A subsequent pass would be needed to suppress the outer.
///
/// **Empty table cells**: an empty table cell containing only `StartParagraph → EndParagraph`
/// will have the inner pair dropped, leaving the cell with no child events. The cell itself is
/// preserved (table cells are not in the skip set).
///
/// **Fail-fast**: if the inner source returns `Err` while a `Start*` is buffered, the error
/// propagates immediately and the buffered `Start*` is dropped silently. The stream is considered
/// terminated; no partial recovery is attempted.
///
/// # Example
///
/// ```
/// use docspec_core::{Event, EventSource, Result, SkipEmptyBlocks};
///
/// struct Replay {
///     events: std::vec::IntoIter<Event>,
/// }
/// impl Replay {
///     fn new(events: Vec<Event>) -> Self {
///         Self { events: events.into_iter() }
///     }
/// }
/// impl EventSource for Replay {
///     fn next_event(&mut self) -> Result<Option<Event>> {
///         Ok(self.events.next())
///     }
/// }
///
/// // An empty heading followed by a heading with text:
/// let inner = Replay::new(vec![
///     Event::StartHeading { id: None, level: 1 },
///     Event::EndHeading,
///     Event::StartHeading { id: None, level: 2 },
///     Event::Text { content: String::from("Hello") },
///     Event::EndHeading,
/// ]);
/// let mut filtered = SkipEmptyBlocks::new(inner);
///
/// // The empty H1 is dropped; the H2 with text passes through.
/// assert_eq!(
///     filtered.next_event().unwrap(),
///     Some(Event::StartHeading { id: None, level: 2 }),
/// );
/// assert_eq!(
///     filtered.next_event().unwrap(),
///     Some(Event::Text { content: String::from("Hello") }),
/// );
/// assert_eq!(filtered.next_event().unwrap(), Some(Event::EndHeading));
/// assert_eq!(filtered.next_event().unwrap(), None);
/// ```
pub struct SkipEmptyBlocks<S: EventSource> {
    inner: S,
    /// Holds a candidate skippable `Start*` (`StartHeading`, `StartBlockQuote`, or
    /// `StartParagraph`) while we peek the next event to decide drop-or-flush.
    buffered: Option<Event>,
    /// Holds a non-skippable event that arrived while we were flushing the previous
    /// `buffered` `Start*`. The next call to `next_event` returns this before pulling
    /// from `inner` again.
    pending: Option<Event>,
}

impl<S: EventSource> SkipEmptyBlocks<S> {
    /// Wraps the given source so that empty `Heading`, `BlockQuote`, and `Paragraph`
    /// Start/End pairs are suppressed in the emitted stream.
    #[inline]
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            buffered: None,
            pending: None,
        }
    }
}

impl<S: EventSource> EventSource for SkipEmptyBlocks<S> {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        loop {
            // 1. Drain `pending` first (already decided to emit on a previous call).
            if let Some(pending) = self.pending.take() {
                return Ok(Some(pending));
            }
            // 2. If a buffered `Start*` exists, peek next from inner.
            //    NOTE: `?` propagation here means an `Err` from inner while a
            //    `Start*` is buffered surfaces IMMEDIATELY on this same call; the
            //    buffered `Start*` is dropped. This is intentional and matches the
            //    project's "Fail Fast" principle (MANIFESTO.md).
            if let Some(buffered) = self.buffered.take() {
                match self.inner.next_event()? {
                    Some(next) if is_matching_end(&buffered, &next) => {
                        // Empty block detected: drop BOTH and keep draining iteratively.
                        continue;
                    }
                    Some(next) if is_skippable_start(&next) => {
                        // Emit `buffered` now; the new `Start*` becomes the new buffer.
                        self.buffered = Some(next);
                        return Ok(Some(buffered));
                    }
                    Some(next) => {
                        // Emit `buffered` now; stash `next` as pending for the next call.
                        self.pending = Some(next);
                        return Ok(Some(buffered));
                    }
                    None => {
                        // Truncated stream: emit buffered `Start*`; subsequent call returns None.
                        return Ok(Some(buffered));
                    }
                }
            }
            // 3. No buffer, no pending. Pull from inner.
            match self.inner.next_event()? {
                Some(event) if is_skippable_start(&event) => {
                    self.buffered = Some(event);
                    // Loop iterates: re-enter the buffered branch above to peek the next event.
                }
                other => return Ok(other),
            }
        }
    }
}

// Returns true if `event` is a candidate start event for empty-block suppression.
fn is_skippable_start(event: &Event) -> bool {
    matches!(
        event,
        Event::StartHeading { .. } | Event::StartBlockQuote { .. } | Event::StartParagraph { .. }
    )
}

// Returns true if `end` is the matching close event for the given `start`.
fn is_matching_end(start: &Event, end: &Event) -> bool {
    matches!(
        (start, end),
        (Event::StartHeading { .. }, Event::EndHeading)
            | (Event::StartBlockQuote { .. }, Event::EndBlockQuote)
            | (Event::StartParagraph { .. }, Event::EndParagraph)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, EventSource, Result};

    /// Minimal in-memory `EventSource` for testing.
    struct Replay {
        events: alloc::vec::IntoIter<Event>,
        error_at_end: bool,
    }

    impl Replay {
        fn new(events: alloc::vec::Vec<Event>) -> Self {
            Self {
                events: events.into_iter(),
                error_at_end: false,
            }
        }

        fn with_terminal_error(events: alloc::vec::Vec<Event>) -> Self {
            Self {
                events: events.into_iter(),
                error_at_end: true,
            }
        }
    }

    impl EventSource for Replay {
        fn next_event(&mut self) -> Result<Option<Event>> {
            if let Some(e) = self.events.next() {
                Ok(Some(e))
            } else if self.error_at_end {
                self.error_at_end = false;
                Err(crate::Error::Other {
                    message: "simulated".into(),
                })
            } else {
                Ok(None)
            }
        }
    }

    fn drain<S: EventSource>(mut src: S) -> alloc::vec::Vec<Event> {
        let mut out = alloc::vec::Vec::new();
        while let Some(e) = src.next_event().expect("unexpected error") {
            out.push(e);
        }
        out
    }

    #[test]
    fn empty_heading_is_dropped() {
        let replay = Replay::new(alloc::vec![
            Event::StartHeading { id: None, level: 1 },
            Event::EndHeading,
        ]);
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), alloc::vec![]);
    }

    #[test]
    fn empty_block_quote_is_dropped() {
        let replay = Replay::new(alloc::vec![
            Event::StartBlockQuote { id: None },
            Event::EndBlockQuote,
        ]);
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), alloc::vec![]);
    }

    #[test]
    fn empty_paragraph_is_dropped() {
        let replay = Replay::new(alloc::vec![
            Event::StartParagraph {
                alignment: None,
                id: None
            },
            Event::EndParagraph,
        ]);
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), alloc::vec![]);
    }

    #[test]
    fn non_empty_heading_is_preserved() {
        let input = alloc::vec![
            Event::StartHeading { id: None, level: 2 },
            Event::Text {
                content: "h".into()
            },
            Event::EndHeading,
        ];
        let replay = Replay::new(input.clone());
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), input);
    }

    #[test]
    fn non_empty_paragraph_is_preserved() {
        let input = alloc::vec![
            Event::StartParagraph {
                alignment: None,
                id: None
            },
            Event::Text {
                content: "p".into()
            },
            Event::EndParagraph,
        ];
        let replay = Replay::new(input.clone());
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), input);
    }

    #[test]
    fn non_empty_block_quote_is_preserved() {
        let input = alloc::vec![
            Event::StartBlockQuote { id: None },
            Event::StartParagraph {
                alignment: None,
                id: None
            },
            Event::Text {
                content: "q".into()
            },
            Event::EndParagraph,
            Event::EndBlockQuote,
        ];
        let replay = Replay::new(input.clone());
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), input);
    }

    #[test]
    fn consecutive_empty_headings_all_dropped() {
        let replay = Replay::new(alloc::vec![
            Event::StartHeading { id: None, level: 1 },
            Event::EndHeading,
            Event::StartHeading { id: None, level: 2 },
            Event::EndHeading,
            Event::StartHeading { id: None, level: 3 },
            Event::EndHeading,
        ]);
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), alloc::vec![]);
    }

    #[test]
    fn empty_then_nonempty_heading() {
        let replay = Replay::new(alloc::vec![
            Event::StartHeading { id: None, level: 1 },
            Event::EndHeading,
            Event::StartHeading { id: None, level: 2 },
            Event::Text {
                content: "x".into()
            },
            Event::EndHeading,
        ]);
        assert_eq!(
            drain(SkipEmptyBlocks::new(replay)),
            alloc::vec![
                Event::StartHeading { id: None, level: 2 },
                Event::Text {
                    content: "x".into()
                },
                Event::EndHeading,
            ]
        );
    }

    #[test]
    fn empty_heading_with_id_is_still_dropped() {
        // Proves the `{ .. }` wildcard in is_skippable_start ignores all fields.
        let replay = Replay::new(alloc::vec![
            Event::StartHeading {
                id: Some("anchor".into()),
                level: 1
            },
            Event::EndHeading,
        ]);
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), alloc::vec![]);
    }

    #[test]
    fn empty_paragraph_with_alignment_is_still_dropped() {
        // Proves alignment field is ignored by the wildcard pattern.
        let replay = Replay::new(alloc::vec![
            Event::StartParagraph {
                alignment: Some(crate::TextAlignment::Center),
                id: None
            },
            Event::EndParagraph,
        ]);
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), alloc::vec![]);
    }

    #[test]
    fn nested_empty_blockquote_containing_empty_paragraph_preserves_outer() {
        // Documents the no-cascading limitation: the inner empty paragraph is dropped,
        // but the outer block quote (now empty) is preserved. Lookback-1 is the contract.
        let replay = Replay::new(alloc::vec![
            Event::StartBlockQuote { id: None },
            Event::StartParagraph {
                alignment: None,
                id: None
            },
            Event::EndParagraph,
            Event::EndBlockQuote,
        ]);
        assert_eq!(
            drain(SkipEmptyBlocks::new(replay)),
            alloc::vec![Event::StartBlockQuote { id: None }, Event::EndBlockQuote,]
        );
    }

    #[test]
    fn non_skippable_kinds_pass_through_unchanged() {
        let input = alloc::vec![Event::StartTable { id: None }, Event::EndTable,];
        let replay = Replay::new(input.clone());
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), input);
    }

    #[test]
    fn empty_document_passes_through() {
        // StartDocument and EndDocument are NOT in the skip set.
        let input = alloc::vec![
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None
            },
            Event::EndDocument,
        ];
        let replay = Replay::new(input.clone());
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), input);
    }

    #[test]
    fn error_from_inner_propagates_when_no_buffer() {
        // When there is no buffered Start*, an Err from the inner source propagates directly.
        let replay = Replay::with_terminal_error(alloc::vec![]);
        let mut filter = SkipEmptyBlocks::new(replay);
        assert!(filter.next_event().is_err());
    }

    #[test]
    fn error_while_start_buffered_surfaces_immediately() {
        // Fail-fast contract (MANIFESTO.md): when the inner source returns Err
        // while a Start* is buffered, the error propagates immediately and the
        // buffered Start* is dropped silently. The stream is considered terminated.
        let replay =
            Replay::with_terminal_error(alloc::vec![Event::StartHeading { id: None, level: 1 },]);
        let mut filter = SkipEmptyBlocks::new(replay);
        // First call: buffers StartHeading, calls inner (gets Err), propagates Err.
        assert!(filter.next_event().is_err());
        // Second call: no buffer, no pending, inner also returns Ok(None).
        assert_eq!(filter.next_event().unwrap(), None);
    }

    #[test]
    fn send_sync_compile_assertion() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SkipEmptyBlocks<Replay>>();
    }

    #[test]
    fn many_consecutive_empty_blocks_do_not_blow_stack() {
        // Stack-safety regression: the previous recursive implementation grew the call
        // stack by ~2 frames per empty block, so a long run could overflow Rust's default
        // 8 MiB main-thread / 2 MiB test-thread stack. The iterative `loop` form must
        // drain an arbitrarily long run in O(1) stack. 100_000 empties is well above the
        // overflow threshold of the old code and still completes in milliseconds.
        const N: usize = 100_000;
        let mut events = alloc::vec::Vec::with_capacity(N * 2);
        for _ in 0..N {
            events.push(Event::StartHeading { id: None, level: 1 });
            events.push(Event::EndHeading);
        }
        let replay = Replay::new(events);
        assert_eq!(drain(SkipEmptyBlocks::new(replay)), alloc::vec![]);
    }
}
