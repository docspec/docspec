//! Smoke tests for in-memory ZIP fixture builder.
#![allow(clippy::expect_used, clippy::unwrap_used)]

mod common;

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use zip::ZipArchive;

    use super::common;

    #[test]
    fn builder_produces_valid_zip_with_two_entries() {
        let bytes = common::build_minimal_docx(
            "word/document.xml",
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#,
        );
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), 2);
        assert!(archive.by_name("_rels/.rels").is_ok());
        assert!(archive.by_name("word/document.xml").is_ok());
    }
}
