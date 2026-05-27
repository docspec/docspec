//! Error types for the CLI.

use thiserror::Error;

/// Result type alias for CLI operations.
pub type Result<T> = core::result::Result<T, CliError>;

/// CLI-specific error types.
///
/// Wraps underlying library errors and adds CLI-specific error conditions.
#[derive(Debug, Error)]
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

    /// Format reader or writer is not yet implemented.
    #[error("{format} reader not yet implemented")]
    FormatNotSupported {
        /// The format that is not supported.
        format: String,
    },

    /// I/O error from file operations.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Input and output paths are the same file.
    #[error("input and output paths refer to the same file")]
    SameInputOutput,
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
