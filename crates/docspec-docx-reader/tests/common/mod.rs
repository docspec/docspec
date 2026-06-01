#![allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]

use std::io::{Cursor, Write as _};
use zip::{write::SimpleFileOptions, ZipWriter};

/// Builds a minimal valid OPC/DOCX package in memory.
/// Contains only `_rels/.rels` and the main document part.
pub fn build_minimal_docx(main_part_path: &str, main_xml: &str) -> Vec<u8> {
    let rels_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
    Target="{main_part_path}"/>
</Relationships>"#
    );
    build_docx_with_rels(&rels_xml, &[(main_part_path, main_xml.as_bytes())])
}

/// Builds an OPC/DOCX package with custom rels XML and arbitrary parts.
pub fn build_docx_with_rels(rels_xml: &str, parts: &[(&str, &[u8])]) -> Vec<u8> {
    let buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buf);
    let options = SimpleFileOptions::default();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(rels_xml.as_bytes()).unwrap();

    for (path, content) in parts {
        zip.start_file(*path, options).unwrap();
        zip.write_all(content).unwrap();
    }

    zip.finish().unwrap().into_inner()
}
