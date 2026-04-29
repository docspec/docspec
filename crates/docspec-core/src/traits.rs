//! Core traits for event sources, sinks, and asset providers.

use std::borrow::Cow;
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

/// Consumes a stream of [`crate::Event`]s to produce output.
///
/// Writers implement this trait to translate document events into a target
/// format. Call [`EventSink::handle_event`] for each event in order, then
/// call [`EventSink::finish`] to flush output and signal completion.
///
/// `finish` consumes `self` to prevent reuse after the stream has ended.
pub trait EventSink {
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

    /// Flush any buffered output and finalize the document.
    ///
    /// Consumes `self` to prevent further use after the stream ends.
    ///
    /// # Errors
    ///
    /// Returns an error if flushing or finalization fails.
    fn finish(self) -> crate::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Mock AssetProvider for testing.
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
            MockAssetProvider { assets }
        }
    }

    impl AssetProvider for MockAssetProvider {
        fn content_type(&self, asset_id: &str) -> Option<Cow<'_, str>> {
            self.assets
                .get(asset_id)
                .map(|(mime, _)| Cow::Borrowed(mime.as_str()))
        }

        fn stream_to(&self, asset_id: &str, writer: &mut dyn Write) -> Option<io::Result<u64>> {
            self.assets
                .get(asset_id)
                .map(|(_, bytes)| writer.write_all(bytes).map(|_| bytes.len() as u64))
        }
    }

    #[test]
    fn test_content_type_known_asset() {
        let provider = MockAssetProvider::new();
        let mime = provider.content_type("image1");
        assert_eq!(mime, Some(Cow::Borrowed("image/png")));
    }

    #[test]
    fn test_content_type_unknown_asset() {
        let provider = MockAssetProvider::new();
        let mime = provider.content_type("unknown");
        assert_eq!(mime, None);
    }

    #[test]
    fn test_stream_to_known_asset() {
        let provider = MockAssetProvider::new();
        let mut buffer = Vec::new();
        let result = provider.stream_to("image1", &mut buffer);
        assert!(matches!(result, Some(Ok(4))));
        assert_eq!(buffer, vec![0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_stream_to_unknown_asset() {
        let provider = MockAssetProvider::new();
        let mut buffer = Vec::new();
        let result = provider.stream_to("unknown", &mut buffer);
        assert!(result.is_none());
    }

    #[test]
    fn test_stream_to_multiple_assets() {
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

    mod event_source_sink_tests {
        use super::*;

        /// Mock EventSource that emits a fixed sequence of events.
        struct MockEventSource {
            events: Vec<crate::Event>,
            index: usize,
        }

        impl MockEventSource {
            fn new(events: Vec<crate::Event>) -> Self {
                MockEventSource { events, index: 0 }
            }
        }

        impl EventSource for MockEventSource {
            fn next_event(&mut self) -> crate::Result<Option<crate::Event>> {
                if self.index < self.events.len() {
                    let event = self.events[self.index].clone();
                    self.index += 1;
                    Ok(Some(event))
                } else {
                    Ok(None)
                }
            }
        }

        /// Mock EventSink that collects events.
        struct MockEventSink {
            events: Vec<crate::Event>,
        }

        impl MockEventSink {
            fn new() -> Self {
                MockEventSink { events: Vec::new() }
            }
        }

        impl EventSink for MockEventSink {
            fn handle_event(&mut self, event: crate::Event) -> crate::Result<()> {
                self.events.push(event);
                Ok(())
            }

            fn finish(self) -> crate::Result<()> {
                Ok(())
            }
        }

        #[test]
        fn test_event_source_emits_events() {
            let events = vec![
                crate::Event::StartDocument {
                    language: None,
                    metadata: None,
                },
                crate::Event::StartParagraph { alignment: None },
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
        fn test_event_sink_collects_events() {
            let mut sink = MockEventSink::new();
            let event1 = crate::Event::StartDocument {
                language: None,
                metadata: None,
            };
            let event2 = crate::Event::StartParagraph { alignment: None };
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

            assert!(result1.is_ok());
            assert!(result2.is_ok());
            assert!(result3.is_ok());
            assert_eq!(sink.events.len(), 3);
        }

        #[test]
        fn test_pipeline_source_to_sink() {
            let events = vec![
                crate::Event::StartDocument {
                    language: None,
                    metadata: None,
                },
                crate::Event::StartParagraph { alignment: None },
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
                assert!(sink.handle_event(event).is_ok());
            }

            assert_eq!(sink.events.len(), 5);
            let finish_result = sink.finish();
            assert!(finish_result.is_ok());
        }

        #[test]
        fn test_event_sink_finish_consumes_self() {
            let sink = MockEventSink::new();
            let result = sink.finish();
            assert!(result.is_ok());
            // sink is consumed, cannot use it again
        }

        #[test]
        fn test_event_source_empty_stream() {
            let mut source = MockEventSource::new(Vec::new());
            assert!(matches!(source.next_event(), Ok(None)));
        }
    }
}
