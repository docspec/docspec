//! Error types for DocSpec operations.

use std::fmt;

/// The position in a source document where an error occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    /// Byte offset from the start of the input.
    pub byte_offset: usize,
    /// Line number (1-based), if available.
    pub line: Option<usize>,
    /// Column number (1-based), if available.
    pub column: Option<usize>,
}

/// Errors that can occur during document processing.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A parse error, optionally with position information.
    Parse {
        /// Human-readable description of what went wrong.
        message: String,
        /// Position in the source where the error occurred, if known.
        position: Option<Position>,
    },
    /// An event sequence violated the well-formedness rules.
    InvalidSequence {
        /// Human-readable description.
        message: String,
        /// The event type that was expected.
        expected: String,
        /// The event type that was actually found.
        found: String,
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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse { message, position } => {
                if let Some(pos) = position {
                    match (pos.line, pos.column) {
                        (Some(line), Some(col)) => write!(
                            f,
                            "parse error at line {}, column {} (byte {}): {}",
                            line, col, pos.byte_offset, message
                        ),
                        (Some(line), None) => write!(
                            f,
                            "parse error at line {} (byte {}): {}",
                            line, pos.byte_offset, message
                        ),
                        _ => write!(f, "parse error at byte {}: {}", pos.byte_offset, message),
                    }
                } else {
                    write!(f, "parse error: {}", message)
                }
            }
            Error::InvalidSequence {
                message,
                expected,
                found,
            } => {
                write!(
                    f,
                    "invalid event sequence: expected {}, found {}: {}",
                    expected, found, message
                )
            }
            Error::Io { source } => {
                write!(f, "I/O error: {}", source)
            }
            Error::Json { message, position } => {
                if let Some(pos) = position {
                    match (pos.line, pos.column) {
                        (Some(line), Some(col)) => write!(
                            f,
                            "JSON error at line {}, column {} (byte {}): {}",
                            line, col, pos.byte_offset, message
                        ),
                        (Some(line), None) => write!(
                            f,
                            "JSON error at line {} (byte {}): {}",
                            line, pos.byte_offset, message
                        ),
                        _ => write!(f, "JSON error at byte {}: {}", pos.byte_offset, message),
                    }
                } else {
                    write!(f, "JSON error: {}", message)
                }
            }
            Error::Other { message } => {
                write!(f, "{}", message)
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source } => Some(source),
            _ => None,
        }
    }
}

/// Result type alias for DocSpec operations.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as StdError;

    #[test]
    fn test_parse_error_with_position() {
        let pos = Position {
            byte_offset: 42,
            line: Some(5),
            column: Some(10),
        };
        let err = Error::Parse {
            message: "unexpected character".to_string(),
            position: Some(pos),
        };
        assert_eq!(
            err.to_string(),
            "parse error at line 5, column 10 (byte 42): unexpected character"
        );
    }

    #[test]
    fn test_parse_error_without_position() {
        let err = Error::Parse {
            message: "unexpected character".to_string(),
            position: None,
        };
        assert_eq!(err.to_string(), "parse error: unexpected character");
    }

    #[test]
    fn test_invalid_sequence_error() {
        let err = Error::InvalidSequence {
            message: "heading must be closed before starting a new one".to_string(),
            expected: "EndHeading".to_string(),
            found: "StartHeading".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "invalid event sequence: expected EndHeading, found StartHeading: heading must be closed before starting a new one"
        );
    }

    #[test]
    fn test_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = Error::Io { source: io_err };
        assert!(err.to_string().starts_with("I/O error:"));
    }

    #[test]
    fn test_json_error_with_position() {
        let pos = Position {
            byte_offset: 100,
            line: Some(10),
            column: Some(5),
        };
        let err = Error::Json {
            message: "invalid JSON syntax".to_string(),
            position: Some(pos),
        };
        assert_eq!(
            err.to_string(),
            "JSON error at line 10, column 5 (byte 100): invalid JSON syntax"
        );
    }

    #[test]
    fn test_json_error_without_position() {
        let err = Error::Json {
            message: "invalid JSON syntax".to_string(),
            position: None,
        };
        assert_eq!(err.to_string(), "JSON error: invalid JSON syntax");
    }

    #[test]
    fn test_other_error() {
        let err = Error::Other {
            message: "something went wrong".to_string(),
        };
        assert_eq!(err.to_string(), "something went wrong");
    }

    #[test]
    fn test_parse_error_with_position_line_only() {
        let pos = Position {
            byte_offset: 42,
            line: Some(5),
            column: None,
        };
        let err = Error::Parse {
            message: "unexpected character".to_string(),
            position: Some(pos),
        };
        assert_eq!(
            err.to_string(),
            "parse error at line 5 (byte 42): unexpected character"
        );
    }

    #[test]
    fn test_parse_error_with_position_byte_only() {
        let pos = Position {
            byte_offset: 42,
            line: None,
            column: None,
        };
        let err = Error::Parse {
            message: "unexpected character".to_string(),
            position: Some(pos),
        };
        assert_eq!(
            err.to_string(),
            "parse error at byte 42: unexpected character"
        );
    }

    #[test]
    fn test_json_error_with_position_line_only() {
        let pos = Position {
            byte_offset: 100,
            line: Some(10),
            column: None,
        };
        let err = Error::Json {
            message: "invalid JSON syntax".to_string(),
            position: Some(pos),
        };
        assert_eq!(
            err.to_string(),
            "JSON error at line 10 (byte 100): invalid JSON syntax"
        );
    }

    #[test]
    fn test_json_error_with_position_byte_only() {
        let pos = Position {
            byte_offset: 100,
            line: None,
            column: None,
        };
        let err = Error::Json {
            message: "invalid JSON syntax".to_string(),
            position: Some(pos),
        };
        assert_eq!(
            err.to_string(),
            "JSON error at byte 100: invalid JSON syntax"
        );
    }

    #[test]
    fn test_error_source_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = Error::Io { source: io_err };
        assert!(StdError::source(&err).is_some());
    }

    #[test]
    fn test_error_source_parse() {
        let err = Error::Parse {
            message: "test".to_string(),
            position: None,
        };
        assert!(StdError::source(&err).is_none());
    }

    #[test]
    fn test_error_source_invalid_sequence() {
        let err = Error::InvalidSequence {
            message: "test".to_string(),
            expected: "A".to_string(),
            found: "B".to_string(),
        };
        assert!(StdError::source(&err).is_none());
    }

    #[test]
    fn test_error_source_json() {
        let err = Error::Json {
            message: "test".to_string(),
            position: None,
        };
        assert!(StdError::source(&err).is_none());
    }

    #[test]
    fn test_error_source_other() {
        let err = Error::Other {
            message: "test".to_string(),
        };
        assert!(StdError::source(&err).is_none());
    }

    #[test]
    fn error_is_send_sync_static() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<Error>();
    }

    #[test]
    fn result_type_alias_works() {
        let _: Result<i32> = Err(Error::Other {
            message: "test".to_string(),
        });
        let _: Result<String> = Ok("success".to_string());
    }
}
