//! Outbound event state for the document parser.
//!
//! [`EmitState`] owns the pending event queue and the bookkeeping that decides
//! which events to produce: open inline styles, the open list nesting stack, the
//! per-level list counters, and the deferred preformatted close.
//!
//! It never reads XML. Holding it separately from the token pump is what allows
//! a caller to borrow input and output state simultaneously.

use alloc::collections::VecDeque;
use std::collections::HashMap;

use docspec_core::{Event, ListStyleType, TextStyleKind};

/// One open level on the list nesting stack.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ListStackEntry {
    /// The `w:numId` of the list this entry belongs to.
    pub num_id: u32,
    /// The 0-indexed nesting depth (`w:ilvl`).
    pub ilvl: u32,
    /// Whether this entry's level is ordered (true) or unordered (false).
    pub is_ordered: bool,
}

/// Pending output and the state that governs it.
#[derive(Debug, Default)]
pub(crate) struct EmitState {
    queue: VecDeque<Event>,
    open_styles: Vec<TextStyleKind>,
    list_stack: Vec<ListStackEntry>,
    list_counters: HashMap<(u32, u32), u64>,
    pending_preformatted_close: bool,
}

impl EmitState {
    /// Queues an event for emission.
    pub(crate) fn push(&mut self, event: Event) {
        self.queue.push_back(event);
    }

    /// Removes and returns the next queued event.
    pub(crate) fn pop(&mut self) -> Option<Event> {
        self.queue.pop_front()
    }

    /// Number of events currently queued; also serves as a checkpoint marker
    /// for [`Self::drain_since`].
    pub(crate) fn queued(&self) -> usize {
        self.queue.len()
    }

    /// True when no events are waiting.
    pub(crate) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Moves every event queued since `checkpoint` into `sink`.
    pub(crate) fn drain_since(&mut self, checkpoint: usize, sink: &mut Vec<Event>) {
        sink.extend(self.queue.drain(checkpoint..));
    }

    /// Marks a preformatted block as ended, deferring its close to the next boundary.
    pub(crate) fn defer_preformatted_close(&mut self) {
        self.pending_preformatted_close = true;
    }

    /// True when a preformatted close is awaiting a boundary.
    pub(crate) fn has_pending_preformatted_close(&self) -> bool {
        self.pending_preformatted_close
    }

    /// Drops a deferred preformatted close without emitting it.
    ///
    /// Used when the next paragraph continues the same preformatted block, so the
    /// block stays open and only a line break separates the two paragraphs.
    pub(crate) fn cancel_preformatted_close(&mut self) {
        self.pending_preformatted_close = false;
    }

    pub(crate) fn extend<I: IntoIterator<Item = Event>>(&mut self, events: I) {
        self.queue.extend(events);
    }

    /// Closes every open inline style span.
    pub(crate) fn close_all_styles(&mut self) {
        while self.open_styles.pop().is_some() {
            self.push(Event::EndTextStyle);
        }
    }

    /// Emits a deferred preformatted close, if one is outstanding.
    pub(crate) fn flush_pending_preformatted_close(&mut self) {
        if self.pending_preformatted_close {
            self.push(Event::EndPreformatted);
            self.pending_preformatted_close = false;
        }
    }

    /// Closes every open list level.
    pub(crate) fn flush_list_stack(&mut self) {
        while let Some(entry) = self.list_stack.pop() {
            self.emit_list_item_end(entry.is_ordered);
        }
    }

    /// Increments the counter for `(num_id, ilvl)` and returns the start value to emit.
    ///
    /// Returns `Some(counter)` when this is the first item for this level, or when the
    /// list is resuming after a break (non-sequential). Returns `None` when the item
    /// immediately follows the previous item at the same level with no intervening
    /// non-list content (sequential continuation).
    pub(crate) fn compute_start(
        &mut self,
        num_id: u32,
        ilvl: u32,
        sequential: bool,
    ) -> Option<u64> {
        let counter = self.list_counters.entry((num_id, ilvl)).or_insert(0);
        let is_first = *counter == 0;
        let emitted_counter = counter.saturating_add(1);
        *counter = emitted_counter;
        (is_first || !sequential).then_some(emitted_counter)
    }

    /// Reconciles the list stack for a new item at `(num_id, ilvl)`.
    ///
    /// Returns `true` if a same-level entry was found and popped from the stack
    /// (the new item is a sequential continuation of the previous item at this level).
    /// Returns `false` if the stack had no entry at this level (the list is starting
    /// fresh or resuming after a break).
    pub(crate) fn reconcile_list_stack(
        &mut self,
        num_id: u32,
        ilvl: u32,
        is_ordered: bool,
    ) -> bool {
        let mut found_sequential = false;
        while let Some(top) = self.list_stack.last().copied() {
            match top.ilvl.cmp(&ilvl) {
                core::cmp::Ordering::Greater => {
                    self.list_stack.pop();
                    self.emit_list_item_end(top.is_ordered);
                }
                core::cmp::Ordering::Equal => {
                    self.list_stack.pop();
                    self.emit_list_item_end(top.is_ordered);
                    found_sequential = true;
                    break;
                }
                core::cmp::Ordering::Less => break,
            }
        }

        let target_depth = usize::try_from(ilvl).unwrap_or(usize::MAX);
        while self.list_stack.len() < target_depth {
            let phantom_ilvl = u32::try_from(self.list_stack.len()).unwrap_or(u32::MAX);
            let phantom_style = if is_ordered {
                ListStyleType::Decimal
            } else {
                ListStyleType::Disc
            };
            self.list_stack.push(ListStackEntry {
                num_id,
                ilvl: phantom_ilvl,
                is_ordered,
            });
            if is_ordered {
                self.emit_list_item_start_ordered(num_id, phantom_ilvl, None, phantom_style);
            } else {
                self.emit_list_item_start_unordered(num_id, phantom_ilvl, phantom_style);
            }
        }

        self.list_stack.push(ListStackEntry {
            num_id,
            ilvl,
            is_ordered,
        });
        found_sequential
    }

    /// Emits the start of an ordered list item.
    pub(crate) fn emit_list_item_start_ordered(
        &mut self,
        num_id: u32,
        ilvl: u32,
        start: Option<u64>,
        style_type: ListStyleType,
    ) {
        self.push(Event::StartOrderedListItem {
            id: Some(num_id.to_string()),
            level: ilvl,
            start,
            style_type,
        });
    }

    /// Emits the start of an unordered list item.
    pub(crate) fn emit_list_item_start_unordered(
        &mut self,
        num_id: u32,
        ilvl: u32,
        style_type: ListStyleType,
    ) {
        self.push(Event::StartUnorderedListItem {
            id: Some(num_id.to_string()),
            level: ilvl,
            style_type,
        });
    }

    /// Emits the end of a list item matching the given ordering.
    pub(crate) fn emit_list_item_end(&mut self, is_ordered: bool) {
        self.push(if is_ordered {
            Event::EndOrderedListItem
        } else {
            Event::EndUnorderedListItem
        });
    }

    /// Opens an inline style span unless that kind is already open.
    pub(crate) fn emit_style_if_not_open(&mut self, kind: TextStyleKind) {
        if !self.open_styles.contains(&kind) {
            self.push(Event::StartTextStyle {
                kind: kind.clone(),
                id: None,
            });
            self.open_styles.push(kind);
        }
    }

    /// Closes the given style kinds, innermost first, if they are open.
    pub(crate) fn close_styles(&mut self, kinds: Vec<TextStyleKind>) {
        for kind in kinds.into_iter().rev() {
            if let Some(index) = self.open_styles.iter().rposition(|open| open == &kind) {
                self.open_styles.remove(index);
                self.push(Event::EndTextStyle);
            }
        }
    }
}
