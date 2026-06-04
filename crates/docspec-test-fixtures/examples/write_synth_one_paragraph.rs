//! Example: write a synthetic one-paragraph DOCX to a file.

use docspec_test_fixtures::synth_docx;
use std::env;
use std::fs;
use std::io::Result;

fn main() -> Result<()> {
    let output_path = env::args()
        .nth(1)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing output path"))?;

    let rels_xml = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let document_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body></w:document>"#;

    let bytes = synth_docx(rels_xml, document_xml);
    fs::write(&output_path, bytes)?;
    eprintln!("Wrote synth DOCX to {output_path}");
    Ok(())
}
