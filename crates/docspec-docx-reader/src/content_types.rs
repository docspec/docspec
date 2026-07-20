//! Content type mapping from `[Content_Types].xml`.

use std::collections::HashMap;
use std::io::Read;

use docspec_core::Error;
use quick_xml::events::Event;
use quick_xml::XmlVersion;

/// Content type lookup for DOCX package parts.
///
/// Parsed from `[Content_Types].xml` via [`parse`]. [`lookup`][ContentTypes::lookup]
/// resolves a ZIP entry path to its MIME type following ECMA-376 §10.1.2:
/// override entries (matched by exact part path) take precedence over
/// default entries (matched by file extension).
pub(crate) struct ContentTypes {
    /// Maps lowercase file extension → MIME type (from `<Default>` elements).
    defaults: HashMap<String, String>,
    /// Maps stripped part path (no leading `/`) → MIME type (from `<Override>` elements).
    overrides: HashMap<String, String>,
}

impl Default for ContentTypes {
    #[inline]
    fn default() -> Self {
        Self {
            defaults: HashMap::new(),
            overrides: HashMap::new(),
        }
    }
}

/// Parses `[Content_Types].xml` bytes into a [`ContentTypes`] lookup table.
///
/// Empty input (`xml = &[]`) represents a missing `[Content_Types].xml` and returns
/// `Ok(ContentTypes::default())` rather than an error.
///
/// # Errors
///
/// Returns [`docspec_core::Error::Parse`] for malformed XML.
pub(crate) fn parse(xml: &[u8]) -> docspec_core::Result<ContentTypes> {
    if xml.is_empty() {
        return Ok(ContentTypes::default());
    }

    let mut xml_reader = quick_xml::Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut element_depth: usize = 0;
    let mut ct = ContentTypes::default();

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                element_depth = element_depth.saturating_add(1);
                process_element(&xml_reader, &element, &mut ct)?;
            }
            Ok(Event::Empty(element)) => {
                process_element(&xml_reader, &element, &mut ct)?;
            }
            Ok(Event::End(_)) => {
                let Some(next_depth) = element_depth.checked_sub(1) else {
                    return Err(parse_error("malformed [Content_Types].xml".to_string()));
                };
                element_depth = next_depth;
            }
            Ok(Event::Eof) => {
                if element_depth != 0 {
                    return Err(parse_error("malformed [Content_Types].xml".to_string()));
                }
                return Ok(ct);
            }
            Err(err) => {
                return Err(parse_error(format!("malformed [Content_Types].xml: {err}")));
            }
            Ok(_) => {}
        }
        buf.clear();
    }
}

impl ContentTypes {
    /// Resolves a ZIP entry path to its MIME content type.
    ///
    /// Checks `Override` entries (exact part path match, leading `/` already stripped
    /// at parse time) first, then falls back to `Default` entries matched by the
    /// lowercase file extension of `part_path`.
    ///
    /// The extension is the substring after the last `.` in the final path segment
    /// (i.e. after the last `/`). Part paths without a `.` in the final segment have
    /// no extension and never match a `Default` entry, even when a `Default` exists
    /// whose `Extension` happens to equal the whole part path.
    ///
    /// Returns `None` if neither an override nor a default entry matches.
    #[inline]
    #[must_use]
    pub(crate) fn lookup<'a>(&'a self, part_path: &str) -> Option<&'a str> {
        if let Some(ct) = self.overrides.get(part_path) {
            return Some(ct.as_str());
        }
        let file_name = part_path.rsplit('/').next().unwrap_or(part_path);
        let (_, ext) = file_name.rsplit_once('.')?;
        if ext.is_empty() {
            return None;
        }
        self.defaults
            .get(&ext.to_ascii_lowercase())
            .map(String::as_str)
    }
}

fn process_element<R>(
    reader: &quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
    ct: &mut ContentTypes,
) -> docspec_core::Result<()>
where
    R: Read,
{
    match element.local_name().as_ref() {
        b"Default" => {
            let ext = attr_string(reader, element, b"Extension")?;
            let content_type = attr_string(reader, element, b"ContentType")?;
            if let (Some(ext_val), Some(ct_val)) = (ext, content_type) {
                ct.defaults.insert(ext_val.to_ascii_lowercase(), ct_val);
            }
        }
        b"Override" => {
            let part_name = attr_string(reader, element, b"PartName")?;
            let content_type = attr_string(reader, element, b"ContentType")?;
            if let (Some(part), Some(ct_val)) = (part_name, content_type) {
                let key = part.strip_prefix('/').unwrap_or(&part).to_string();
                ct.overrides.insert(key, ct_val);
            }
        }
        _ => {}
    }
    Ok(())
}

fn attr_string<R>(
    reader: &quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
    name: &[u8],
) -> docspec_core::Result<Option<String>>
where
    R: Read,
{
    for attribute_result in element.attributes() {
        let attribute = attribute_result
            .map_err(|err| parse_error(format!("malformed [Content_Types].xml: {err}")))?;
        if attribute.key.local_name().as_ref() == name {
            return attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                .map(|value| Some(value.into_owned()))
                .map_err(|err| parse_error(format!("malformed [Content_Types].xml: {err}")));
        }
    }
    Ok(None)
}

fn parse_error(message: String) -> Error {
    Error::Parse {
        message,
        position: None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn default_extension_lookup() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="png" ContentType="image/png"/>
</Types>"#;
        let ct = parse(xml).expect("parse should succeed");
        assert_eq!(ct.lookup("word/media/image1.png"), Some("image/png"));
    }

    #[test]
    fn override_beats_default() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
        let ct = parse(xml).expect("parse should succeed");
        assert_eq!(
            ct.lookup("word/document.xml"),
            Some(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
            )
        );
    }

    #[test]
    fn default_when_empty() {
        let ct = parse(&[]).expect("parse of empty input should succeed");
        assert_eq!(ct.lookup("word/media/image1.png"), None);
        assert_eq!(ct.lookup("word/document.xml"), None);
    }

    #[test]
    fn extension_lookup_is_case_insensitive() {
        let xml = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="PNG" ContentType="image/png"/>
</Types>"#;
        let ct = parse(xml).expect("parse should succeed");
        assert_eq!(ct.lookup("word/media/photo.png"), Some("image/png"));
        assert_eq!(ct.lookup("word/media/photo.PNG"), Some("image/png"));
    }

    #[test]
    fn lookup_returns_none_for_unknown_extension() {
        let xml = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="png" ContentType="image/png"/>
</Types>"#;
        let ct = parse(xml).expect("parse should succeed");
        assert_eq!(ct.lookup("word/media/file.docx"), None);
    }

    #[test]
    fn override_strips_leading_slash_from_part_name() {
        let xml = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Override PartName="/word/document.xml" ContentType="application/vnd.docx"/>
</Types>"#;
        let ct = parse(xml).expect("parse should succeed");
        assert_eq!(ct.lookup("word/document.xml"), Some("application/vnd.docx"));
        assert_eq!(ct.lookup("/word/document.xml"), None);
    }

    #[test]
    fn non_self_closing_elements_are_parsed() {
        let xml = br#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="png" ContentType="image/png"></Default>
  <Override PartName="/word/document.xml" ContentType="application/vnd.docx"></Override>
</Types>"#;
        let ct = parse(xml).expect("parse should succeed");
        assert_eq!(ct.lookup("word/media/image1.png"), Some("image/png"));
        assert_eq!(ct.lookup("word/document.xml"), Some("application/vnd.docx"));
    }

    #[test]
    fn malformed_xml_returns_error() {
        let result = parse(b"<Types><broken>");
        assert!(matches!(result, Err(docspec_core::Error::Parse { .. })));
    }

    #[test]
    fn multiple_defaults_and_overrides() {
        let xml = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.rels"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.docx"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.styles"/>
</Types>"#;
        let ct = parse(xml).expect("parse should succeed");
        assert_eq!(ct.lookup("word/media/image1.png"), Some("image/png"));
        assert_eq!(ct.lookup("word/media/photo.jpeg"), Some("image/jpeg"));
        assert_eq!(ct.lookup("word/document.xml"), Some("application/vnd.docx"));
        assert_eq!(ct.lookup("word/styles.xml"), Some("application/vnd.styles"));
        assert_eq!(ct.lookup("word/theme/theme1.xml"), Some("application/xml"));
    }

    #[test]
    fn lookup_no_extension_returns_none() {
        let xml = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#;
        let ct = parse(xml).expect("parse should succeed");
        assert_eq!(ct.lookup("noextension"), None);
    }

    #[test]
    fn lookup_extensionless_part_does_not_match_default_with_same_name() {
        let xml = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="png" ContentType="image/png"/>
</Types>"#;
        let ct = parse(xml).expect("parse should succeed");
        assert_eq!(ct.lookup("png"), None);
        assert_eq!(ct.lookup("media/png"), None);
        assert_eq!(ct.lookup("dotted.dir/noext"), None);
        assert_eq!(ct.lookup("trailing."), None);
    }
}
