//! ODT to `DocSpec` event stream reader — **placeholder**.
//!
//! ODT is not parsed yet. [`OdtReader`] ignores its input and always emits an
//! empty document: [`Event::StartDocument`] followed by [`Event::EndDocument`].
//! Writers turn that into their normal empty-document output.
//!
//! ```
//! use docspec_odt_reader::{EventSource, OdtReader};
//!
//! let mut reader = OdtReader::new();
//! while let Some(event) = reader.next_event()? {
//!     println!("{event:?}");
//! }
//! # Ok::<(), docspec_core::Error>(())
//! ```

pub use docspec_core::EventSource;
use docspec_core::{Event, Result};

/// Which event comes next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    /// `StartDocument` is next.
    #[default]
    NotStarted,
    /// `EndDocument` is next.
    Started,
    /// The stream is exhausted.
    Finished,
}

/// Placeholder ODT reader that emits an empty document.
#[derive(Clone, Debug, Default)]
pub struct OdtReader {
    phase: Phase,
}

impl OdtReader {
    /// Create a reader. There is no input to pass yet: ODT is not parsed.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl EventSource for OdtReader {
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        Ok(match self.phase {
            Phase::NotStarted => {
                self.phase = Phase::Started;
                Some(Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                })
            }
            Phase::Started => {
                self.phase = Phase::Finished;
                Some(Event::EndDocument)
            }
            Phase::Finished => None,
        })
    }
}
