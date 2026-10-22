//! Relationship (`.rels`) part parsing.
//!
//! A rels part is read once into a list of [`Relationship`] values; the
//! callers then pick what they need from that list. Every internal target is
//! resolved the same way, through [`normalize_relative_target`]: joined with
//! the owning part's directory, `.` and `..` segments collapsed, and only a
//! `..` that would leave the package root is rejected.

use std::collections::HashMap;
use std::io::Read;

use docspec_core::Error;
use quick_xml::events::{BytesStart, Event};

use crate::document::xml::{attr_string, parse_error};

/// Error prefix for every malformed relationship part, root or document-level.
const RELS_ERROR_CONTEXT: &str = "malformed _rels/.rels";

/// Maps relationship Id (e.g., "rId7") to Target URL/path for every <Relationship> entry whose Type ends with "/hyperlink".
pub(crate) type HyperlinkMap = HashMap<String, String>;

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

/// One `<Relationship>` element that carries both a `Type` and a `Target`.
struct Relationship {
    /// The `Id` attribute, absent on malformed-but-tolerated entries.
    id: Option<String>,
    /// The `Type` URI; only its last path segment is significant.
    rel_type: String,
    /// The raw `Target` attribute, before any path resolution.
    target: String,
    /// `true` when `TargetMode="External"`.
    external: bool,
}

/// Everything the document part needs from `word/_rels/document.xml.rels`.
///
/// Built by a single pass over the part. Internal targets are already
/// resolved to package paths; the first internal styles and numbering
/// relationship wins, later hyperlinks and images with a repeated Id overwrite
/// earlier ones.
#[derive(Debug, Default)]
pub(crate) struct PartRels {
    /// Hyperlink Id → raw target.
    pub hyperlinks: HyperlinkMap,
    /// Image Id → resolved package path or external URL.
    pub images: ImageMap,
    /// Package path of `styles.xml`, when an internal styles relationship exists.
    pub styles_path: Option<String>,
    /// Package path of `numbering.xml`, when an internal numbering relationship exists.
    pub numbering_path: Option<String>,
}

impl PartRels {
    /// Parses a part relationships XML, resolving internal targets against `document_path`.
    ///
    /// Returns `Err` if the XML is malformed or an internal target escapes the package root.
    pub(crate) fn parse(rels_xml: &[u8], document_path: &str) -> docspec_core::Result<Self> {
        let mut rels = Self::default();
        for Relationship {
            id,
            rel_type,
            target,
            external,
        } in parse_relationships(rels_xml)?
        {
            let kind = rel_type.rsplit_once('/').map_or("", |(_, kind)| kind);
            match (kind, id) {
                ("hyperlink", Some(id)) => {
                    rels.hyperlinks.insert(id, target);
                }
                ("image", Some(id)) => {
                    let target = if external {
                        target
                    } else {
                        normalize_relative_target(document_path, &target)?
                    };
                    rels.images.insert(
                        id,
                        ImageRel {
                            target,
                            is_external: external,
                        },
                    );
                }
                ("styles", _) if rels.styles_path.is_none() && !external => {
                    rels.styles_path = Some(normalize_relative_target(document_path, &target)?);
                }
                ("numbering", _) if rels.numbering_path.is_none() && !external => {
                    rels.numbering_path = Some(normalize_relative_target(document_path, &target)?);
                }
                _ => {}
            }
        }
        Ok(rels)
    }
}

/// Finds the main document part from the package relationships (`_rels/.rels`).
///
/// Returns the first `/officeDocument` target, resolved against the package root.
pub(crate) fn find_document_target(rels_xml: &[u8]) -> docspec_core::Result<String> {
    parse_relationships(rels_xml)?
        .into_iter()
        .find(|rel| rel.rel_type.ends_with("/officeDocument"))
        .map_or_else(
            || {
                Err(Error::Parse {
                    message: "no officeDocument relationship".to_string(),
                    position: None,
                })
            },
            |rel| normalize_relative_target("", &rel.target),
        )
}

/// Every `<Relationship>` with a `Type` and a `Target`, in document order.
///
/// Any XML error, unbalanced element or malformed attribute fails with
/// [`RELS_ERROR_CONTEXT`], as every rels consumer has always reported it.
fn parse_relationships(rels_xml: &[u8]) -> docspec_core::Result<Vec<Relationship>> {
    let mut reader = quick_xml::Reader::from_reader(rels_xml);
    let mut buf = Vec::new();
    let mut element_depth: usize = 0;
    let mut relationships = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                element_depth = element_depth.saturating_add(1);
                if element.local_name().as_ref() == b"Relationship" {
                    relationships.extend(relationship(&reader, &element)?);
                }
            }
            Ok(Event::Empty(element)) if element.local_name().as_ref() == b"Relationship" => {
                relationships.extend(relationship(&reader, &element)?);
            }
            Ok(Event::End(_)) => {
                let Some(next_depth) = element_depth.checked_sub(1) else {
                    return Err(parse_error(RELS_ERROR_CONTEXT.to_string()));
                };
                element_depth = next_depth;
            }
            Ok(Event::Eof) => {
                if element_depth != 0 {
                    return Err(parse_error(RELS_ERROR_CONTEXT.to_string()));
                }
                return Ok(relationships);
            }
            Err(_err) => return Err(parse_error(RELS_ERROR_CONTEXT.to_string())),
            Ok(_) => {}
        }
        buf.clear();
    }
}

/// Reads one `<Relationship>` element; `None` when it lacks a `Type` or `Target`.
fn relationship<R>(
    reader: &quick_xml::Reader<R>,
    element: &BytesStart<'_>,
) -> docspec_core::Result<Option<Relationship>>
where
    R: Read,
{
    let id = attr_string(reader, element, b"Id", RELS_ERROR_CONTEXT)?;
    let rel_type = attr_string(reader, element, b"Type", RELS_ERROR_CONTEXT)?;
    let target = attr_string(reader, element, b"Target", RELS_ERROR_CONTEXT)?;
    let external = attr_string(reader, element, b"TargetMode", RELS_ERROR_CONTEXT)?.as_deref()
        == Some("External");
    Ok(match (rel_type, target) {
        (Some(rel_type), Some(target)) => Some(Relationship {
            id,
            rel_type,
            target,
            external,
        }),
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
fn resolve_relative_target(base_part: &str, target: &str) -> String {
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
/// or `Target="../customXml/item1.xml"` from `word/_rels/document.xml.rels`,
/// so `..` is collapsed rather than rejected. A `..` segment that pops past
/// the start of the resolved path escapes the package and returns
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

    const DOC: &str = "word/document.xml";

    fn rels_with(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{body}
</Relationships>"#
        )
    }

    fn office_document_rels(target: &str) -> String {
        rels_with(&format!(
            r#"  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{target}"/>"#
        ))
    }

    fn typed_rel(id: &str, kind: &str, target: &str, mode: Option<&str>) -> String {
        let mode_attr = mode.map_or(String::new(), |m| format!(r#" TargetMode="{m}""#));
        format!(
            r#"  <Relationship Id="{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{kind}" Target="{target}"{mode_attr}/>"#
        )
    }

    fn parse_error_message(result: docspec_core::Result<impl core::fmt::Debug>) -> String {
        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(position, None);
                message
            }
            other => panic!("expected Error::Parse, got {other:?}"),
        }
    }

    #[test]
    fn find_document_target_returns_first_office_document_relationship() {
        let rels_xml = rels_with(&format!(
            "{}\n{}\n{}",
            typed_rel("rId1", "metadata", "docProps/core.xml", None),
            typed_rel("rId2", "officeDocument", "word/document.xml", None),
            typed_rel("rId3", "officeDocument", "word/other.xml", None),
        ));
        assert_eq!(
            find_document_target(rels_xml.as_bytes()).expect("document path"),
            "word/document.xml"
        );
    }

    #[test]
    fn find_document_target_resolves_against_package_root() {
        for (target, expected) in [
            ("/word/document.xml", "word/document.xml"),
            ("./word/./document.xml", "word/document.xml"),
            ("word/sub/../document.xml", "word/document.xml"),
            ("word/doc&amp;ument.xml", "word/doc&ument.xml"),
        ] {
            let rels_xml = office_document_rels(target);
            assert_eq!(
                find_document_target(rels_xml.as_bytes()).expect("document path"),
                expected,
                "target {target}"
            );
        }
    }

    #[test]
    fn find_document_target_rejects_target_escaping_package_root() {
        let rels_xml = office_document_rels("../foo/document.xml");
        assert_eq!(
            parse_error_message(find_document_target(rels_xml.as_bytes())),
            "rels target escapes package root: ../foo/document.xml"
        );
    }

    #[test]
    fn find_document_target_errors_when_no_office_document_relationship() {
        for body in [
            "",
            r#"  <Relationship Id="rId1" Type="http://example.com/not-office" Target="word/document.xml"/>"#,
            r#"  <Group><Relationship Id="rId1" Type="http://example.com/not-office" Target="word/document.xml"></Relationship></Group>"#,
            // A Relationship without a Target is skipped.
            r#"  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"/>"#,
        ] {
            let rels_xml = rels_with(body);
            assert_eq!(
                parse_error_message(find_document_target(rels_xml.as_bytes())),
                "no officeDocument relationship",
                "body {body}"
            );
        }
    }

    #[test]
    fn find_document_target_accepts_non_empty_and_namespaced_elements() {
        let rels_xml = r#"<r:Relationships xmlns:r="http://schemas.openxmlformats.org/package/2006/relationships">
  <r:Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"></r:Relationship>
</r:Relationships>"#;
        assert_eq!(
            find_document_target(rels_xml.as_bytes()).expect("document path"),
            "word/document.xml"
        );
    }

    #[test]
    fn malformed_rels_xml_is_rejected() {
        for rels_xml in ["</Relationships>", "<Relationships><", "<broken>"] {
            assert_eq!(
                parse_error_message(find_document_target(rels_xml.as_bytes())),
                "malformed _rels/.rels",
                "input {rels_xml}"
            );
            assert_eq!(
                parse_error_message(PartRels::parse(rels_xml.as_bytes(), DOC)),
                "malformed _rels/.rels",
                "input {rels_xml}"
            );
        }
    }

    #[test]
    fn malformed_relationship_attribute_is_rejected_with_position() {
        let rels_xml = rels_with(
            r#"  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target=word/document.xml/>"#,
        );
        assert_eq!(
            parse_error_message(find_document_target(rels_xml.as_bytes())),
            "malformed _rels/.rels: position 120: attribute value must be enclosed in `\"` or `'`"
        );
    }

    #[test]
    fn bad_attribute_entity_is_rejected() {
        let rels_xml = office_document_rels("word/&bogus;.xml");
        assert_eq!(
            parse_error_message(find_document_target(rels_xml.as_bytes())),
            "malformed _rels/.rels: at 6..11: unrecognized entity `bogus`"
        );
    }

    #[test]
    fn part_rels_parse_of_empty_relationships_is_empty() {
        let rels_xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#;
        let rels = PartRels::parse(rels_xml.as_bytes(), DOC).expect("part rels");
        assert!(rels.hyperlinks.is_empty());
        assert!(rels.images.is_empty());
        assert_eq!(rels.styles_path, None);
        assert_eq!(rels.numbering_path, None);
    }

    #[test]
    fn part_rels_collects_hyperlinks_of_both_namespaces_and_any_target_mode() {
        let rels_xml = rels_with(&format!(
            "{}\n{}\n{}\n{}\n{}",
            typed_rel("rId1", "hyperlink", "https://example.com/one", None),
            r#"  <Relationship Id="rId2" Type="http://purl.oclc.org/ooxml/officeDocument/relationships/hyperlink" Target="https://example.com/two"/>"#,
            typed_rel(
                "rId3",
                "hyperlink",
                "https://example.com/three",
                Some("External")
            ),
            typed_rel("rId4", "hyperlink", "other.docx", Some("Internal")),
            typed_rel("rId5", "styles", "styles.xml", None),
        ));
        let rels = PartRels::parse(rels_xml.as_bytes(), DOC).expect("part rels");
        let expected: HyperlinkMap = [
            ("rId1", "https://example.com/one"),
            ("rId2", "https://example.com/two"),
            ("rId3", "https://example.com/three"),
            ("rId4", "other.docx"),
        ]
        .into_iter()
        .map(|(id, target)| (id.to_string(), target.to_string()))
        .collect();
        assert_eq!(rels.hyperlinks, expected);
    }

    #[test]
    fn part_rels_skips_hyperlinks_and_images_without_id() {
        let rels_xml = rels_with(
            r#"  <Relationship Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com"/>
  <Relationship Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>"#,
        );
        let rels = PartRels::parse(rels_xml.as_bytes(), DOC).expect("part rels");
        assert!(rels.hyperlinks.is_empty());
        assert!(rels.images.is_empty());
    }

    #[test]
    fn part_rels_resolves_internal_images_and_keeps_external_urls() {
        let rels_xml = rels_with(&format!(
            "{}\n{}\n{}\n{}",
            typed_rel("rId5", "image", "media/image1.png", None),
            typed_rel("rId6", "image", "../media/image2.png", None),
            typed_rel("rId7", "image", "./media/sub/../image3.png", None),
            typed_rel(
                "rId8",
                "image",
                "https://example.com/img.png",
                Some("External")
            ),
        ));
        let rels = PartRels::parse(rels_xml.as_bytes(), DOC).expect("part rels");
        let image = |id: &str| {
            let rel = rels.images.get(id).expect("image present");
            (rel.target.as_str(), rel.is_external)
        };
        assert_eq!(rels.images.len(), 4);
        assert_eq!(image("rId5"), ("word/media/image1.png", false));
        assert_eq!(image("rId6"), ("media/image2.png", false));
        assert_eq!(image("rId7"), ("word/media/image3.png", false));
        assert_eq!(image("rId8"), ("https://example.com/img.png", true));
    }

    #[test]
    fn part_rels_rejects_image_escaping_package_root() {
        let rels_xml = rels_with(&typed_rel("rId5", "image", "../../../etc/passwd", None));
        assert_eq!(
            parse_error_message(PartRels::parse(rels_xml.as_bytes(), DOC)),
            "rels target escapes package root: word/../../../etc/passwd"
        );
    }

    #[test]
    fn part_rels_resolves_first_internal_styles_and_numbering_targets() {
        let rels_xml = rels_with(&format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            typed_rel(
                "rId1",
                "styles",
                "https://example.com/styles.xml",
                Some("External")
            ),
            typed_rel("rId2", "styles", "styles.xml", None),
            typed_rel("rId3", "styles", "styles2.xml", None),
            typed_rel("rId4", "numbering", "../word/numbering.xml", None),
            typed_rel("rId5", "numbering", "numbering2.xml", None),
            typed_rel("rId6", "fontTable", "fontTable.xml", None),
        ));
        let rels = PartRels::parse(rels_xml.as_bytes(), DOC).expect("part rels");
        assert_eq!(rels.styles_path.as_deref(), Some("word/styles.xml"));
        assert_eq!(rels.numbering_path.as_deref(), Some("word/numbering.xml"));
    }

    #[test]
    fn part_rels_ignores_external_only_styles_and_numbering() {
        let rels_xml = rels_with(&format!(
            "{}\n{}",
            typed_rel("rId1", "styles", "styles.xml", Some("External")),
            typed_rel("rId2", "numbering", "numbering.xml", Some("External")),
        ));
        let rels = PartRels::parse(rels_xml.as_bytes(), DOC).expect("part rels");
        assert_eq!(rels.styles_path, None);
        assert_eq!(rels.numbering_path, None);
    }

    #[test]
    fn part_rels_rejects_styles_and_numbering_escaping_package_root() {
        for kind in ["styles", "numbering"] {
            let rels_xml = rels_with(&typed_rel("rId1", kind, "../../etc/passwd", None));
            assert_eq!(
                parse_error_message(PartRels::parse(rels_xml.as_bytes(), DOC)),
                "rels target escapes package root: word/../../etc/passwd",
                "kind {kind}"
            );
        }
    }

    #[test]
    fn derive_part_rels_path_with_and_without_directory() {
        assert_eq!(
            derive_part_rels_path("word/document.xml"),
            "word/_rels/document.xml.rels"
        );
        assert_eq!(derive_part_rels_path("foo"), "_rels/foo.rels");
    }

    #[test]
    fn resolve_relative_target_joins_strips_and_handles_root_parts() {
        assert_eq!(
            resolve_relative_target("word/document.xml", "styles.xml"),
            "word/styles.xml"
        );
        assert_eq!(
            resolve_relative_target("word/document.xml", "/word/styles.xml"),
            "word/styles.xml"
        );
        assert_eq!(
            resolve_relative_target("document.xml", "styles.xml"),
            "styles.xml"
        );
    }
}
