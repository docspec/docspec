//! Tests.

#[cfg(test)]
mod tests {
    use core::error::Error as StdError;
    use docspec_core::*;

    #[test]
    fn error_is_send_sync_static() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<Error>();
    }

    #[test]
    fn error_source_invalid_sequence() {
        let err = Error::InvalidSequence {
            message: "test".to_string(),
            expected: "A".to_string(),
            found: "B".to_string(),
        };
        assert_eq!(StdError::source(&err).map(ToString::to_string), None);
    }

    #[test]
    fn error_source_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = Error::Io { source: io_err };
        assert_eq!(
            StdError::source(&err).map(ToString::to_string),
            Some("access denied".to_string())
        );
    }

    #[test]
    fn error_source_json() {
        let err = Error::Json {
            message: "test".to_string(),
            position: None,
        };
        assert_eq!(StdError::source(&err).map(ToString::to_string), None);
    }

    #[test]
    fn error_source_other() {
        let err = Error::Other {
            message: "test".to_string(),
        };
        assert_eq!(StdError::source(&err).map(ToString::to_string), None);
    }

    #[test]
    fn error_source_parse() {
        let err = Error::Parse {
            message: "test".to_string(),
            position: None,
        };
        assert_eq!(StdError::source(&err).map(ToString::to_string), None);
    }

    #[test]
    fn invalid_sequence_error() {
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
    fn io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = Error::Io { source: io_err };
        assert_eq!(err.to_string(), "I/O error: file not found");
    }

    #[test]
    fn json_error_with_position() {
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
    fn json_error_with_position_byte_only() {
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
    fn json_error_with_position_line_only() {
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
    fn json_error_without_position() {
        let err = Error::Json {
            message: "invalid JSON syntax".to_string(),
            position: None,
        };
        assert_eq!(err.to_string(), "JSON error: invalid JSON syntax");
    }

    #[test]
    fn other_error() {
        let err = Error::Other {
            message: "something went wrong".to_string(),
        };
        assert_eq!(err.to_string(), "something went wrong");
    }

    #[test]
    fn parse_error_with_position() {
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
    fn parse_error_with_position_byte_only() {
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
    fn parse_error_with_position_line_only() {
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
    fn parse_error_without_position() {
        let err = Error::Parse {
            message: "unexpected character".to_string(),
            position: None,
        };
        assert_eq!(err.to_string(), "parse error: unexpected character");
    }

    #[test]
    fn result_type_alias_works() {
        drop::<Result<i32>>(Err(Error::Other {
            message: "test".to_string(),
        }));
        drop::<Result<String>>(Ok("success".to_string()));
    }
}
