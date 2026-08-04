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

/// Maximum bytes a single XML token (one text node, comment, or skipped subtree)
/// may pull from the input before the pump fails.
///
/// `quick_xml` reads a whole node into scratch in one call, so a single 2 GiB
/// `<w:t>` run would inflate memory to gigabytes even though the surrounding
/// document streams — the streaming guarantee only bounds memory across *many*
/// nodes, not within one pathological node. This cap restores the guarantee: the
/// window resets before every token, so a legitimate document of any size (made
/// of normal-sized nodes) streams unaffected, while a single oversized node fails
/// fast. 64 MiB is far above any real run, heading, or attribute value.
const MAX_XML_NODE_BYTES: u64 = 64 * 1024 * 1024;

/// Error message surfaced when a single node exceeds [`MAX_XML_NODE_BYTES`].
const NODE_LIMIT_MESSAGE: &str = "document.xml node exceeds the size limit (possible zip bomb)";

/// A [`Read`] adapter that fails once more than `limit` bytes are read within the
/// current window. [`reset_window`](Self::reset_window) is called before each XML
/// token, so the limit applies *per node* rather than per document: a huge
/// document of many small nodes streams, but one oversized node stops fast.
struct NodeCappedReader<R> {
    inner: R,
    read_in_window: u64,
    limit: u64,
}

impl<R> NodeCappedReader<R> {
    fn new(inner: R, limit: u64) -> Self {
        Self {
            inner,
            read_in_window: 0,
            limit,
        }
    }

    fn reset_window(&mut self) {
        self.read_in_window = 0;
    }
}

impl<R: Read> Read for NodeCappedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read_in_window = self
            .read_in_window
            .saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
        if self.read_in_window > self.limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                NODE_LIMIT_MESSAGE,
            ));
        }
        Ok(n)
    }
}

/// Streaming XML token source for the main document part.
pub(crate) struct XmlCursor {
    reader: quick_xml::Reader<BufReader<NodeCappedReader<Box<dyn Read + Send>>>>,
    scratch: Vec<u8>,
}

impl XmlCursor {
    /// Wraps a quick-xml reader, inserting a per-node size cap around its input.
    ///
    /// End-name checking is disabled because the parser resolves element scope
    /// from its own state and must tolerate the mismatched close tags that real
    /// Word output contains.
    pub(crate) fn new(reader: quick_xml::Reader<BufReader<Box<dyn Read + Send>>>) -> Self {
        // Unwrap the caller's reader down to the boxed stream and re-wrap it with
        // the per-node cap. Nothing has been read yet at construction, so
        // `BufReader::into_inner` discards only an empty buffer.
        let stream: Box<dyn Read + Send> = reader.into_inner().into_inner();
        let capped = NodeCappedReader::new(stream, MAX_XML_NODE_BYTES);
        let mut capped_reader = quick_xml::Reader::from_reader(BufReader::new(capped));
        capped_reader.config_mut().check_end_names = false;
        Self {
            reader: capped_reader,
            scratch: Vec::with_capacity(SCRATCH_CAPACITY),
        }
    }

    /// Resets the per-node byte window so the next token starts from zero.
    fn reset_node_window(&mut self) {
        self.reader.get_mut().get_mut().reset_window();
    }

    /// Reads the next token, detached from the scratch buffer.
    ///
    /// The returned event owns its bytes so callers may keep it across further
    /// reads. That copy is what a future borrowed-token pump will remove.
    pub(crate) fn read_owned(&mut self) -> Result<XmlEvent<'static>> {
        self.scratch.clear();
        self.reset_node_window();
        Ok(self
            .reader
            .read_event_into(&mut self.scratch)
            .map_err(map_quick_xml_error)?
            .into_owned())
    }

    /// Discards every token up to and including the close tag matching `start`.
    pub(crate) fn skip_subtree(&mut self, start: &BytesStart<'_>) -> Result<()> {
        let end = start.to_end().into_owned();
        self.reset_node_window();
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

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;
    use std::io::Cursor;

    #[test]
    fn node_capped_reader_allows_up_to_limit_then_errors() {
        // A single read at the limit is fine.
        let mut at_limit = NodeCappedReader::new(Cursor::new(vec![b'A'; 8]), 8);
        let mut buf = Vec::new();
        assert_eq!(at_limit.read_to_end(&mut buf).unwrap(), 8);

        // One byte over the limit surfaces the zip-bomb error.
        let mut over_limit = NodeCappedReader::new(Cursor::new(vec![b'A'; 9]), 8);
        let mut sink = [0; 16];
        let err = over_limit.read(&mut sink).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(err.to_string(), NODE_LIMIT_MESSAGE);
    }

    #[test]
    fn reset_window_lets_a_new_node_start_from_zero() {
        // Two 6-byte reads exceed a limit of 8 only if the window is not reset.
        let mut reader = NodeCappedReader::new(Cursor::new(vec![b'A'; 12]), 8);
        let mut first = [0; 6];
        assert_eq!(reader.read(&mut first).unwrap(), 6);
        reader.reset_window();
        let mut second = [0; 6];
        assert_eq!(reader.read(&mut second).unwrap(), 6);
    }
}
