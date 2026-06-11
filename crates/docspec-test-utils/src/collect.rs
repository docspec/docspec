//! Generic event-collection harness for [`EventSource`] readers in tests.
//!
//! Replaces the per-reader `collect_events` / `drive` boilerplate that
//! integration tests across reader crates would otherwise re-implement.

use docspec_core::{Event, EventSource, Result};

/// Drains an [`EventSource`] to completion, returning all emitted events.
///
/// Use [`try_collect_events`] when the test needs to inspect the error
/// rather than panic on it.
///
/// # Panics
///
/// Panics with the underlying error if [`EventSource::next_event`] ever
/// returns `Err`.
#[inline]
pub fn collect_events<R: EventSource>(reader: &mut R) -> Vec<Event> {
    try_collect_events(reader).expect("event source returned an error")
}

/// Drains an [`EventSource`] to completion, propagating any error to the
/// caller.
///
/// # Errors
///
/// Returns the first error produced by [`EventSource::next_event`].
#[inline]
pub fn try_collect_events<R: EventSource>(reader: &mut R) -> Result<Vec<Event>> {
    let mut events = Vec::new();
    while let Some(event) = reader.next_event()? {
        events.push(event);
    }
    Ok(events)
}
