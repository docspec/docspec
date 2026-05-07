//! Error types for `DocSpec` operations.

use core::fmt;

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
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An event sequence violated the well-formedness rules.
    InvalidSequence {
        /// The event type that was expected.
        expected: String,
        /// The event type that was actually found.
        found: String,
        /// Human-readable description.
        message: String,
    },
    /// An I/O error from the underlying reader or writer.
    Io {
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// A JSON parse or serialization error.
    Json {
        /// Human-readable description.
        message: String,
        /// Position in the JSON source, if known.
        position: Option<Position>,
    },
    /// An unclassified error.
    Other {
        /// Human-readable description.
        message: String,
    },
    /// A parse error, optionally with position information.
    Parse {
        /// Human-readable description of what went wrong.
        message: String,
        /// Position in the source where the error occurred, if known.
        position: Option<Position>,
    },
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSequence {
                message,
                expected,
                found,
            } => {
                write!(
                    f,
                    "invalid event sequence: expected {expected}, found {found}: {message}"
                )
            }
            Self::Io { source } => {
                write!(f, "I/O error: {source}")
            }
            Self::Json { message, position } => {
                if let Some(pos) = position {
                    match (pos.line, pos.column) {
                        (Some(line), Some(col)) => write!(
                            f,
                            "JSON error at line {line}, column {col} (byte {byte}): {message}",
                            byte = pos.byte_offset
                        ),
                        (Some(line), None) => write!(
                            f,
                            "JSON error at line {line} (byte {byte}): {message}",
                            byte = pos.byte_offset
                        ),
                        _ => write!(
                            f,
                            "JSON error at byte {byte}: {message}",
                            byte = pos.byte_offset
                        ),
                    }
                } else {
                    write!(f, "JSON error: {message}")
                }
            }
            Self::Other { message } => {
                write!(f, "{message}")
            }
            Self::Parse { message, position } => {
                if let Some(pos) = position {
                    match (pos.line, pos.column) {
                        (Some(line), Some(col)) => write!(
                            f,
                            "parse error at line {line}, column {col} (byte {byte}): {message}",
                            byte = pos.byte_offset
                        ),
                        (Some(line), None) => write!(
                            f,
                            "parse error at line {line} (byte {byte}): {message}",
                            byte = pos.byte_offset
                        ),
                        _ => write!(
                            f,
                            "parse error at byte {byte}: {message}",
                            byte = pos.byte_offset
                        ),
                    }
                } else {
                    write!(f, "parse error: {message}")
                }
            }
        }
    }
}

impl core::error::Error for Error {
    #[inline]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Io { source } => Some(source),
            Self::InvalidSequence { .. }
            | Self::Json { .. }
            | Self::Other { .. }
            | Self::Parse { .. } => None,
        }
    }
}

/// Result type alias for `DocSpec` operations.
pub type Result<T> = core::result::Result<T, Error>;
