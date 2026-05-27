//! Error types for `DocSpec` operations.

use core::fmt;

use thiserror::Error;

/// The position in a source document where an error occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    /// Byte offset from the start of the input.
    pub byte_offset: usize,
    /// Column number (1-based), if available.
    pub column: Option<usize>,
    /// Line number (1-based), if available.
    pub line: Option<usize>,
}

/// Errors that can occur during document processing.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// An event sequence violated the well-formedness rules.
    #[error("invalid event sequence: expected {expected}, found {found}: {message}")]
    InvalidSequence {
        /// The event type that was expected.
        expected: String,
        /// The event type that was actually found.
        found: String,
        /// Human-readable description.
        message: String,
    },
    /// An I/O error from the underlying reader or writer.
    #[error("I/O error: {source}")]
    Io {
        /// The underlying I/O error.
        #[from]
        source: std::io::Error,
    },
    /// A JSON parse or serialization error.
    #[error("{}", DisplayPos { label: "JSON error", message: message.as_str(), position: position.as_ref() })]
    Json {
        /// Human-readable description.
        message: String,
        /// Position in the JSON source, if known.
        position: Option<Position>,
    },
    /// An unclassified error.
    #[error("{message}")]
    Other {
        /// Human-readable description.
        message: String,
    },
    /// A parse error, optionally with position information.
    #[error("{}", DisplayPos { label: "parse error", message: message.as_str(), position: position.as_ref() })]
    Parse {
        /// Human-readable description of what went wrong.
        message: String,
        /// Position in the source where the error occurred, if known.
        position: Option<Position>,
    },
}

/// Result type alias for `DocSpec` operations.
pub type Result<T> = core::result::Result<T, Error>;

struct DisplayPos<'a> {
    label: &'a str,
    message: &'a str,
    position: Option<&'a Position>,
}

impl fmt::Display for DisplayPos<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)?;
        if let Some(pos) = self.position {
            let byte = pos.byte_offset;
            match (pos.line, pos.column) {
                (Some(line), Some(col)) => {
                    write!(f, " at line {line}, column {col} (byte {byte})")?;
                }
                (Some(line), _) => {
                    write!(f, " at line {line} (byte {byte})")?;
                }
                _ => {
                    write!(f, " at byte {byte}")?;
                }
            }
        }
        write!(f, ": {}", self.message)
    }
}
