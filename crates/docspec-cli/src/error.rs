//! Error types for the CLI, and the exit code each one maps to.

use std::io::ErrorKind;
use std::path::PathBuf;

use thiserror::Error;

#[cfg(feature = "http")]
use docspec_http::server::ServerError;

/// Process exit codes, from `<sysexits.h>` except where noted.
///
/// One code has one meaning, so a script can branch on `$?` without parsing
/// stderr. The HTTP server draws the same lines with 415, 422 and 500.
pub mod exit {
    /// Command used incorrectly. `2` is clap's convention (and bash's), kept
    /// deliberately instead of sysexits' `EX_USAGE = 64` so clap's own parse
    /// errors and ours share a code.
    pub const USAGE: u8 = 2;
    /// `EX_DATAERR`: the input data was incorrect in some way.
    pub const DATAERR: u8 = 65;
    /// `EX_NOINPUT`: an input file did not exist.
    pub const NOINPUT: u8 = 66;
    /// `EX_UNAVAILABLE`: a service is unavailable; here, the listen address.
    #[cfg(feature = "http")]
    pub const UNAVAILABLE: u8 = 69;
    /// `EX_SOFTWARE`: an internal software error, a `DocSpec` defect.
    pub const SOFTWARE: u8 = 70;
    /// `EX_OSERR`: an operating system error, such as a resource it refused.
    #[cfg(feature = "http")]
    pub const OSERR: u8 = 71;
    /// `EX_CANTCREAT`: a user-specified output file cannot be created.
    pub const CANTCREAT: u8 = 73;
    /// `EX_IOERR`: an error occurred while doing I/O on some file.
    pub const IOERR: u8 = 74;
    /// `EX_NOPERM`: permission denied.
    pub const NOPERM: u8 = 77;
}

/// Result type alias for CLI operations.
pub type Result<T> = core::result::Result<T, CliError>;

/// CLI-specific error types.
///
/// Wraps underlying library errors and adds CLI-specific error conditions.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CliError {
    /// Conversion pipeline error from `docspec_core`.
    #[error(transparent)]
    Conversion(#[from] docspec_core::Error),

    /// Cannot detect format from path or explicit flag.
    #[error("{message}")]
    FormatDetection {
        /// Human-readable description of the detection failure.
        message: String,
    },

    /// HTTP server error.
    #[cfg(feature = "http")]
    #[error("HTTP server error: {0}")]
    Http(#[from] ServerError),

    /// The input path cannot be read: missing, not permitted, or not a file.
    #[error("cannot read input `{}`: {source}", path.display())]
    InputUnreadable {
        /// The input path as given on the command line.
        path: PathBuf,
        /// The underlying error from the file system.
        source: std::io::Error,
    },

    /// I/O error from file operations.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// The output path cannot be created.
    #[error("cannot create output `{}`: {source}", path.display())]
    OutputUnwritable {
        /// The output path as given on the command line.
        path: PathBuf,
        /// The underlying error from the file system.
        source: std::io::Error,
    },

    /// Input and output paths are the same file.
    #[error("input and output paths refer to the same file")]
    SameInputOutput,
}

impl CliError {
    /// The process exit code for this error, per [`exit`].
    ///
    /// Classification is by error kind, never by message text. A core
    /// `InvalidSequence` can only be a `DocSpec` defect, so it is
    /// [`exit::SOFTWARE`]; every other conversion error describes the input
    /// and is [`exit::DATAERR`], except a core `Io`, which on the CLI is a
    /// real read or write failure and so [`exit::IOERR`].
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Conversion(docspec_core::Error::InvalidSequence { .. }) => exit::SOFTWARE,
            Self::Conversion(docspec_core::Error::Io { .. }) | Self::Io(_) => exit::IOERR,
            Self::Conversion(_) => exit::DATAERR,
            Self::FormatDetection { .. } | Self::SameInputOutput => exit::USAGE,
            Self::InputUnreadable { source, .. } => match source.kind() {
                ErrorKind::NotFound => exit::NOINPUT,
                ErrorKind::PermissionDenied => exit::NOPERM,
                _ => exit::IOERR,
            },
            Self::OutputUnwritable { source, .. } => match source.kind() {
                ErrorKind::PermissionDenied => exit::NOPERM,
                _ => exit::CANTCREAT,
            },
            #[cfg(feature = "http")]
            Self::Http(ServerError::Listen(source)) => match source.kind() {
                ErrorKind::PermissionDenied => exit::NOPERM,
                ErrorKind::AddrInUse | ErrorKind::AddrNotAvailable => exit::UNAVAILABLE,
                _ => exit::OSERR,
            },
            #[cfg(feature = "http")]
            Self::Http(ServerError::RuntimeBuild(_)) => exit::OSERR,
            #[cfg(feature = "http")]
            Self::Http(_) => exit::SOFTWARE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_conversion_error() {
        let inner = docspec_core::Error::Other {
            message: "pipeline failed".to_string(),
        };
        let err = CliError::Conversion(inner);
        assert!(err.to_string().contains("pipeline failed"));
    }

    #[test]
    fn display_format_detection_error() {
        let err = CliError::FormatDetection {
            message: "cannot detect format".to_string(),
        };
        assert_eq!(err.to_string(), "cannot detect format");
    }

    #[test]
    fn display_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = CliError::Io(io_err);
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn display_same_input_output() {
        let err = CliError::SameInputOutput;
        assert!(err.to_string().contains("same file"));
    }

    #[test]
    fn from_docspec_error() {
        let inner = docspec_core::Error::Other {
            message: "test".to_string(),
        };
        let err = CliError::from(inner);
        assert!(matches!(err, CliError::Conversion(_)));
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let err = CliError::from(io_err);
        assert!(matches!(err, CliError::Io(_)));
    }

    fn io(kind: ErrorKind) -> std::io::Error {
        std::io::Error::new(kind, "io")
    }

    fn core_error(kind: &str) -> docspec_core::Error {
        match kind {
            "parse" => docspec_core::Error::Parse {
                message: "bad".to_owned(),
                position: None,
            },
            "json" => docspec_core::Error::Json {
                message: "bad".to_owned(),
                position: None,
            },
            "other" => docspec_core::Error::Other {
                message: "bad".to_owned(),
            },
            "io" => docspec_core::Error::Io {
                source: io(ErrorKind::UnexpectedEof),
            },
            "limit" => docspec_core::Error::Limit {
                message: "too deep".to_owned(),
            },
            _ => docspec_core::Error::InvalidSequence {
                expected: "a".to_owned(),
                found: "b".to_owned(),
                message: "bad".to_owned(),
            },
        }
    }

    #[test]
    fn conversion_errors_exit_by_kind() {
        assert_eq!(CliError::Conversion(core_error("parse")).exit_code(), 65);
        assert_eq!(CliError::Conversion(core_error("json")).exit_code(), 65);
        assert_eq!(CliError::Conversion(core_error("other")).exit_code(), 65);
        assert_eq!(CliError::Conversion(core_error("io")).exit_code(), 74);
        assert_eq!(CliError::Conversion(core_error("limit")).exit_code(), 65);
        assert_eq!(
            CliError::Conversion(core_error("invalid_sequence")).exit_code(),
            70
        );
    }

    #[test]
    fn usage_errors_exit_2() {
        assert_eq!(
            CliError::FormatDetection {
                message: "x".to_owned()
            }
            .exit_code(),
            2
        );
        assert_eq!(CliError::SameInputOutput.exit_code(), 2);
    }

    #[test]
    fn io_during_conversion_exits_74() {
        assert_eq!(CliError::Io(io(ErrorKind::BrokenPipe)).exit_code(), 74);
    }

    #[test]
    fn input_unreadable_exits_by_io_kind() {
        let unreadable = |kind| CliError::InputUnreadable {
            path: PathBuf::from("in.md"),
            source: io(kind),
        };
        assert_eq!(unreadable(ErrorKind::NotFound).exit_code(), 66);
        assert_eq!(unreadable(ErrorKind::PermissionDenied).exit_code(), 77);
        assert_eq!(unreadable(ErrorKind::Other).exit_code(), 74);
        assert_eq!(
            unreadable(ErrorKind::NotFound).to_string(),
            "cannot read input `in.md`: io"
        );
    }

    #[test]
    fn output_unwritable_exits_by_io_kind() {
        let unwritable = |kind| CliError::OutputUnwritable {
            path: PathBuf::from("out.json"),
            source: io(kind),
        };
        assert_eq!(unwritable(ErrorKind::NotFound).exit_code(), 73);
        assert_eq!(unwritable(ErrorKind::PermissionDenied).exit_code(), 77);
        assert_eq!(
            unwritable(ErrorKind::NotFound).to_string(),
            "cannot create output `out.json`: io"
        );
    }

    #[cfg(feature = "http")]
    #[test]
    fn server_errors_exit_by_cause() {
        let listen = |kind| CliError::Http(ServerError::Listen(io(kind)));
        assert_eq!(listen(ErrorKind::AddrInUse).exit_code(), 69);
        assert_eq!(listen(ErrorKind::AddrNotAvailable).exit_code(), 69);
        assert_eq!(listen(ErrorKind::PermissionDenied).exit_code(), 77);
        assert_eq!(listen(ErrorKind::Other).exit_code(), 71);
        assert_eq!(
            CliError::Http(ServerError::RuntimeBuild(io(ErrorKind::Other))).exit_code(),
            71
        );
    }
}
