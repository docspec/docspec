//! Stream-driver helper for connecting an [`EventSource`] to an [`EventSink`].

use crate::{EventSink, EventSource, Result};

/// Pull events from `source` and push them into `sink` until the source
/// drains, then call [`EventSink::finish`] on the sink.
///
/// This is the canonical way to run a `DocSpec` conversion pipeline. The
/// function returns on the first error from either side and never buffers
/// events beyond the single in-flight event.
///
/// # Errors
///
/// Returns any error produced by `source.next_event()`, `sink.handle_event()`,
/// or `sink.finish()`. Errors propagate immediately; processing stops at the
/// first error.
#[inline]
pub fn pipe<S, K>(mut source: S, mut sink: K) -> Result<()>
where
    S: EventSource,
    K: EventSink,
{
    while let Some(event) = source.next_event()? {
        sink.handle_event(event)?;
    }
    sink.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, Event};
    use alloc::rc::Rc;
    use alloc::vec;
    use alloc::vec::Vec;
    use core::cell::{Cell, RefCell};

    /// Externally-observable probe shared between a test and its sink. The sink
    /// holds one `Rc` clone, the test holds another; mutations to `finished`
    /// and `events` survive the `EventSink::finish(self)` consume because the
    /// `Rc` keeps the inner cells alive on the test side.
    #[derive(Default, Clone)]
    struct Probe {
        events: Rc<RefCell<Vec<Event>>>,
        finished: Rc<Cell<bool>>,
    }

    struct VecSource {
        events: alloc::vec::IntoIter<Event>,
    }
    impl VecSource {
        fn new(events: Vec<Event>) -> Self {
            Self {
                events: events.into_iter(),
            }
        }
    }
    impl EventSource for VecSource {
        fn next_event(&mut self) -> Result<Option<Event>> {
            Ok(self.events.next())
        }
    }

    struct CollectingSink {
        probe: Probe,
    }
    impl CollectingSink {
        fn new(probe: Probe) -> Self {
            Self { probe }
        }
    }
    impl EventSink for CollectingSink {
        fn finish(self) -> Result<()> {
            self.probe.finished.set(true);
            Ok(())
        }
        fn handle_event(&mut self, event: Event) -> Result<()> {
            self.probe.events.borrow_mut().push(event);
            Ok(())
        }
    }

    struct ErroringSource;
    impl EventSource for ErroringSource {
        fn next_event(&mut self) -> Result<Option<Event>> {
            Err(Error::Other {
                message: "source boom".to_string(),
            })
        }
    }

    struct ErroringSink {
        finished: Rc<Cell<bool>>,
    }
    impl EventSink for ErroringSink {
        fn finish(self) -> Result<()> {
            self.finished.set(true);
            Ok(())
        }
        fn handle_event(&mut self, _: Event) -> Result<()> {
            Err(Error::Other {
                message: "sink boom".to_string(),
            })
        }
    }

    #[test]
    fn pipe_drains_empty_source_and_finishes() {
        let probe = Probe::default();
        let result = pipe(VecSource::new(vec![]), CollectingSink::new(probe.clone()));
        assert!(result.is_ok());
        assert!(
            probe.finished.get(),
            "finish() must be called when source drains cleanly"
        );
        assert!(
            probe.events.borrow().is_empty(),
            "no events should reach the sink from an empty source"
        );
    }

    #[test]
    fn pipe_forwards_all_events_and_finishes() {
        let probe = Probe::default();
        let events = vec![
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::EndDocument,
        ];
        let result = pipe(VecSource::new(events), CollectingSink::new(probe.clone()));
        assert!(result.is_ok());
        assert!(
            probe.finished.get(),
            "finish() must be called after all events are forwarded"
        );
        let collected = probe.events.borrow();
        assert!(
            matches!(
                collected.as_slice(),
                [Event::StartDocument { .. }, Event::EndDocument]
            ),
            "expected exactly [StartDocument, EndDocument], got {collected:?}"
        );
    }

    #[test]
    fn pipe_propagates_source_error() {
        let probe = Probe::default();
        let result = pipe(ErroringSource, CollectingSink::new(probe.clone()));
        assert!(matches!(result, Err(Error::Other { .. })));
        assert!(
            !probe.finished.get(),
            "finish() must be skipped when the source errors"
        );
    }

    #[test]
    fn pipe_propagates_sink_error_and_skips_finish() {
        let finished = Rc::new(Cell::new(false));
        let events = vec![Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }];
        let result = pipe(
            VecSource::new(events),
            ErroringSink {
                finished: Rc::clone(&finished),
            },
        );
        assert!(matches!(result, Err(Error::Other { .. })));
        assert!(
            !finished.get(),
            "finish() must be skipped when the sink errors during handle_event"
        );
    }
}
