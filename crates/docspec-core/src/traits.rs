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

#[cfg(test)]
mod tests {
    mod event_source_sink_tests {
        use super::*;

        /// Mock `EventSink` that collects events.
        struct MockEventSink {
            events: Vec<crate::Event>,
        }

        impl MockEventSink {
            fn new() -> Self {
                Self { events: Vec::new() }
            }
        }

        impl EventSink for MockEventSink {
            fn finish(self) -> crate::Result<()> {
                Ok(())
            }

            fn handle_event(&mut self, event: crate::Event) -> crate::Result<()> {
                self.events.push(event);
                Ok(())
            }
        }

        /// Mock `EventSource` that emits a fixed sequence of events.
        struct MockEventSource {
            events: Vec<crate::Event>,
            index: usize,
        }

        impl MockEventSource {
            fn new(events: Vec<crate::Event>) -> Self {
                Self { events, index: 0 }
            }
        }

        impl EventSource for MockEventSource {
            fn next_event(&mut self) -> crate::Result<Option<crate::Event>> {
                if let Some(event) = self.events.get(self.index).cloned() {
                    self.index = self.index.saturating_add(1);
                    Ok(Some(event))
                } else {
                    Ok(None)
                }
            }
        }

        #[test]
        fn event_sink_collects_events() {
            let mut sink = MockEventSink::new();
            let event1 = crate::Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            };
            let event2 = crate::Event::StartParagraph {
                alignment: None,
                id: None,
            };
            let event3 = crate::Event::Text {
                content: "test".to_string(),
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            };

            let result1 = sink.handle_event(event1.clone());
            let result2 = sink.handle_event(event2.clone());
            let result3 = sink.handle_event(event3.clone());

            assert!(matches!(result1, Ok(())));
            assert!(matches!(result2, Ok(())));
            assert!(matches!(result3, Ok(())));
            assert_eq!(sink.events.len(), 3);
        }

        #[test]
        fn event_sink_finish_consumes_self() {
            let sink = MockEventSink::new();
            let result = sink.finish();
            assert!(matches!(result, Ok(())));
            // sink is consumed, cannot use it again
        }

        #[test]
        fn event_source_emits_events() {
            let events = vec![
                crate::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                crate::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                crate::Event::Text {
                    content: "Hello".to_string(),
                    bold: false,
                    italic: false,
                    code: false,
                    strikethrough: false,
                    underline: false,
                    subscript: false,
                    superscript: false,
                    mark: None,
                },
                crate::Event::EndParagraph,
                crate::Event::EndDocument,
            ];
            let mut source = MockEventSource::new(events.clone());

            let mut collected = Vec::new();
            while let Ok(Some(event)) = source.next_event() {
                collected.push(event);
            }

            assert_eq!(collected.len(), 5);
            assert_eq!(collected, events);
        }

        #[test]
        fn event_source_empty_stream() {
            let mut source = MockEventSource::new(Vec::new());
            assert!(matches!(source.next_event(), Ok(None)));
        }

        #[test]
        fn pipeline_source_to_sink() {
            let events = vec![
                crate::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                crate::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                crate::Event::Text {
                    content: "Pipeline test".to_string(),
                    bold: false,
                    italic: false,
                    code: false,
                    strikethrough: false,
                    underline: false,
                    subscript: false,
                    superscript: false,
                    mark: None,
                },
                crate::Event::EndParagraph,
                crate::Event::EndDocument,
            ];
            let mut source = MockEventSource::new(events.clone());
            let mut sink = MockEventSink::new();

            while let Ok(Some(event)) = source.next_event() {
                assert!(matches!(sink.handle_event(event), Ok(())));
            }

            assert_eq!(sink.events.len(), 5);
            let finish_result = sink.finish();
            assert!(matches!(finish_result, Ok(())));
        }
    }

    use super::*;
    use std::collections::HashMap;

    /// Mock `AssetProvider` for testing.
    struct MockAssetProvider {
        assets: HashMap<String, (String, Vec<u8>)>,
    }

    impl MockAssetProvider {
        fn new() -> Self {
            let mut assets = HashMap::new();
            assets.insert(
                "image1".to_string(),
                ("image/png".to_string(), vec![0x89, 0x50, 0x4E, 0x47]),
            );
            assets.insert(
                "doc1".to_string(),
                ("application/pdf".to_string(), vec![0x25, 0x50, 0x44, 0x46]),
            );
            Self { assets }
        }
    }

    impl AssetProvider for MockAssetProvider {
        fn content_type(&self, asset_id: &str) -> Option<Cow<'_, str>> {
            self.assets
                .get(asset_id)
                .map(|(mime, _)| Cow::Borrowed(mime.as_str()))
        }

        fn stream_to(&self, asset_id: &str, writer: &mut dyn Write) -> Option<io::Result<u64>> {
            self.assets.get(asset_id).map(|(_, bytes)| {
                writer
                    .write_all(bytes)
                    .map(|()| u64::try_from(bytes.len()).unwrap_or(u64::MAX))
            })
        }
    }

    #[test]
    fn content_type_known_asset() {
        let provider = MockAssetProvider::new();
        let mime = provider.content_type("image1");
        assert_eq!(mime, Some(Cow::Borrowed("image/png")));
    }

    #[test]
    fn content_type_unknown_asset() {
        let provider = MockAssetProvider::new();
        let mime = provider.content_type("unknown");
        assert_eq!(mime, None);
    }

    #[test]
    fn stream_to_known_asset() {
        let provider = MockAssetProvider::new();
        let mut buffer = Vec::new();
        let result = provider.stream_to("image1", &mut buffer);
        assert!(matches!(result, Some(Ok(4))));
        assert_eq!(buffer, vec![0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn stream_to_multiple_assets() {
        let provider = MockAssetProvider::new();
        let mut buffer1 = Vec::new();
        let mut buffer2 = Vec::new();

        let result1 = provider.stream_to("image1", &mut buffer1);
        let result2 = provider.stream_to("doc1", &mut buffer2);

        assert!(matches!(result1, Some(Ok(4))));
        assert_eq!(buffer1, vec![0x89, 0x50, 0x4E, 0x47]);

        assert!(matches!(result2, Some(Ok(4))));
        assert_eq!(buffer2, vec![0x25, 0x50, 0x44, 0x46]);
    }

    #[test]
    fn stream_to_unknown_asset() {
        let provider = MockAssetProvider::new();
        let mut buffer = Vec::new();
        let result = provider.stream_to("unknown", &mut buffer);
        assert!(result.is_none());
    }
}
