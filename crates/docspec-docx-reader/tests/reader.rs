//! Integration tests for `DocxReader`.
#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

mod fixture;

#[test]
fn synth_docx_roundtrips_through_zip_archive() {
    use std::io::Cursor;
    use zip::ZipArchive;

    let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;
    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body>
</w:document>"#;

    let bytes = fixture::synth_docx(rels_xml, document_xml);
    let cursor = Cursor::new(bytes);
    let archive = ZipArchive::new(cursor).expect("should be valid ZIP");
    assert_eq!(
        archive.len(),
        2,
        "expected exactly 2 entries: _rels/.rels and word/document.xml"
    );
}
