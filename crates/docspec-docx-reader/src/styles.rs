use std::io::Read;

use docspec_core::{Error, Result};
use quick_xml::events::Event;
use quick_xml::XmlVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StyleType {
    Paragraph,
    Character,
    Table,
    Numbering,
}

impl StyleType {
    fn from_attr(s: &str) -> Option<Self> {
        match s {
            "paragraph" => Some(Self::Paragraph),
            "character" => Some(Self::Character),
            "table" => Some(Self::Table),
            "numbering" => Some(Self::Numbering),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    id: String,
    kind: StyleType,
    name: Option<String>,
    based_on: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StyleList {
    styles: Vec<Style>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StyleClassification {
    Heading { level: u8 },
    BlockQuote,
    Code,
}

const HEADING_NAME_PREFIX: &str = "heading ";

fn classify_name(name: &str, kind: StyleType) -> Option<StyleClassification> {
    match name {
        "Title" => Some(StyleClassification::Heading { level: 1 }),
        "Quote" | "Block Text" | "Block Quote" | "Block Quotation" | "Intense Quote" => {
            Some(StyleClassification::BlockQuote)
        }
        "Source Code" | "SourceCode" | "Codeblock" if kind == StyleType::Paragraph => {
            Some(StyleClassification::Code)
        }
        "Verbatim Char" if kind == StyleType::Character => Some(StyleClassification::Code),
        n if n.starts_with(HEADING_NAME_PREFIX) => {
            let rest = n.strip_prefix(HEADING_NAME_PREFIX)?;
            let level = match rest.parse::<u8>() {
                Ok(level) => level,
                Err(err) if *err.kind() == core::num::IntErrorKind::PosOverflow => u8::MAX,
                Err(_) => return None,
            };
            (level >= 1).then_some(StyleClassification::Heading { level })
        }
        _ => None,
    }
}

impl StyleList {
    pub fn parse<R: Read>(reader: R) -> Result<Self> {
        let mut xml_reader = quick_xml::Reader::from_reader(std::io::BufReader::new(reader));
        let mut buf = Vec::new();
        let mut styles: Vec<Style> = Vec::new();

        loop {
            match xml_reader.read_event_into(&mut buf) {
                Ok(Event::Start(elem)) if elem.local_name().as_ref() == b"style" => {
                    let maybe_style_type = get_attr(&elem, b"type")?
                        .as_deref()
                        .and_then(StyleType::from_attr);
                    if let (Some(style_type), Some(id)) = (
                        maybe_style_type,
                        get_attr(&elem, b"styleId")?.filter(|s| !s.is_empty()),
                    ) {
                        let style = parse_style_body(&mut xml_reader, &mut buf, id, style_type)?;
                        styles.push(style);
                    }
                }

                Ok(Event::Eof) => break,

                Err(err) => {
                    return Err(parse_error(format!("malformed styles.xml: {err}")));
                }
                Ok(_) => {}
            }
            buf.clear();
        }

        Ok(Self { styles })
    }

    pub fn get_by_id(&self, id: &str) -> Option<&Style> {
        self.styles.iter().find(|style| style.id == id)
    }

    pub fn classify(&self, id: &str) -> Option<StyleClassification> {
        let mut visited: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut current = self.get_by_id(id)?;
        let kind = current.kind;
        loop {
            if !visited.insert(current.id.as_str()) {
                return None;
            }
            if let Some(name) = current.name.as_deref() {
                if let Some(classification) = classify_name(name, kind) {
                    return Some(classification);
                }
            }
            current = self.get_by_id(current.based_on.as_deref()?)?;
        }
    }
}

fn parse_style_body<R: std::io::BufRead>(
    xml_reader: &mut quick_xml::Reader<R>,
    buf: &mut Vec<u8>,
    id: String,
    kind: StyleType,
) -> Result<Style> {
    let mut name: Option<String> = None;
    let mut based_on: Option<String> = None;

    loop {
        match xml_reader.read_event_into(buf) {
            Ok(Event::Empty(elem)) if elem.local_name().as_ref() == b"name" => {
                if let Some(value) = get_attr(&elem, b"val")?.filter(|s| !s.is_empty()) {
                    name = Some(value);
                }
            }
            Ok(Event::Empty(elem)) if elem.local_name().as_ref() == b"basedOn" => {
                if let Some(value) = get_attr(&elem, b"val")?.filter(|s| !s.is_empty()) {
                    based_on = Some(value);
                }
            }
            Ok(Event::End(elem)) if elem.local_name().as_ref() == b"style" => break,
            Ok(Event::Eof) => {
                return Err(parse_error("unexpected EOF inside <w:style>".to_string()));
            }
            Err(err) => {
                return Err(parse_error(format!("malformed styles.xml: {err}")));
            }
            Ok(_) => {}
        }
        buf.clear();
    }
    Ok(Style {
        id,
        kind,
        name,
        based_on,
    })
}

fn get_attr(elem: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Result<Option<String>> {
    for attribute_result in elem.attributes() {
        let attribute =
            attribute_result.map_err(|err| parse_error(format!("malformed attribute: {err}")))?;
        if attribute.key.local_name().as_ref() == name {
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|err| parse_error(format!("malformed attribute value: {err}")))?
                .into_owned();
            return Ok(Some(value));
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
#[cfg(not(coverage))]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn styles_xml(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
{body}
</w:styles>"#
        )
    }

    #[test]
    fn parse_empty_styles_returns_empty_list() {
        let xml = styles_xml("");

        let result = StyleList::parse(Cursor::new(xml.into_bytes()));

        match result {
            Ok(list) => assert_eq!(list, StyleList::default()),
            Err(err) => assert_eq!(format!("{err:?}"), "expected empty StyleList"),
        }
    }

    #[test]
    fn parse_single_paragraph_style_with_name() {
        let xml = styles_xml(
            r#"<w:style w:type="paragraph" w:styleId="Normal">
                 <w:name w:val="Normal"/>
               </w:style>"#,
        );

        let result = StyleList::parse(Cursor::new(xml.into_bytes()));

        match result {
            Ok(list) => assert_eq!(
                list,
                StyleList {
                    styles: vec![Style {
                        id: "Normal".to_string(),
                        kind: StyleType::Paragraph,
                        name: Some("Normal".to_string()),
                        based_on: None,
                    }],
                }
            ),
            Err(err) => assert_eq!(format!("{err:?}"), "expected single Normal style"),
        }
    }

    #[test]
    fn parse_style_with_based_on() {
        let xml = styles_xml(
            r#"<w:style w:type="paragraph" w:styleId="Heading1">
                 <w:name w:val="heading 1"/>
                 <w:basedOn w:val="Normal"/>
               </w:style>"#,
        );

        let result = StyleList::parse(Cursor::new(xml.into_bytes()));

        match result {
            Ok(list) => assert_eq!(
                list,
                StyleList {
                    styles: vec![Style {
                        id: "Heading1".to_string(),
                        kind: StyleType::Paragraph,
                        name: Some("heading 1".to_string()),
                        based_on: Some("Normal".to_string()),
                    }],
                }
            ),
            Err(err) => assert_eq!(format!("{err:?}"), "expected Heading1 with basedOn"),
        }
    }

    #[test]
    fn parse_stores_character_and_paragraph_styles_with_their_types() {
        let xml = styles_xml(
            r#"<w:style w:type="character" w:styleId="DefaultParagraphFont">
                 <w:name w:val="Default Paragraph Font"/>
               </w:style>
               <w:style w:type="paragraph" w:styleId="Normal">
                 <w:name w:val="Normal"/>
               </w:style>"#,
        );

        let result = StyleList::parse(Cursor::new(xml.into_bytes()));

        match result {
            Ok(list) => assert_eq!(
                list,
                StyleList {
                    styles: vec![
                        Style {
                            id: "DefaultParagraphFont".to_string(),
                            kind: StyleType::Character,
                            name: Some("Default Paragraph Font".to_string()),
                            based_on: None,
                        },
                        Style {
                            id: "Normal".to_string(),
                            kind: StyleType::Paragraph,
                            name: Some("Normal".to_string()),
                            based_on: None,
                        },
                    ],
                }
            ),
            Err(err) => assert_eq!(format!("{err:?}"), "expected character + paragraph styles"),
        }
    }

    fn paragraph(id: &str, name: Option<&str>, based_on: Option<&str>) -> Style {
        Style {
            id: id.to_string(),
            kind: StyleType::Paragraph,
            name: name.map(str::to_string),
            based_on: based_on.map(str::to_string),
        }
    }

    fn character(id: &str, name: Option<&str>, based_on: Option<&str>) -> Style {
        Style {
            id: id.to_string(),
            kind: StyleType::Character,
            name: name.map(str::to_string),
            based_on: based_on.map(str::to_string),
        }
    }

    #[test]
    fn classify_name_recognizes_title() {
        assert_eq!(
            classify_name("Title", StyleType::Paragraph),
            Some(StyleClassification::Heading { level: 1 })
        );
    }

    #[test]
    fn classify_name_recognizes_block_quote_aliases() {
        assert_eq!(
            classify_name("Block Quote", StyleType::Paragraph),
            Some(StyleClassification::BlockQuote)
        );
        assert_eq!(
            classify_name("Intense Quote", StyleType::Paragraph),
            Some(StyleClassification::BlockQuote)
        );
    }

    #[test]
    fn classify_name_recognizes_source_code_paragraph_aliases() {
        assert_eq!(
            classify_name("Source Code", StyleType::Paragraph),
            Some(StyleClassification::Code)
        );
        assert_eq!(
            classify_name("SourceCode", StyleType::Paragraph),
            Some(StyleClassification::Code)
        );
        assert_eq!(
            classify_name("Codeblock", StyleType::Paragraph),
            Some(StyleClassification::Code)
        );
    }

    #[test]
    fn classify_name_rejects_html_family_and_plain_text_for_paragraph() {
        assert_eq!(
            classify_name("HTML Preformatted", StyleType::Paragraph),
            None
        );
        assert_eq!(classify_name("HTML Code", StyleType::Paragraph), None);
        assert_eq!(classify_name("HTML Sample", StyleType::Paragraph), None);
        assert_eq!(classify_name("Plain Text", StyleType::Paragraph), None);
    }

    #[test]
    fn classify_name_recognizes_verbatim_char_only_for_character_style() {
        assert_eq!(
            classify_name("Verbatim Char", StyleType::Character),
            Some(StyleClassification::Code)
        );
        assert_eq!(classify_name("Verbatim Char", StyleType::Paragraph), None);
    }

    #[test]
    fn classify_name_rejects_source_code_for_character_style() {
        assert_eq!(classify_name("Source Code", StyleType::Character), None);
        assert_eq!(classify_name("SourceCode", StyleType::Character), None);
        assert_eq!(classify_name("Codeblock", StyleType::Character), None);
    }

    #[test]
    fn classify_name_parses_heading_level() {
        assert_eq!(
            classify_name("heading 3", StyleType::Paragraph),
            Some(StyleClassification::Heading { level: 3 })
        );
    }

    #[test]
    fn classify_name_saturates_heading_level_above_u8_max() {
        assert_eq!(
            classify_name("heading 999", StyleType::Paragraph),
            Some(StyleClassification::Heading { level: 255 })
        );
    }

    #[test]
    fn classify_name_rejects_zero_heading_level() {
        assert_eq!(classify_name("heading 0", StyleType::Paragraph), None);
    }

    #[test]
    fn classify_name_rejects_non_numeric_heading() {
        assert_eq!(classify_name("heading abc", StyleType::Paragraph), None);
    }

    #[test]
    fn classify_name_returns_none_for_unknown_name() {
        assert_eq!(
            classify_name("Some Random Style", StyleType::Paragraph),
            None
        );
    }

    #[test]
    fn get_by_id_returns_matching_style() {
        let list = StyleList {
            styles: vec![paragraph("Normal", Some("Normal"), None)],
        };

        assert_eq!(
            list.get_by_id("Normal"),
            Some(&paragraph("Normal", Some("Normal"), None))
        );
    }

    #[test]
    fn get_by_id_returns_none_for_missing_style() {
        let list = StyleList::default();

        assert_eq!(list.get_by_id("Missing"), None);
    }

    #[test]
    fn classify_resolves_name_on_the_style_itself() {
        let list = StyleList {
            styles: vec![paragraph("Title", Some("Title"), None)],
        };

        assert_eq!(
            list.classify("Title"),
            Some(StyleClassification::Heading { level: 1 })
        );
    }

    #[test]
    fn classify_walks_based_on_chain_until_match() {
        let list = StyleList {
            styles: vec![
                paragraph("Normal", Some("Normal"), None),
                paragraph("Heading1", Some("heading 1"), Some("Normal")),
                paragraph("CustomHeading", Some("My Heading"), Some("Heading1")),
            ],
        };

        assert_eq!(
            list.classify("CustomHeading"),
            Some(StyleClassification::Heading { level: 1 })
        );
    }

    #[test]
    fn classify_returns_none_when_chain_has_no_known_name() {
        let list = StyleList {
            styles: vec![
                paragraph("A", Some("Unknown A"), Some("B")),
                paragraph("B", Some("Unknown B"), None),
            ],
        };

        assert_eq!(list.classify("A"), None);
    }

    #[test]
    fn classify_detects_cycle_and_returns_none() {
        let list = StyleList {
            styles: vec![
                paragraph("A", Some("Unknown A"), Some("B")),
                paragraph("B", Some("Unknown B"), Some("A")),
            ],
        };

        assert_eq!(list.classify("A"), None);
    }

    #[test]
    fn classify_skips_styles_without_names() {
        let list = StyleList {
            styles: vec![
                paragraph("Anon", None, Some("Normal")),
                paragraph("Normal", Some("Title"), None),
            ],
        };

        assert_eq!(
            list.classify("Anon"),
            Some(StyleClassification::Heading { level: 1 })
        );
    }

    #[test]
    fn classify_returns_none_for_missing_id() {
        assert_eq!(StyleList::default().classify("Missing"), None);
    }

    #[test]
    fn classify_returns_code_for_character_style_named_verbatim_char() {
        let list = StyleList {
            styles: vec![character("VerbatimChar", Some("Verbatim Char"), None)],
        };

        assert_eq!(
            list.classify("VerbatimChar"),
            Some(StyleClassification::Code)
        );
    }

    #[test]
    fn classify_returns_none_for_paragraph_style_named_verbatim_char() {
        let list = StyleList {
            styles: vec![paragraph("VerbatimPara", Some("Verbatim Char"), None)],
        };

        assert_eq!(list.classify("VerbatimPara"), None);
    }

    #[test]
    fn classify_returns_none_for_paragraph_style_named_html_preformatted() {
        let list = StyleList {
            styles: vec![paragraph(
                "HTMLSudahDiformat",
                Some("HTML Preformatted"),
                Some("Normal"),
            )],
        };

        assert_eq!(list.classify("HTMLSudahDiformat"), None);
    }

    #[test]
    fn classify_returns_code_for_paragraph_style_named_sourcecode() {
        let list = StyleList {
            styles: vec![paragraph("SourceCode", Some("SourceCode"), None)],
        };

        assert_eq!(list.classify("SourceCode"), Some(StyleClassification::Code));
    }

    #[test]
    fn parse_preserves_order_across_multiple_paragraph_styles() {
        let xml = styles_xml(
            r#"<w:style w:type="paragraph" w:styleId="Normal">
                 <w:name w:val="Normal"/>
               </w:style>
               <w:style w:type="paragraph" w:styleId="Heading1">
                 <w:name w:val="heading 1"/>
                 <w:basedOn w:val="Normal"/>
               </w:style>"#,
        );

        let result = StyleList::parse(Cursor::new(xml.into_bytes()));

        match result {
            Ok(list) => assert_eq!(
                list,
                StyleList {
                    styles: vec![
                        Style {
                            id: "Normal".to_string(),
                            kind: StyleType::Paragraph,
                            name: Some("Normal".to_string()),
                            based_on: None,
                        },
                        Style {
                            id: "Heading1".to_string(),
                            kind: StyleType::Paragraph,
                            name: Some("heading 1".to_string()),
                            based_on: Some("Normal".to_string()),
                        },
                    ],
                }
            ),
            Err(err) => assert_eq!(format!("{err:?}"), "expected two styles in order"),
        }
    }
}
