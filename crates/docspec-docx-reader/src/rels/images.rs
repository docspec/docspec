//! Image relationship collection from DOCX relationship parts.

use super::paths::normalize_relative_target;
use super::{parse_error, ImageMap, ImageRel};
use std::collections::HashMap;
use std::io::Read;

use docspec_core::Error;
use quick_xml::{events::Event, XmlVersion};

const REL_TYPE_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

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

#[cfg(all(test, coverage))]
mod coverage_tests {
    use super::*;

    fn assert_parse_error(result: docspec_core::Result<ImageMap>, expected_message: &str) {
        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, expected_message);
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected relationship parse error"),
        }
    }

    #[test]
    fn collect_image_map_accepts_non_empty_relationship_element() {
        let rels_xml = r#"<Relationships>
  <Relationship Id="rId1" Type="http://example.com/image" Target="media/image.png" Extra="ignored"></Relationship>
</Relationships>"#;

        let result = collect_image_map(rels_xml.as_bytes(), "word/document.xml");

        match result {
            Ok(map) => assert_eq!(
                map.get("rId1")
                    .map(|rel| (rel.target.as_str(), rel.is_external)),
                Some(("word/media/image.png", false))
            ),
            Err(err) => assert_eq!(format!("{err:?}"), "expected image map"),
        }
    }

    #[test]
    fn collect_image_map_rejects_unexpected_closing_element() {
        let result = collect_image_map("</Relationships>".as_bytes(), "word/document.xml");

        assert_parse_error(result, "malformed _rels/.rels");
    }

    #[test]
    fn collect_image_map_rejects_unclosed_relationships_element() {
        let result = collect_image_map("<Relationships>".as_bytes(), "word/document.xml");

        assert_parse_error(result, "malformed _rels/.rels");
    }

    #[test]
    fn collect_image_map_rejects_xml_parser_error() {
        let result = collect_image_map("<Relationships><".as_bytes(), "word/document.xml");

        assert_parse_error(result, "malformed _rels/.rels");
    }

    #[test]
    fn collect_image_map_rejects_malformed_relationship_attribute() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target=word/document.xml/>
</Relationships>"#;

        let result = collect_image_map(rels_xml.as_bytes(), "word/document.xml");

        assert_parse_error(
            result,
            "malformed _rels/.rels: position 120: attribute value must be enclosed in `\"` or `'`",
        );
    }

    #[test]
    fn collect_image_map_rejects_bad_attribute_entity() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/&bogus;.xml"/>
</Relationships>"#;

        let result = collect_image_map(rels_xml.as_bytes(), "word/document.xml");

        assert_parse_error(
            result,
            "malformed _rels/.rels: at 6..11: unrecognized entity `bogus`",
        );
    }
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

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
