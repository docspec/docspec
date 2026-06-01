//! Tests.

#[cfg(test)]
mod tests {
    mod event_source_sink_tests {
        use docspec_core::*;

        /// Mock `EventSink` that collects events.
        struct MockEventSink {
            events: Vec<docspec_core::Event>,
        }

        impl MockEventSink {
            fn new() -> Self {
                Self { events: Vec::new() }
            }
        }

        impl EventSink for MockEventSink {
            fn finish(self) -> docspec_core::Result<()> {
                Ok(())
            }

            fn handle_event(&mut self, event: docspec_core::Event) -> docspec_core::Result<()> {
                self.events.push(event);
                Ok(())
            }
        }

        /// Mock `EventSource` that emits a fixed sequence of events.
        struct MockEventSource {
            events: Vec<docspec_core::Event>,
            index: usize,
        }

        impl MockEventSource {
            fn new(events: Vec<docspec_core::Event>) -> Self {
                Self { events, index: 0 }
            }
        }

        impl EventSource for MockEventSource {
            fn next_event(&mut self) -> docspec_core::Result<Option<docspec_core::Event>> {
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
            let event1 = docspec_core::Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            };
            let event2 = docspec_core::Event::StartParagraph {
                alignment: None,
                id: None,
            };
            let event3 = docspec_core::Event::Text {
                content: "test".to_string(),
                style: TextStyle::default(),
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
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "Hello".to_string(),
                    style: TextStyle::default(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
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
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "Pipeline test".to_string(),
                    style: TextStyle::default(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
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

    extern crate alloc;

    use alloc::borrow::Cow;
    use docspec_core::*;
    use std::collections::HashMap;
    use std::io::{self, Write};

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
        assert_eq!(buffer, Vec::<u8>::new());
    }
}
