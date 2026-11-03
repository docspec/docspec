//! ODT to `DocSpec` event stream reader — **placeholder**.
//!
//! ODT is not parsed yet. [`OdtReader`] accepts its input but does not read it,
//! and always emits an empty document: [`Event::StartDocument`] followed by
//! [`Event::EndDocument`]. Writers turn that into their normal empty-document
//! output. The constructors already have the shape a real reader needs, so the
//! parser can replace this without changing the API.
//!
//! ```
//! use std::io::Cursor;
//! use docspec_odt_reader::{EventSource, OdtReader};
//!
//! let mut reader = OdtReader::from_reader(Cursor::new(b"PK..."))?;
//! while let Some(event) = reader.next_event()? {
//!     println!("{event:?}");
//! }
//! # Ok::<(), docspec_core::Error>(())
//! ```

use std::io::{Read, Seek};
use std::path::Path;

pub use docspec_core::EventSource;
use docspec_core::{Error, Event, Result};

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
#[derive(Clone, Debug)]
pub struct OdtReader {
    phase: Phase,
}

impl OdtReader {
    /// Create a reader from any `Read + Seek` source.
    ///
    /// Placeholder: the source is not read.
    ///
    /// # Errors
    ///
    /// Never fails yet; the `Result` is there for the real parser.
    #[inline]
    pub fn from_reader<R>(reader: R) -> Result<Self>
    where
        R: Read + Seek + Send + 'static,
    {
        drop(reader);
        Ok(Self {
            phase: Phase::NotStarted,
        })
    }

    /// Create a reader for the file at `path`.
    ///
    /// Placeholder: the file is opened but not read.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be opened or `path` is a directory.
    #[inline]
    pub fn from_path<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let file = std::fs::File::open(path).map_err(|source| Error::Io { source })?;
        let metadata = file.metadata().map_err(|source| Error::Io { source })?;
        if metadata.is_dir() {
            return Err(Error::Io {
                source: std::io::ErrorKind::IsADirectory.into(),
            });
        }
        Self::from_reader(file)
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
