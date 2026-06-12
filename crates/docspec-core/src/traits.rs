//! Core traits for event sources, sinks, and asset handles.

/// A self-contained handle to a single embedded asset.
///
/// Returned by readers inside `ImageSource::Asset(Arc<dyn AssetHandle>)`.
/// The handle carries everything needed to resolve content type and stream
/// bytes — no external provider lookup is required.
pub trait AssetHandle: Send + Sync + core::fmt::Debug {
    /// MIME content type (e.g. `"image/png"`). `None` means unknown.
    fn content_type(&self) -> Option<std::borrow::Cow<'_, str>>;

    /// Stream the asset bytes to `writer`. Returns total bytes written.
    ///
    /// # Errors
    /// Returns `io::Error` if the underlying source fails or the asset is
    /// no longer accessible (e.g. ZIP entry vanished, mutex poisoned).
    fn stream_to(&self, writer: &mut dyn std::io::Write) -> std::io::Result<u64>;

    /// Opaque identifier used for `Debug` formatting and `PartialEq` on
    /// `ImageSource::Asset`. Format is reader-defined (e.g. `"zip://word/media/image1.png"`).
    fn asset_id(&self) -> &str;
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
    /// stream is well-formed per the rules in [`crate::event`].
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
