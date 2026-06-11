//! Generic event-feeding harness for [`EventSink`] writers in tests.
//!
//! Replaces the per-writer `for event in events { sink.handle_event(...)?; }
//! sink.finish()?;` boilerplate that integration tests across writer crates
//! would otherwise re-implement.

use docspec_core::{Event, EventSink, Result};

/// Pushes every event in `events` to `sink`, then consumes `sink` by calling
/// [`EventSink::finish`].
///
/// Use [`try_drive`] when the test needs to inspect the error rather than
/// panic on it.
///
/// # Panics
///
/// Panics with the underlying error if [`EventSink::handle_event`] or
/// [`EventSink::finish`] ever returns `Err`.
#[inline]
pub fn drive<W: EventSink, I: IntoIterator<Item = Event>>(sink: W, events: I) {
    try_drive(sink, events).expect("event sink returned an error");
}

/// Pushes every event in `events` to `sink`, then consumes `sink` by calling
/// [`EventSink::finish`], propagating the first error to the caller.
///
/// # Errors
///
/// Returns the first error produced by [`EventSink::handle_event`] or
/// [`EventSink::finish`].
#[inline]
pub fn try_drive<W: EventSink, I: IntoIterator<Item = Event>>(
    mut sink: W,
    events: I,
) -> Result<()> {
    for event in events {
        sink.handle_event(event)?;
    }
    sink.finish()?;
    Ok(())
}
