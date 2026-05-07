//! Core traits for event sources, sinks, and asset providers.

use alloc::borrow::Cow;
use std::io;
use std::io::Write;

/// Provides access to binary assets referenced in the event stream.
///
/// Readers register assets as they are encountered. Writers call [`AssetProvider::stream_to`]
/// on demand — bytes stream through, never buffer. Assets must remain accessible until
/// the `EndDocument` event is processed.
pub trait AssetProvider: Send + Sync {
    /// Returns the MIME content type for the given asset ID, or `None` if the asset is not found.
    fn content_type(&self, asset_id: &str) -> Option<Cow<'_, str>>;

    /// Streams the asset bytes to the given writer.
    ///
    /// Returns `Some(Ok(bytes_written))` on success, `Some(Err(_))` on write error,
    /// or `None` if the asset is not found.
    fn stream_to(&self, asset_id: &str, writer: &mut dyn Write) -> Option<io::Result<u64>>;
}

/// Consumes a stream of [`crate::Event`]s to produce output.
///
/// Writers implement this trait to translate document events into a target
/// format. Call [`EventSink::handle_event`] for each event in order, then
/// call [`EventSink::finish`] to flush output and signal completion.
///
/// `finish` consumes `self` to prevent reuse after the stream has ended.
pub trait EventSink {
    /// Flush any buffered output and finalize the document.
    ///
    /// Consumes `self` to prevent further use after the stream ends.
    ///
    /// # Errors
    ///
    /// Returns an error if flushing or finalization fails.
    fn finish(self) -> crate::Result<()>;

    /// Process one event from the stream.
    ///
    /// Events must arrive in valid document order. Writers may assume the
    /// stream is well-formed per the rules in EVENTS.md.
    ///
    /// # Errors
    ///
    /// Returns an error if the sink cannot process the event (write failure,
    /// invalid format, resource exhaustion).
    fn handle_event(&mut self, event: crate::Event) -> crate::Result<()>;
}

/// Produces a stream of [`crate::Event`]s from a document source.
///
/// The pull-based design gives the consumer control: only fetch events when
/// ready to process them. This provides natural backpressure and constant
/// memory usage regardless of document size.
///
/// Return `Ok(None)` to signal the end of the stream. Return `Err` for fatal
/// errors that prevent further reading.
pub trait EventSource {
    /// Returns the next event from the stream, or `None` if the stream has ended.
    ///
    /// # Errors
    ///
    /// Returns an error if the source encounters a fatal problem (malformed
    /// input, truncated stream, I/O failure). After an error, the stream is
    /// considered terminated.
    fn next_event(&mut self) -> crate::Result<Option<crate::Event>>;
}
