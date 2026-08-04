//! The XML token pump for `document.xml`.
//!
//! [`XmlCursor`] owns the quick-xml reader together with its scratch buffer, so
//! reading a token borrows only the cursor. Keeping it separate from the parse
//! state is what lets a caller hold `&mut` to both at once.

use std::io::{BufReader, Read};

use docspec_core::{Error, Result};
use quick_xml::events::{BytesStart, Event as XmlEvent};

/// Initial scratch capacity, sized to hold a typical run of markup without regrowing.
const SCRATCH_CAPACITY: usize = 4096;

/// Streaming XML token source for the main document part.
pub(crate) struct XmlCursor {
    reader: quick_xml::Reader<BufReader<Box<dyn Read + Send>>>,
    scratch: Vec<u8>,
}

impl XmlCursor {
    /// Wraps a quick-xml reader.
    ///
    /// End-name checking is disabled because the parser resolves element scope
    /// from its own state and must tolerate the mismatched close tags that real
    /// Word output contains.
    pub(crate) fn new(mut reader: quick_xml::Reader<BufReader<Box<dyn Read + Send>>>) -> Self {
        reader.config_mut().check_end_names = false;
        Self {
            reader,
            scratch: Vec::with_capacity(SCRATCH_CAPACITY),
        }
    }

    /// Reads the next token, detached from the scratch buffer.
    ///
    /// The returned event owns its bytes so callers may keep it across further
    /// reads. That copy is what a future borrowed-token pump will remove.
    pub(crate) fn read_owned(&mut self) -> Result<XmlEvent<'static>> {
        self.scratch.clear();
        Ok(self
            .reader
            .read_event_into(&mut self.scratch)
            .map_err(map_quick_xml_error)?
            .into_owned())
    }

    /// Discards every token up to and including the close tag matching `start`.
    pub(crate) fn skip_subtree(&mut self, start: &BytesStart<'_>) -> Result<()> {
        let end = start.to_end().into_owned();
        self.reader
            .read_to_end_into(end.name(), &mut self.scratch)
            .map_err(map_quick_xml_error)?;
        Ok(())
    }
}

impl core::fmt::Debug for XmlCursor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("XmlCursor").finish_non_exhaustive()
    }
}

/// Converts a quick-xml failure into the crate error type, preserving I/O kind.
fn map_quick_xml_error(err: quick_xml::Error) -> Error {
    match err {
        quick_xml::Error::Io(source) => Error::Io {
            source: std::io::Error::new(source.kind(), source.to_string()),
        },
        other => Error::Parse {
            message: format!("malformed document.xml: {other}"),
            position: None,
        },
    }
}
