//! Integration tests for the DOCX reader re-exported through the docspec facade.

#![cfg(feature = "docx")]
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[cfg(test)]
mod tests {
    use std::io::{Cursor, ErrorKind};

    use docspec::readers::DocxReader;
    use docspec::{Error, EventSource};

    #[test]
    fn docx_reader_implements_event_source() {
        fn assert_event_source<S: EventSource>() {}
        assert_event_source::<DocxReader>();
    }

    #[test]
    fn docx_reader_from_path_propagates_not_found_io_error() {
        let result = DocxReader::from_path("/nonexistent/path/does/not/exist.docx");
        let err = result.expect_err("missing file must produce an error");
        assert!(
            matches!(&err, Error::Io { source } if source.kind() == ErrorKind::NotFound),
            "expected Error::Io with ErrorKind::NotFound, got: {err:?}"
        );
    }

    #[test]
    fn docx_reader_from_reader_rejects_non_zip_bytes() {
        let bogus = Cursor::new(b"not a zip archive at all".to_vec());
        let result = DocxReader::from_reader(bogus);
        let err = result.expect_err("non-zip input must produce an error");
        assert_eq!(err.to_string(), "parse error: not a valid ZIP archive");
    }
}
