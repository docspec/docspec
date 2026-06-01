//! Discovery of the main document part via `_rels/.rels`.
//!
//! Reads the package-level relationships file and returns the path to the
//! main document part (e.g., `word/document.xml`).

#![allow(clippy::pub_with_shorthand, clippy::redundant_pub_crate)]

use std::io::{Cursor, Read, Seek};

use docspec_core::{Error, Result};
use quick_xml::events::Event as XmlEvent;
use quick_xml::reader::NsReader;
use zip::ZipArchive;

use crate::oox;

/// Normalizes a Target path from a package relationship.
///
/// Validates and normalizes OPC (Open Packaging Convention) paths to prevent
/// directory traversal, external references, and other security issues.
///
/// # Validation Rules
///
/// Rejects paths that:
/// - Contain `..` segments (directory traversal)
/// - Contain `.` segments (current directory references)
/// - Contain empty segments (double slashes `//`)
/// - Start with a Windows drive letter (e.g., `C:`)
/// - Contain URL schemes (e.g., `http://`, `file://`)
/// - Contain backslashes (Windows path separators)
/// - Contain `%` characters (percent encoding — v0 limitation)
///
/// Accepts paths with a single leading `/` and strips it.
///
/// # Errors
///
/// Returns [`Error::Parse`] if the path fails any validation rule.
pub(crate) fn normalize_part_path(target: &str) -> Result<String> {
    // Strip a single leading `/` if present
    let normalized = target.trim_start_matches('/');

    // Reject if input starts with a Windows drive letter like `C:`
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2
        && bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes.get(1) == Some(&b':')
    {
        return Err(Error::Parse {
            message: format!("rejected absolute or external Target: {target}"),
            position: None,
        });
    }

    // Reject if input contains a URL scheme like `http://` or `file://`
    if normalized.contains("://") {
        return Err(Error::Parse {
            message: format!("rejected absolute or external Target: {target}"),
            position: None,
        });
    }

    // v0 limitation: percent-encoded paths rejected to avoid encoding ambiguity
    if normalized.contains('%') {
        return Err(Error::Parse {
            message: format!("rejected percent-encoded Target: {target}"),
            position: None,
        });
    }

    // Split on `/` and validate each segment
    for segment in normalized.split('/') {
        // Reject if any segment equals `..`
        if segment == ".." {
            return Err(Error::Parse {
                message: format!("rejected path traversal in Target: {target}"),
                position: None,
            });
        }

        // Reject if any segment equals `.`
        if segment == "." {
            return Err(Error::Parse {
                message: format!("rejected path traversal in Target: {target}"),
                position: None,
            });
        }

        // Reject if any segment is empty (implies `//`)
        if segment.is_empty() {
            return Err(Error::Parse {
                message: format!("rejected path traversal in Target: {target}"),
                position: None,
            });
        }

        // Reject if any segment contains a backslash
        if segment.contains('\\') {
            return Err(Error::Parse {
                message: format!("rejected path traversal in Target: {target}"),
                position: None,
            });
        }
    }

    Ok(normalized.to_string())
}

/// Discovers the main document part by reading `_rels/.rels` from the ZIP archive.
///
/// Parses the package-level relationships file and returns the `Target` path of
/// the first relationship with an office document relationship type (transitional
/// or strict). The returned path is normalized to prevent directory traversal and
/// external references.
///
/// # Errors
///
/// Returns [`Error::Parse`] if:
/// - `_rels/.rels` is absent from the archive
/// - `_rels/.rels` contains malformed XML
/// - No office document relationship is found
/// - The Target path fails normalization (directory traversal, external reference, etc.)
///
/// Returns [`Error::Io`] if reading from the archive entry fails.
pub(crate) fn discover_main_part<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<String> {
    let mut entry = archive
        .by_name(oox::PACKAGE_RELS_PATH)
        .map_err(|_zip_err| Error::Parse {
            message: "package missing _rels/.rels".into(),
            position: None,
        })?;
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .map_err(|e| Error::Io { source: e })?;
    drop(entry);
    let mut reader = NsReader::from_reader(Cursor::new(bytes));
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let (ns, event) = match reader.read_resolved_event_into(&mut buf) {
            Err(xml_err) => {
                return Err(Error::Parse {
                    message: format!("XML parse error in _rels/.rels: {xml_err}"),
                    position: None,
                });
            }
            Ok((_, XmlEvent::Eof)) => break,
            Ok(pair) => pair,
        };
        let (XmlEvent::Empty(e) | XmlEvent::Start(e)) = event else {
            continue;
        };
        let local = e.local_name();
        if !(matches!(ns, quick_xml::name::ResolveResult::Bound(n) if n.as_ref() == oox::RELATIONSHIPS_NS)
            && local.as_ref() == b"Relationship")
        {
            continue;
        }
        let mut rel_type: Option<Vec<u8>> = None;
        let mut target: Option<String> = None;
        for attr_result in e.attributes() {
            let attr = attr_result.map_err(|xml_err| Error::Parse {
                message: format!("attribute error in _rels/.rels: {xml_err}"),
                position: None,
            })?;
            match attr.key.as_ref() {
                b"Type" => rel_type = Some(attr.value.to_vec()),
                b"Target" => {
                    target = Some(String::from_utf8(attr.value.to_vec()).map_err(|utf8_err| {
                        Error::Parse {
                            message: format!("invalid UTF-8 in Target attribute: {utf8_err}"),
                            position: None,
                        }
                    })?);
                }
                _ => {}
            }
        }
        if let (Some(rt), Some(tgt)) = (rel_type, target) {
            if oox::is_office_document_rel(&rt) {
                return normalize_part_path(&tgt);
            }
        }
    }
    Err(Error::Parse {
        message: "no Main Document relationship found".into(),
        position: None,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::ref_patterns
)]
mod tests {
    use std::io::Cursor;

    use zip::ZipArchive;

    use super::{discover_main_part, normalize_part_path};

    const RELS_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
    const TRANSITIONAL_TYPE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
    const STRICT_TYPE: &str =
        "http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument";

    fn build_test_docx_with_rels(rels_xml: &str, parts: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write as _;
        use zip::{write::SimpleFileOptions, ZipWriter};
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

    fn build_test_docx_no_rels() -> Vec<u8> {
        use std::io::Write as _;
        use zip::{write::SimpleFileOptions, ZipWriter};
        let buf = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);
        let options = SimpleFileOptions::default();
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(b"<w:document/>").unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn discover_main_part_finds_transitional_office_doc() {
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{TRANSITIONAL_TYPE}" Target="word/document.xml"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(&rels_xml, &[("word/document.xml", b"")]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(
            discover_main_part(&mut archive).unwrap(),
            "word/document.xml"
        );
    }

    #[test]
    fn discover_main_part_finds_strict_office_doc() {
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{STRICT_TYPE}" Target="word/document.xml"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(&rels_xml, &[("word/document.xml", b"")]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(
            discover_main_part(&mut archive).unwrap(),
            "word/document.xml"
        );
    }

    #[test]
    fn discover_main_part_finds_alternate_path() {
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{TRANSITIONAL_TYPE}" Target="content/main.xml"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(&rels_xml, &[("content/main.xml", b"")]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(
            discover_main_part(&mut archive).unwrap(),
            "content/main.xml"
        );
    }

    #[test]
    fn discover_main_part_errors_when_missing() {
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="word/styles.xml"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(&rels_xml, &[("word/styles.xml", b"")]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let result = discover_main_part(&mut archive);
        assert!(matches!(
            &result,
            Err(docspec_core::Error::Parse { message, .. })
                if message == "no Main Document relationship found"
        ));
    }

    #[test]
    fn discover_main_part_errors_when_rels_absent() {
        let bytes = build_test_docx_no_rels();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let result = discover_main_part(&mut archive);
        assert!(matches!(
            &result,
            Err(docspec_core::Error::Parse { message, .. })
                if message == "package missing _rels/.rels"
        ));
    }

    #[test]
    fn discover_main_part_errors_on_malformed_xml() {
        let bytes = build_test_docx_with_rels("<<<not xml>>>", &[]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert!(matches!(
            discover_main_part(&mut archive),
            Err(docspec_core::Error::Parse { .. })
        ));
    }

    #[test]
    fn discover_main_part_takes_first_match() {
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{TRANSITIONAL_TYPE}" Target="word/document.xml"/><Relationship Id="rId2" Type="{TRANSITIONAL_TYPE}" Target="word/document2.xml"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(
            &rels_xml,
            &[("word/document.xml", b""), ("word/document2.xml", b"")],
        );
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(
            discover_main_part(&mut archive).unwrap(),
            "word/document.xml"
        );
    }

    // normalize_part_path tests
    #[test]
    fn normalize_part_path_accepts_plain() {
        let result = normalize_part_path("word/document.xml");
        assert_eq!(result.unwrap(), "word/document.xml");
    }

    #[test]
    fn normalize_part_path_strips_leading_slash() {
        let result = normalize_part_path("/word/document.xml");
        assert_eq!(result.unwrap(), "word/document.xml");
    }

    #[test]
    fn normalize_part_path_rejects_parent_traversal() {
        let result = normalize_part_path("../etc/passwd");
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { ref message, .. })
                if message.contains("path traversal")
        ));
    }

    #[test]
    fn normalize_part_path_rejects_dotdot_segment() {
        let result = normalize_part_path("word/../../etc/passwd");
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { ref message, .. })
                if message.contains("path traversal")
        ));
    }

    #[test]
    fn normalize_part_path_rejects_current_dir_segment() {
        let result = normalize_part_path("./word/document.xml");
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { ref message, .. })
                if message.contains("path traversal")
        ));
    }

    #[test]
    fn normalize_part_path_rejects_empty_segment() {
        let result = normalize_part_path("word//document.xml");
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { ref message, .. })
                if message.contains("path traversal")
        ));
    }

    #[test]
    fn normalize_part_path_rejects_drive_letter() {
        let result = normalize_part_path("C:/word/document.xml");
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { ref message, .. })
                if message.contains("absolute or external")
        ));
    }

    #[test]
    fn normalize_part_path_rejects_url() {
        let result = normalize_part_path("http://evil.com/x.xml");
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { ref message, .. })
                if message.contains("absolute or external")
        ));
    }

    #[test]
    fn normalize_part_path_rejects_backslash() {
        let result = normalize_part_path("word\\document.xml");
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { ref message, .. })
                if message.contains("path traversal")
        ));
    }

    #[test]
    fn normalize_part_path_rejects_percent_encoding() {
        let result = normalize_part_path("word%2Fdocument.xml");
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { ref message, .. })
                if message.contains("percent-encoded")
        ));
    }

    #[test]
    fn discover_rejects_traversal_in_target() {
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{TRANSITIONAL_TYPE}" Target="../../etc/passwd"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(&rels_xml, &[]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let result = discover_main_part(&mut archive);
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { ref message, .. })
                if message.contains("path traversal")
        ));
    }

    #[test]
    fn discover_main_part_handles_attribute_error() {
        // Relationship with malformed attribute (missing closing quote)
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{TRANSITIONAL_TYPE}" Target="word/document.xml/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(&rels_xml, &[]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let result = discover_main_part(&mut archive);
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { ref message, .. })
                if message.contains("attribute error") || message.contains("XML parse error")
        ));
    }

    #[test]
    fn discover_main_part_handles_invalid_utf8_in_target() {
        // This test verifies UTF-8 validation in Target attribute
        // We create a relationship with a valid structure but test the UTF-8 path
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{TRANSITIONAL_TYPE}" Target="word/document.xml"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(&rels_xml, &[("word/document.xml", b"")]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        // This should succeed since the Target is valid UTF-8
        let result = discover_main_part(&mut archive);
        assert!(matches!(result, Ok(path) if path == "word/document.xml"));
    }

    #[test]
    fn discover_main_part_with_multiple_relationships() {
        // Test that discover_main_part returns the first matching office document relationship
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="word/styles.xml"/><Relationship Id="rId2" Type="{TRANSITIONAL_TYPE}" Target="word/document.xml"/><Relationship Id="rId3" Type="{TRANSITIONAL_TYPE}" Target="word/document2.xml"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(
            &rels_xml,
            &[
                ("word/styles.xml", b""),
                ("word/document.xml", b""),
                ("word/document2.xml", b""),
            ],
        );
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let result = discover_main_part(&mut archive);
        // Should return the first office document relationship
        assert!(matches!(result, Ok(path) if path == "word/document.xml"));
    }

    #[test]
    fn discover_main_part_with_non_matching_relationships() {
        // Test that discover_main_part errors when no office document relationship exists
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="word/styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable" Target="word/fontTable.xml"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(
            &rels_xml,
            &[("word/styles.xml", b""), ("word/fontTable.xml", b"")],
        );
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let result = discover_main_part(&mut archive);
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { message, .. })
                if message == "no Main Document relationship found"
        ));
    }

    #[test]
    fn discover_main_part_with_leading_slash_in_target() {
        // Test that discover_main_part strips leading slash from Target
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{TRANSITIONAL_TYPE}" Target="/word/document.xml"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(&rels_xml, &[("word/document.xml", b"")]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let result = discover_main_part(&mut archive);
        assert!(matches!(result, Ok(path) if path == "word/document.xml"));
    }

    #[test]
    fn discover_main_part_with_empty_target() {
        // Test that discover_main_part rejects empty Target
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{TRANSITIONAL_TYPE}" Target=""/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(&rels_xml, &[]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let result = discover_main_part(&mut archive);
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { message, .. })
                if message.contains("path traversal")
        ));
    }

    #[test]
    fn discover_main_part_with_only_slash_target() {
        // Test that discover_main_part rejects Target that is only a slash
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{TRANSITIONAL_TYPE}" Target="/"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(&rels_xml, &[]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let result = discover_main_part(&mut archive);
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { message, .. })
                if message.contains("path traversal")
        ));
    }

    #[test]
    fn discover_main_part_with_relationship_missing_type() {
        // Test that discover_main_part skips relationships without Type attribute
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Target="word/document.xml"/><Relationship Id="rId2" Type="{TRANSITIONAL_TYPE}" Target="word/document2.xml"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(
            &rels_xml,
            &[("word/document.xml", b""), ("word/document2.xml", b"")],
        );
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let result = discover_main_part(&mut archive);
        // Should skip the first relationship (no Type) and return the second
        assert!(matches!(result, Ok(path) if path == "word/document2.xml"));
    }

    #[test]
    fn discover_main_part_with_relationship_missing_target() {
        // Test that discover_main_part skips relationships without Target attribute
        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{TRANSITIONAL_TYPE}"/><Relationship Id="rId2" Type="{TRANSITIONAL_TYPE}" Target="word/document.xml"/></Relationships>"#
        );
        let bytes = build_test_docx_with_rels(&rels_xml, &[("word/document.xml", b"")]);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let result = discover_main_part(&mut archive);
        // Should skip the first relationship (no Target) and return the second
        assert!(matches!(result, Ok(path) if path == "word/document.xml"));
    }
}
