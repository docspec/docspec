//! Error types for the CLI.

use core::fmt;

/// Result type alias for CLI operations.
pub type Result<T> = core::result::Result<T, CliError>;

/// CLI-specific error types.
///
/// Wraps underlying library errors and adds CLI-specific error conditions.
#[derive(Debug)]
pub enum CliError {
    /// Conversion pipeline error from `docspec_core`.
    Conversion(docspec_core::Error),

    /// Cannot detect format from path or explicit flag.
    FormatDetection {
        /// Human-readable description of the detection failure.
        message: String,
    },

    /// Format reader or writer is not yet implemented.
    FormatNotSupported {
        /// The format that is not supported.
        format: String,
    },

    /// I/O error from file operations.
    Io(std::io::Error),

    /// Input and output paths are the same file.
    SameInputOutput,
}

impl fmt::Display for CliError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conversion(err) => write!(f, "{err}"),
            Self::FormatDetection { message } => write!(f, "{message}"),
            Self::FormatNotSupported { format } => {
                write!(f, "{format} reader not yet implemented")
            }
            Self::Io(err) => write!(f, "{err}"),
            Self::SameInputOutput => {
                write!(f, "input and output paths refer to the same file")
            }
        }
    }
}

impl core::error::Error for CliError {
    #[inline]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Conversion(err) => Some(err),
            Self::FormatDetection { .. }
            | Self::FormatNotSupported { .. }
            | Self::SameInputOutput => None,
            Self::Io(err) => Some(err),
        }
    }
}

impl From<docspec_core::Error> for CliError {
    #[inline]
    fn from(err: docspec_core::Error) -> Self {
        Self::Conversion(err)
    }
}

impl From<std::io::Error> for CliError {
    #[inline]
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
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
    fn display_format_not_supported_error() {
        let err = CliError::FormatNotSupported {
            format: "blocknote".to_string(),
        };
        assert_eq!(err.to_string(), "blocknote reader not yet implemented");
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
}
