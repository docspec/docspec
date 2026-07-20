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

pub fn find_document_target<R>(reader: R) -> docspec_core::Result<String>
where
    R: Read,
{
    let mut xml_reader = quick_xml::Reader::from_reader(std::io::BufReader::new(reader));
    let mut buf = Vec::new();
    let mut element_depth: usize = 0;

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                element_depth = element_depth.saturating_add(1);
                if element.local_name().as_ref() == b"Relationship" {
                    if let Some(target) = office_document_target(&xml_reader, &element)? {
                        let document_path = target.strip_prefix('/').unwrap_or(&target).to_string();
                        return validate_document_path(&document_path);
                    }
                }
            }
            Ok(Event::Empty(element)) if element.local_name().as_ref() == b"Relationship" => {
                if let Some(target) = office_document_target(&xml_reader, &element)? {
                    let document_path = target.strip_prefix('/').unwrap_or(&target).to_string();
                    return validate_document_path(&document_path);
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
                return Err(Error::Parse {
                    message: "no officeDocument relationship".to_string(),
                    position: None,
                });
            }
            Err(_err) => {
                return Err(parse_error("malformed _rels/.rels".to_string()));
            }
            Ok(_) => {}
        }
        buf.clear();
    }
}

/// Finds the styles part target from a part relationships file.
///
/// Returns `Ok(None)` if no `/styles` relationship is present (legal per ECMA-376 §11.3.12).
/// Returns `Err` if the target contains a path-traversal (`..`) segment.
/// Ignores relationships with `TargetMode="External"`.
pub fn find_styles_target<R>(reader: R) -> docspec_core::Result<Option<String>>
where
    R: Read,
{
    let mut xml_reader = quick_xml::Reader::from_reader(std::io::BufReader::new(reader));
    let mut buf = Vec::new();
    let mut element_depth: usize = 0;

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                element_depth = element_depth.saturating_add(1);
                if element.local_name().as_ref() == b"Relationship" {
                    if let Some(target) = styles_target(&xml_reader, &element)? {
                        let styles_path = target.strip_prefix('/').unwrap_or(&target).to_string();
                        return validate_document_path(&styles_path).map(Some);
                    }
                }
            }
            Ok(Event::Empty(element)) if element.local_name().as_ref() == b"Relationship" => {
                if let Some(target) = styles_target(&xml_reader, &element)? {
                    let styles_path = target.strip_prefix('/').unwrap_or(&target).to_string();
                    return validate_document_path(&styles_path).map(Some);
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
                return Ok(None);
            }
            Err(_err) => {
                return Err(parse_error("malformed _rels/.rels".to_string()));
            }
            Ok(_) => {}
        }
        buf.clear();
    }
}

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

/// Finds the numbering part target from a part relationships file.
///
/// Returns `Ok(None)` if no `/numbering` relationship is present (legal per ECMA-376 §17.9).
/// Returns `Err` if the target contains a path-traversal (`..`) segment.
/// Ignores relationships with `TargetMode="External"`.
pub fn find_numbering_target<R>(reader: R) -> docspec_core::Result<Option<String>>
where
    R: Read,
{
    let mut xml_reader = quick_xml::Reader::from_reader(std::io::BufReader::new(reader));
    let mut buf = Vec::new();
    let mut element_depth: usize = 0;

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                element_depth = element_depth.saturating_add(1);
                if element.local_name().as_ref() == b"Relationship" {
                    if let Some(target) = numbering_target(&xml_reader, &element)? {
                        let numbering_path =
                            target.strip_prefix('/').unwrap_or(&target).to_string();
                        return validate_document_path(&numbering_path).map(Some);
                    }
                }
            }
            Ok(Event::Empty(element)) if element.local_name().as_ref() == b"Relationship" => {
                if let Some(target) = numbering_target(&xml_reader, &element)? {
                    let numbering_path = target.strip_prefix('/').unwrap_or(&target).to_string();
                    return validate_document_path(&numbering_path).map(Some);
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
                return Ok(None);
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

fn validate_document_path(document_path: &str) -> docspec_core::Result<String> {
    if document_path.split('/').any(|component| component == "..") {
        return Err(Error::Parse {
            message: format!("rels target contains parent reference: {document_path}"),
            position: None,
        });
    }

    Ok(document_path.to_string())
}

fn office_document_target<R>(
    reader: &quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
) -> docspec_core::Result<Option<String>>
where
    R: Read,
{
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
            b"Type" => rel_type = Some(value),
            b"Target" => target = Some(value),
            _ => {}
        }
    }

    Ok(match (rel_type, target) {
        (Some(found_type), Some(found_target)) if found_type.ends_with("/officeDocument") => {
            Some(found_target)
        }
        _ => None,
    })
}

fn styles_target<R>(
    reader: &quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
) -> docspec_core::Result<Option<String>>
where
    R: Read,
{
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
            b"Type" => rel_type = Some(value),
            b"Target" => target = Some(value),
            b"TargetMode" => target_mode = Some(value),
            _ => {}
        }
    }

    Ok(match (rel_type, target, target_mode) {
        (Some(found_type), Some(_), Some(mode))
            if found_type.ends_with("/styles") && mode == "External" =>
        {
            None
        }
        (Some(found_type), Some(found_target), _) if found_type.ends_with("/styles") => {
            Some(found_target)
        }
        _ => None,
    })
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

fn numbering_target<R>(
    reader: &quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
) -> docspec_core::Result<Option<String>>
where
    R: Read,
{
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
            b"Type" => rel_type = Some(value),
            b"Target" => target = Some(value),
            b"TargetMode" => target_mode = Some(value),
            _ => {}
        }
    }

    Ok(match (rel_type, target, target_mode) {
        (Some(found_type), Some(_), Some(mode))
            if found_type.ends_with("/numbering") && mode == "External" =>
        {
            None
        }
        (Some(found_type), Some(found_target), _) if found_type.ends_with("/numbering") => {
            Some(found_target)
        }
        _ => None,
    })
}

/// Derives the part relationships file path from a part path.
///
/// Per ECMA-376 Part 2 §6.5.2.3:
/// - `"word/document.xml"` → `"word/_rels/document.xml.rels"`
/// - `"foo"` (no directory) → `"_rels/foo.rels"`
pub fn derive_part_rels_path(part_path: &str) -> String {
    part_path.rfind('/').map_or_else(
        || format!("_rels/{part_path}.rels"),
        |slash_pos| {
            let (dir, file_with_slash) = part_path.split_at(slash_pos);
            let file = file_with_slash.strip_prefix('/').unwrap_or_default();
            format!("{dir}/_rels/{file}.rels")
        },
    )
}

/// Resolves a relative target against a base part path.
///
/// Per ECMA-376 Part 2 §6.5.3.4: strip the filename from `base_part`,
/// then join with `target`. Leading `/` in `target` is stripped (absolute paths
/// within the package are treated as relative to the root).
pub fn resolve_relative_target(base_part: &str, target: &str) -> String {
    let target_stripped = target.strip_prefix('/').unwrap_or(target);

    if target.starts_with('/') {
        target_stripped.to_string()
    } else if let Some(slash_pos) = base_part.rfind('/') {
        let (base_dir, _) = base_part.split_at(slash_pos.saturating_add(1));
        format!("{base_dir}{target_stripped}")
    } else {
        target_stripped.to_string()
    }
}

/// Resolves a relative target against `base_part` and canonicalizes `.` / `..`
/// segments per ECMA-376 Part 2 §6.5.3 / RFC 3986 §5.2.
///
/// Word and other DOCX writers routinely emit `Target="../media/image1.png"`
/// or `Target="../customXml/item1.xml"` from `word/_rels/document.xml.rels`;
/// rejecting `..` outright (the old behavior of [`validate_document_path`])
/// breaks reading those files. This helper collapses the segments instead,
/// while still refusing references that escape the package root — a `..`
/// segment that pops past the start of the resolved path returns
/// [`docspec_core::Error::Parse`].
fn normalize_relative_target(base_part: &str, target: &str) -> docspec_core::Result<String> {
    let joined = resolve_relative_target(base_part, target);
    let mut normalized: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if normalized.pop().is_none() {
                    return Err(parse_error(format!(
                        "rels target escapes package root: {joined}"
                    )));
                }
            }
            other => normalized.push(other),
        }
    }
    Ok(normalized.join("/"))
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use std::io::Cursor;

    fn minimal_rels(target: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{target}"/>
</Relationships>"#
        )
    }

    fn assert_document_path(result: docspec_core::Result<String>, expected: &str) {
        match result {
            Ok(path) => assert_eq!(path, expected),
            Err(err) => assert_eq!(format!("{err:?}"), "expected document path"),
        }
    }

    #[test]
    fn find_document_target_returns_target_for_simple_rels() {
        let rels_xml = minimal_rels("word/document.xml");

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_target_errors_when_no_office_document_relationship() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://example.com/not-office" Target="word/document.xml"/>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "no officeDocument relationship");
                assert_eq!(position, None);
            }
            other => assert_eq!(
                format!("{other:?}"),
                "expected no officeDocument parse error"
            ),
        }
    }

    #[test]
    fn find_document_target_errors_after_balanced_nested_non_matching_rels() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Group><Relationship Id="rId1" Type="http://example.com/not-office" Target="word/document.xml"></Relationship></Group>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "no officeDocument relationship");
                assert_eq!(position, None);
            }
            other => assert_eq!(
                format!("{other:?}"),
                "expected balanced traversal parse error"
            ),
        }
    }

    #[test]
    fn find_document_target_errors_on_unexpected_closing_element() {
        let result = find_document_target(Cursor::new("</Relationships>".as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "malformed _rels/.rels");
                assert_eq!(position, None);
            }
            other => assert_eq!(
                format!("{other:?}"),
                "expected unexpected closing element parse error"
            ),
        }
    }

    #[test]
    fn find_document_target_errors_on_rels_xml_parser_error() {
        let result = find_document_target(Cursor::new("<Relationships><".as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "malformed _rels/.rels");
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected rels parser error"),
        }
    }

    #[test]
    fn find_document_target_strips_leading_slash() {
        let rels_xml = minimal_rels("/word/document.xml");

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_target_picks_office_document_among_multiple_relationships() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://example.com/metadata" Target="docProps/core.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId3" Type="http://example.com/theme" Target="word/theme/theme1.xml"/>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_target_accepts_non_empty_relationship_element() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"></Relationship>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_target_errors_on_malformed_relationship_attribute() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target=word/document.xml/>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "malformed _rels/.rels: position 120: attribute value must be enclosed in `\"` or `'`"
                );
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected attribute parse error"),
        }
    }

    #[test]
    fn find_document_target_errors_on_bad_attribute_entity() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/&bogus;.xml"/>
</Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "malformed _rels/.rels: at 6..11: unrecognized entity `bogus`"
                );
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected entity parse error"),
        }
    }

    #[test]
    fn find_document_target_tolerates_namespaced_relationship_element() {
        let rels_xml = r#"<r:Relationships xmlns:r="http://schemas.openxmlformats.org/package/2006/relationships">
  <r:Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</r:Relationships>"#;

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        assert_document_path(result, "word/document.xml");
    }

    #[test]
    fn find_document_target_errors_on_malformed_rels_xml() {
        let result = find_document_target(Cursor::new("<broken>".as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(message, "malformed _rels/.rels");
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected malformed rels parse error"),
        }
    }

    #[test]
    fn find_document_target_rejects_target_with_dotdot_segment() {
        let rels_xml = minimal_rels("../foo/document.xml");

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "rels target contains parent reference: ../foo/document.xml"
                );
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected dotdot parse error"),
        }
    }

    #[test]
    fn find_document_target_handles_target_with_entities() {
        let rels_xml = minimal_rels("word/doc&amp;ument.xml");

        let result = find_document_target(Cursor::new(rels_xml.as_bytes()));
        assert_document_path(result, "word/doc&ument.xml");
    }

    fn minimal_styles_rels(target: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="{target}"/>
</Relationships>"#
        )
    }

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

    #[test]
    fn find_styles_returns_target_when_present() {
        let rels_xml = minimal_styles_rels("word/styles.xml");

        let result = find_styles_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Ok(Some(path)) => assert_eq!(path, "word/styles.xml"),
            other => assert_eq!(format!("{other:?}"), "expected Some(word/styles.xml)"),
        }
    }

    #[test]
    fn find_styles_returns_none_when_absent() {
        let rels_xml = minimal_rels("word/document.xml");

        let result = find_styles_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Ok(None) => {}
            other => assert_eq!(format!("{other:?}"), "expected Ok(None)"),
        }
    }

    #[test]
    fn find_styles_rejects_dotdot() {
        let rels_xml = minimal_styles_rels("../etc/passwd");

        let result = find_styles_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "rels target contains parent reference: ../etc/passwd"
                );
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected dotdot parse error"),
        }
    }

    #[test]
    fn find_styles_ignores_external_target_mode() {
        let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="word/styles.xml" TargetMode="External"/>
</Relationships>"#;

        let result = find_styles_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Ok(None) => {}
            other => assert_eq!(format!("{other:?}"), "expected Ok(None) for external"),
        }
    }

    #[test]
    fn find_styles_returns_first_styles_match() {
        let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
   <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="word/styles.xml"/>
   <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="word/styles2.xml"/>
</Relationships>"#;

        let result = find_styles_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Ok(Some(path)) => assert_eq!(path, "word/styles.xml"),
            other => assert_eq!(format!("{other:?}"), "expected first match"),
        }
    }

    fn minimal_numbering_rels(target: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
   <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="{target}"/>
</Relationships>"#
        )
    }

    #[test]
    fn find_numbering_target_returns_target_when_present() {
        let rels_xml = minimal_numbering_rels("word/numbering.xml");

        let result = find_numbering_target(Cursor::new(rels_xml.as_bytes()));

        assert_eq!(result.ok(), Some(Some("word/numbering.xml".to_string())));
    }

    #[test]
    fn find_numbering_target_returns_none_when_absent() {
        let rels_xml = minimal_rels("word/document.xml");

        let result = find_numbering_target(Cursor::new(rels_xml.as_bytes()));

        assert_eq!(result.ok(), Some(None));
    }

    #[test]
    fn find_numbering_target_picks_numbering_among_multiple_relationships() {
        let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
   <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="word/styles.xml"/>
   <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="word/numbering.xml"/>
   <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable" Target="word/fontTable.xml"/>
</Relationships>"#;

        let result = find_numbering_target(Cursor::new(rels_xml.as_bytes()));

        assert_eq!(result.ok(), Some(Some("word/numbering.xml".to_string())));
    }

    #[test]
    fn find_numbering_target_rejects_target_with_dotdot_segment() {
        let rels_xml = minimal_numbering_rels("../etc/passwd");

        let result = find_numbering_target(Cursor::new(rels_xml.as_bytes()));

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "rels target contains parent reference: ../etc/passwd"
                );
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected dotdot parse error"),
        }
    }

    #[test]
    fn find_numbering_target_returns_none_on_empty_rels() {
        let rels_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#;

        let result = find_numbering_target(Cursor::new(rels_xml.as_bytes()));

        assert_eq!(result.ok(), Some(None));
    }

    #[test]
    fn derive_part_rels_path_with_directory() {
        let result = derive_part_rels_path("word/document.xml");
        assert_eq!(result, "word/_rels/document.xml.rels");
    }

    #[test]
    fn derive_part_rels_path_without_directory() {
        let result = derive_part_rels_path("foo");
        assert_eq!(result, "_rels/foo.rels");
    }

    #[test]
    fn resolve_relative_target_with_directory() {
        let result = resolve_relative_target("word/document.xml", "styles.xml");
        assert_eq!(result, "word/styles.xml");
    }

    #[test]
    fn resolve_relative_target_strips_leading_slash() {
        let result = resolve_relative_target("word/document.xml", "/word/styles.xml");
        assert_eq!(result, "word/styles.xml");
    }

    #[test]
    fn resolve_relative_target_without_directory() {
        let result = resolve_relative_target("document.xml", "styles.xml");
        assert_eq!(result, "styles.xml");
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
