#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Skeleton integration tests for `DocxReader`.

#[cfg(test)]
mod tests {
    use docspec_docx_reader::DocxReader;
    use std::io::Cursor;

    #[test]
    fn returns_err_on_empty() {
        let result = DocxReader::new(Cursor::new(Vec::<u8>::new()));
        assert!(result.is_err());
    }
}
