mod parts;
mod paths;

pub(crate) use parts::{find_document_target, find_numbering_target, find_styles_target};
use paths::normalize_relative_target;
pub(crate) use paths::{derive_part_rels_path, resolve_relative_target};

use std::collections::HashMap;
use std::io::Read;

use docspec_core::Error;
use quick_xml::events::Event;
use quick_xml::XmlVersion;

/// Maps relationship Id (e.g., "rId7") to Target URL/path for every <Relationship> entry whose Type ends with "/hyperlink".
pub(crate) type HyperlinkMap = HashMap<String, String>;

const REL_TYPE_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

/// Represents an image relationship in a DOCX package.
#[derive(Debug)]
pub(crate) struct ImageRel {
    /// Resolved package path for internal images, or raw URL for external ones.
    pub target: String,
    /// `true` when `TargetMode="External"`.
    pub is_external: bool,
}

/// Maps relationship Id (e.g., "rId5") to [`ImageRel`] for every `<Relationship>` entry
/// whose Type ends with "/image".
pub(crate) type ImageMap = HashMap<String, ImageRel>;

pub(crate) fn collect_hyperlink_map<R>(reader: R) -> Result<HyperlinkMap, Error>
where
    R: Read,
{
    let mut xml_reader = quick_xml::Reader::from_reader(std::io::BufReader::new(reader));
    let mut buf = Vec::new();
    let mut element_depth: usize = 0;
    let mut hyperlinks = HashMap::new();

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                element_depth = element_depth.saturating_add(1);
                if element.local_name().as_ref() == b"Relationship" {
                    if let Some((id, target)) = hyperlink_entry(&xml_reader, &element)? {
                        hyperlinks.insert(id, target);
                    }
                }
            }
            Ok(Event::Empty(element)) if element.local_name().as_ref() == b"Relationship" => {
                if let Some((id, target)) = hyperlink_entry(&xml_reader, &element)? {
                    hyperlinks.insert(id, target);
                }
            }
            Ok(Event::End(_)) => {
                let Some(next_depth) = element_depth.checked_sub(1) else {
                    return Err(parse_error("malformed _rels/.rels".to_string()));
                };
                element_depth = next_depth;
            }
            Ok(Event::Eof) => {
                if element_depth != 0 {
                    return Err(parse_error("malformed _rels/.rels".to_string()));
                }
                return Ok(hyperlinks);
            }
            Err(_err) => {
                return Err(parse_error("malformed _rels/.rels".to_string()));
            }
            Ok(_) => {}
        }
        buf.clear();
    }
}

/// Parses a DOCX part relationships XML and returns every image relationship.
///
/// For internal images, resolves the target path relative to `document_path` and
/// validates it contains no `..` traversal segments. For external images
/// (`TargetMode="External"`), stores the raw URL without path resolution.
///
/// Returns `Err` if the XML is malformed or an internal target contains a `..` segment.
pub(crate) fn collect_image_map(rels_xml: &[u8], document_path: &str) -> Result<ImageMap, Error> {
    let mut xml_reader = quick_xml::Reader::from_reader(std::io::BufReader::new(rels_xml));
    let mut buf = Vec::new();
    let mut element_depth: usize = 0;
    let mut images = HashMap::new();

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                element_depth = element_depth.saturating_add(1);
                if element.local_name().as_ref() == b"Relationship" {
                    if let Some((id, rel)) = image_entry(&xml_reader, &element, document_path)? {
                        images.insert(id, rel);
                    }
                }
            }
            Ok(Event::Empty(element)) if element.local_name().as_ref() == b"Relationship" => {
                if let Some((id, rel)) = image_entry(&xml_reader, &element, document_path)? {
                    images.insert(id, rel);
                }
            }
            Ok(Event::End(_)) => {
                let Some(next_depth) = element_depth.checked_sub(1) else {
                    return Err(parse_error("malformed _rels/.rels".to_string()));
                };
                element_depth = next_depth;
            }
            Ok(Event::Eof) => {
                if element_depth != 0 {
                    return Err(parse_error("malformed _rels/.rels".to_string()));
                }
                return Ok(images);
            }
            Err(_err) => {
                return Err(parse_error("malformed _rels/.rels".to_string()));
            }
            Ok(_) => {}
        }
        buf.clear();
    }
}

fn parse_error(message: String) -> Error {
    Error::Parse {
        message,
        position: None,
    }
}

fn hyperlink_entry<R>(
    reader: &quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
) -> Result<Option<(String, String)>, Error>
where
    R: Read,
{
    let mut id = None;
    let mut rel_type = None;
    let mut target = None;

    for attribute_result in element.attributes() {
        let attribute = attribute_result.map_err(|err| Error::Parse {
            message: format!("malformed _rels/.rels: {err}"),
            position: None,
        })?;
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|err| Error::Parse {
                message: format!("malformed _rels/.rels: {err}"),
                position: None,
            })?
            .into_owned();

        match attribute.key.local_name().as_ref() {
            b"Id" => id = Some(value),
            b"Type" => rel_type = Some(value),
            b"Target" => target = Some(value),
            _ => {}
        }
    }

    Ok(match (id, rel_type, target) {
        (Some(found_id), Some(found_type), Some(found_target))
            if found_type.ends_with("/hyperlink") =>
        {
            Some((found_id, found_target))
        }
        _ => None,
    })
}

fn image_entry<R>(
    reader: &quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
    document_path: &str,
) -> Result<Option<(String, ImageRel)>, Error>
where
    R: Read,
{
    let mut id = None;
    let mut rel_type = None;
    let mut target = None;
    let mut target_mode = None;

    for attribute_result in element.attributes() {
        let attribute = attribute_result.map_err(|err| Error::Parse {
            message: format!("malformed _rels/.rels: {err}"),
            position: None,
        })?;
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|err| Error::Parse {
                message: format!("malformed _rels/.rels: {err}"),
                position: None,
            })?
            .into_owned();

        match attribute.key.local_name().as_ref() {
            b"Id" => id = Some(value),
            b"Type" => rel_type = Some(value),
            b"Target" => target = Some(value),
            b"TargetMode" => target_mode = Some(value),
            _ => {}
        }
    }

    match (id, rel_type, target) {
        (Some(found_id), Some(found_type), Some(found_target))
            if found_type == REL_TYPE_IMAGE || found_type.ends_with("/image") =>
        {
            let is_external = target_mode.as_deref() == Some("External");
            if is_external {
                Ok(Some((
                    found_id,
                    ImageRel {
                        target: found_target,
                        is_external: true,
                    },
                )))
            } else {
                let validated = normalize_relative_target(document_path, &found_target)?;
                Ok(Some((
                    found_id,
                    ImageRel {
                        target: validated,
                        is_external: false,
                    },
                )))
            }
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use std::io::Cursor;

    fn assert_hyperlink_map(result: docspec_core::Result<HyperlinkMap>, expected: &[(&str, &str)]) {
        let expected_map = expected
            .iter()
            .map(|(id, target)| ((*id).to_string(), (*target).to_string()))
            .collect::<HyperlinkMap>();

        match result {
            Ok(map) => assert_eq!(map, expected_map),
            Err(err) => assert_eq!(format!("{err:?}"), "expected hyperlink map"),
        }
    }

    #[test]
    fn collect_hyperlink_map_returns_empty_for_empty_relationships() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[]);
    }

    #[test]
    fn collect_hyperlink_map_finds_strict_uri_hyperlink() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://purl.oclc.org/ooxml/officeDocument/relationships/hyperlink" Target="https://example.com"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId1", "https://example.com")]);
    }

    #[test]
    fn collect_hyperlink_map_finds_transitional_uri_hyperlink() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId1", "https://example.com")]);
    }

    #[test]
    fn collect_hyperlink_map_skips_non_hyperlink_relationships() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="word/styles.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[]);
    }

    #[test]
    fn collect_hyperlink_map_collects_multiple_hyperlinks() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/one"/>
  <Relationship Id="rId2" Type="http://purl.oclc.org/ooxml/officeDocument/relationships/hyperlink" Target="https://example.com/two"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/three"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(
            result,
            &[
                ("rId1", "https://example.com/one"),
                ("rId2", "https://example.com/two"),
                ("rId3", "https://example.com/three"),
            ],
        );
    }

    #[test]
    fn collect_hyperlink_map_preserves_id_to_target_mapping() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId7" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://rust-lang.org"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId7", "https://rust-lang.org")]);
    }

    #[test]
    fn collect_hyperlink_map_handles_internal_target_mode() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="other.docx" TargetMode="Internal"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId1", "other.docx")]);
    }

    #[test]
    fn collect_hyperlink_map_handles_external_target_mode() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId1", "https://example.com")]);
    }

    #[test]
    fn collect_hyperlink_map_handles_missing_target_mode() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com"/>
</Relationships>"#;

        let result = collect_hyperlink_map(Cursor::new(rels_xml.as_bytes()));

        assert_hyperlink_map(result, &[("rId1", "https://example.com")]);
    }

    #[test]
    fn collect_hyperlink_map_returns_err_on_malformed_xml() {
        let result = collect_hyperlink_map(Cursor::new("<broken>".as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "malformed _rels/.rels");
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected malformed rels parse error"),
        }
    }

    fn minimal_image_rels(target: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="{target}"/>
</Relationships>"#
        )
    }

    #[test]
    fn collect_image_map_internal() {
        let rels_xml = minimal_image_rels("media/image1.png");

        let result = collect_image_map(rels_xml.as_bytes(), "word/document.xml");

        match result {
            Ok(map) => {
                assert_eq!(map.len(), 1);
                let rel = map.get("rId5").expect("rId5 must be present");
                assert_eq!(rel.target, "word/media/image1.png");
                assert!(!rel.is_external);
            }
            Err(err) => panic!("expected Ok, got {err:?}"),
        }
    }

    #[test]
    fn collect_image_map_external() {
        let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="https://example.com/img.png" TargetMode="External"/>
</Relationships>"#;

        let result = collect_image_map(rels_xml.as_bytes(), "word/document.xml");

        match result {
            Ok(map) => {
                assert_eq!(map.len(), 1);
                let rel = map.get("rId5").expect("rId5 must be present");
                assert_eq!(rel.target, "https://example.com/img.png");
                assert!(rel.is_external);
            }
            Err(err) => panic!("expected Ok, got {err:?}"),
        }
    }

    #[test]
    fn collect_image_map_rejects_dotdot_escaping_root() {
        let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../../../etc/passwd"/>
</Relationships>"#;

        let result = collect_image_map(rels_xml.as_bytes(), "word/document.xml");

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "rels target escapes package root: word/../../../etc/passwd"
                );
                assert_eq!(position, None);
            }
            other => panic!("expected Err(Parse), got {other:?}"),
        }
    }

    #[test]
    fn collect_image_map_normalizes_parent_segment_into_package_root() {
        let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>
</Relationships>"#;

        let result = collect_image_map(rels_xml.as_bytes(), "word/document.xml");

        match result {
            Ok(map) => {
                assert_eq!(map.len(), 1);
                let rel = map.get("rId5").expect("rId5 must be present");
                assert_eq!(rel.target, "media/image1.png");
                assert!(!rel.is_external);
            }
            Err(err) => panic!("expected Ok, got {err:?}"),
        }
    }

    #[test]
    fn collect_image_map_collapses_dot_and_double_dot_segments() {
        let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="./media/sub/../image1.png"/>
</Relationships>"#;

        let result = collect_image_map(rels_xml.as_bytes(), "word/document.xml");

        match result {
            Ok(map) => {
                let rel = map.get("rId5").expect("rId5 must be present");
                assert_eq!(rel.target, "word/media/image1.png");
                assert!(!rel.is_external);
            }
            Err(err) => panic!("expected Ok, got {err:?}"),
        }
    }
}
