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

    #[test]
    fn asset_handle_is_dyn_compatible_send_sync() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<std::sync::Arc<dyn docspec_core::AssetHandle>>();
    }
}
