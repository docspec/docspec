//! DOCX numbering definitions (`numbering.xml`) lookup structures.

use core::fmt;
use std::{collections::HashMap, io::Read};

use crate::properties;
use docspec_core::{Error, ListStyleType};
use quick_xml::{events::Event, XmlVersion};

/// Outcome of a list level definition: either a list with a style, or not a list.
#[derive(Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum LevelOutcome {
    /// Level is not a list (numFmt="none").
    NotAList,
    /// Level is a list with the given style.
    List(ListStyleType),
}

/// Result of resolving a list item's numId and ilvl to a list style.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ListLookupResult {
    /// Whether this is a list item (true) or a regular paragraph (false).
    pub is_list: bool,
    /// Whether the list is ordered (true) or unordered (false).
    pub is_ordered: bool,
    /// Visual style of the list marker.
    pub style_type: ListStyleType,
}

/// Minimal numbering lookup structure mapping numId → abstractNumId → levels.
pub struct MinimalNumbering {
    /// Maps abstractNumId to array of level outcomes.
    abstract_levels: HashMap<u32, [Option<LevelOutcome>; 9]>,
    /// Maps numId to abstractNumId.
    num_to_abstract: HashMap<u32, u32>,
}

impl MinimalNumbering {
    /// Creates an empty `MinimalNumbering` with no entries.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            abstract_levels: HashMap::new(),
            num_to_abstract: HashMap::new(),
        }
    }

    /// Resolves a numId and ilvl to a list lookup result.
    ///
    /// Returns a `ListLookupResult` indicating whether the item is a list,
    /// whether it is ordered, and its style type. Gracefully degrades on
    /// missing entries by returning sensible defaults.
    #[inline]
    #[must_use]
    pub fn resolve(&self, num_id: u32, ilvl: u32) -> ListLookupResult {
        // numId == 0 is a sentinel meaning "paragraph escapes list membership" (§17.9.18)
        if num_id == 0 {
            return ListLookupResult {
                is_list: false,
                is_ordered: true,
                style_type: ListStyleType::Decimal,
            };
        }

        // Lookup numId → abstractNumId
        let Some(&abstract_num_id) = self.num_to_abstract.get(&num_id) else {
            // Unknown numId: graceful degradation
            return ListLookupResult {
                is_list: false,
                is_ordered: true,
                style_type: ListStyleType::Decimal,
            };
        };

        // Lookup abstractNumId → levels array
        let Some(levels) = self.abstract_levels.get(&abstract_num_id) else {
            // Unknown abstractNumId: default to ordered decimal list
            return ListLookupResult {
                is_list: true,
                is_ordered: true,
                style_type: ListStyleType::Decimal,
            };
        };

        // Clamp ilvl to 0..=8 (spec max is 8)
        let clamped_ilvl = usize::try_from(core::cmp::min(ilvl, 8)).unwrap_or(8);

        // Read the level outcome
        match levels.get(clamped_ilvl).and_then(|opt| opt.as_ref()) {
            None => {
                // Level not defined: default to ordered decimal list
                ListLookupResult {
                    is_list: true,
                    is_ordered: true,
                    style_type: ListStyleType::Decimal,
                }
            }
            Some(LevelOutcome::NotAList) => {
                // Explicit numFmt="none"
                ListLookupResult {
                    is_list: false,
                    is_ordered: true,
                    style_type: ListStyleType::Decimal,
                }
            }
            Some(LevelOutcome::List(style)) => {
                // Determine if ordered based on style
                let is_ordered = !matches!(
                    style,
                    ListStyleType::Disc | ListStyleType::Circle | ListStyleType::Square
                );
                ListLookupResult {
                    is_list: true,
                    is_ordered,
                    style_type: *style,
                }
            }
        }
    }
}

impl Default for MinimalNumbering {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for MinimalNumbering {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MinimalNumbering")
            .field("abstract_levels", &self.abstract_levels)
            .field("num_to_abstract", &self.num_to_abstract)
            .finish()
    }
}

/// Parses DOCX `numbering.xml` into the minimal list lookup structure.
///
/// # Errors
///
/// Returns an error for I/O failures and malformed XML structure.
#[inline]
pub fn parse_numbering<R: Read>(reader: R) -> docspec_core::Result<MinimalNumbering> {
    let mut xml_reader = quick_xml::Reader::from_reader(std::io::BufReader::new(reader));
    let mut buf = Vec::new();
    let mut element_depth: usize = 0;
    let mut numbering = MinimalNumbering::new();

    let mut current_abstract = None;
    let mut current_ilvl = None;
    let mut current_level_outcome = None;
    let mut current_num = None;

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(element)) => {
                element_depth = element_depth.saturating_add(1);
                match element.local_name().as_ref() {
                    b"abstractNum" => {
                        current_abstract = attr_u32(&xml_reader, &element, b"abstractNumId")?;
                    }
                    b"lvl" if current_abstract.is_some() => {
                        current_ilvl = attr_u32(&xml_reader, &element, b"ilvl")?;
                        current_level_outcome = None;
                    }
                    b"numFmt" if current_abstract.is_some() && current_ilvl.is_some() => {
                        if let Some(value) = attr_string(&xml_reader, &element, b"val")? {
                            current_level_outcome = Some(properties::parse_num_fmt(&value));
                        }
                    }
                    b"num" => {
                        current_num = attr_u32(&xml_reader, &element, b"numId")?;
                    }
                    b"abstractNumId" => {
                        if let (Some(num_id), Some(abstract_num_id)) =
                            (current_num, attr_u32(&xml_reader, &element, b"val")?)
                        {
                            numbering.num_to_abstract.insert(num_id, abstract_num_id);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(element)) => match element.local_name().as_ref() {
                b"numFmt" if current_abstract.is_some() && current_ilvl.is_some() => {
                    if let Some(value) = attr_string(&xml_reader, &element, b"val")? {
                        current_level_outcome = Some(properties::parse_num_fmt(&value));
                    }
                }
                b"abstractNumId" => {
                    if let (Some(num_id), Some(abstract_num_id)) =
                        (current_num, attr_u32(&xml_reader, &element, b"val")?)
                    {
                        numbering.num_to_abstract.insert(num_id, abstract_num_id);
                    }
                }
                _ => {}
            },
            Ok(Event::End(element)) => {
                match element.local_name().as_ref() {
                    b"lvl" => finish_level(
                        &mut numbering,
                        current_abstract,
                        &mut current_ilvl,
                        &mut current_level_outcome,
                    ),
                    b"abstractNum" => current_abstract = None,
                    b"num" => current_num = None,
                    _ => {}
                }

                let Some(next_depth) = element_depth.checked_sub(1) else {
                    return Err(parse_error("malformed numbering.xml".to_string()));
                };
                element_depth = next_depth;
            }
            Ok(Event::Eof) => {
                if element_depth != 0 {
                    return Err(parse_error("malformed numbering.xml".to_string()));
                }
                return Ok(numbering);
            }
            Err(err) => {
                return Err(parse_error(format!("malformed numbering.xml: {err}")));
            }
            Ok(_) => {}
        }
        buf.clear();
    }
}

fn finish_level(
    numbering: &mut MinimalNumbering,
    current_abstract: Option<u32>,
    current_ilvl: &mut Option<u32>,
    current_level_outcome: &mut Option<LevelOutcome>,
) {
    if let (Some(abstract_num_id), Some(ilvl)) = (current_abstract, *current_ilvl) {
        let levels = numbering
            .abstract_levels
            .entry(abstract_num_id)
            .or_insert_with(|| core::array::from_fn(|_| None));
        let clamped_ilvl = usize::try_from(core::cmp::min(ilvl, 8)).unwrap_or(8);
        if let Some(level) = levels.get_mut(clamped_ilvl) {
            *level = Some(
                current_level_outcome
                    .take()
                    .unwrap_or(LevelOutcome::List(ListStyleType::Decimal)),
            );
        }
    }

    *current_ilvl = None;
    *current_level_outcome = None;
}

fn attr_u32<R: Read>(
    reader: &quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
    name: &[u8],
) -> docspec_core::Result<Option<u32>> {
    let Some(value) = attr_string(reader, element, name)? else {
        return Ok(None);
    };

    Ok(value.parse::<u32>().ok())
}

fn attr_string<R: Read>(
    reader: &quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
    name: &[u8],
) -> docspec_core::Result<Option<String>> {
    for attribute_result in element.attributes() {
        let attribute = attribute_result
            .map_err(|err| parse_error(format!("malformed numbering.xml attribute: {err}")))?;
        if attribute.key.local_name().as_ref() == name {
            return attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                .map(|value| Some(value.into_owned()))
                .map_err(|err| parse_error(format!("malformed numbering.xml attribute: {err}")));
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
    use super::*;

    macro_rules! parse_ok {
        ($xml:expr) => {{
            let result = parse_numbering($xml);
            assert_eq!(result.is_ok(), true);
            match result {
                Ok(numbering) => numbering,
                Err(_) => return,
            }
        }};
    }

    #[test]
    fn resolve_returns_not_list_for_num_id_zero() {
        let numbering = MinimalNumbering::new();

        let result = numbering.resolve(0, 0);

        assert!(!result.is_list);
        assert!(result.is_ordered);
        assert_eq!(result.style_type, ListStyleType::Decimal);
    }

    #[test]
    fn resolve_returns_not_list_for_unknown_num_id() {
        let numbering = MinimalNumbering::new();

        let result = numbering.resolve(999, 0);

        assert!(!result.is_list);
        assert!(result.is_ordered);
        assert_eq!(result.style_type, ListStyleType::Decimal);
    }

    #[test]
    fn resolve_returns_decimal_for_unknown_abstract_num_id() {
        let mut numbering = MinimalNumbering::new();
        numbering.num_to_abstract.insert(1, 999);

        let result = numbering.resolve(1, 0);

        assert!(result.is_list);
        assert!(result.is_ordered);
        assert_eq!(result.style_type, ListStyleType::Decimal);
    }

    #[test]
    fn resolve_returns_decimal_for_missing_ilvl_slot() {
        let mut numbering = MinimalNumbering::new();
        numbering.num_to_abstract.insert(1, 1);
        numbering
            .abstract_levels
            .insert(1, core::array::from_fn(|_| None));

        let result = numbering.resolve(1, 0);

        assert!(result.is_list);
        assert!(result.is_ordered);
        assert_eq!(result.style_type, ListStyleType::Decimal);
    }

    #[test]
    fn resolve_returns_not_list_for_not_a_list_slot() {
        let mut numbering = MinimalNumbering::new();
        numbering.num_to_abstract.insert(1, 1);
        let mut levels = core::array::from_fn(|_| None);
        levels[0] = Some(LevelOutcome::NotAList);
        numbering.abstract_levels.insert(1, levels);

        let result = numbering.resolve(1, 0);

        assert!(!result.is_list);
        assert!(result.is_ordered);
        assert_eq!(result.style_type, ListStyleType::Decimal);
    }

    #[test]
    fn resolve_returns_lower_alpha_for_level_with_lower_alpha() {
        let mut numbering = MinimalNumbering::new();
        numbering.num_to_abstract.insert(1, 1);
        let mut levels = core::array::from_fn(|_| None);
        levels[0] = Some(LevelOutcome::List(ListStyleType::LowerAlpha));
        numbering.abstract_levels.insert(1, levels);

        let result = numbering.resolve(1, 0);

        assert!(result.is_list);
        assert!(result.is_ordered);
        assert_eq!(result.style_type, ListStyleType::LowerAlpha);
    }

    #[test]
    fn resolve_returns_correct_style_for_each_variant() {
        let test_cases = vec![
            (ListStyleType::Decimal, true),
            (ListStyleType::LowerAlpha, true),
            (ListStyleType::UpperAlpha, true),
            (ListStyleType::LowerRoman, true),
            (ListStyleType::UpperRoman, true),
            (ListStyleType::Disc, false),
            (ListStyleType::Circle, false),
            (ListStyleType::Square, false),
        ];

        for (style, expected_ordered) in test_cases {
            let mut numbering = MinimalNumbering::new();
            numbering.num_to_abstract.insert(1, 1);
            let mut levels = core::array::from_fn(|_| None);
            levels[0] = Some(LevelOutcome::List(style));
            numbering.abstract_levels.insert(1, levels);

            let result = numbering.resolve(1, 0);

            assert!(result.is_list, "Failed for style: {style:?}");
            assert_eq!(
                result.is_ordered, expected_ordered,
                "Failed for style: {style:?}"
            );
            assert_eq!(result.style_type, style, "Failed for style: {style:?}");
        }
    }

    #[test]
    fn resolve_clamps_ilvl_above_eight() {
        let mut numbering = MinimalNumbering::new();
        numbering.num_to_abstract.insert(1, 1);
        let mut levels = core::array::from_fn(|_| None);
        levels[8] = Some(LevelOutcome::List(ListStyleType::UpperRoman));
        numbering.abstract_levels.insert(1, levels);

        let result = numbering.resolve(1, 15);

        assert!(result.is_list);
        assert!(result.is_ordered);
        assert_eq!(result.style_type, ListStyleType::UpperRoman);
    }

    #[test]
    fn parse_numbering_returns_empty_for_empty_xml() {
        let xml = br#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#;

        let numbering = parse_ok!(&xml[..]);

        assert_eq!(numbering.abstract_levels.len(), 0);
        assert_eq!(numbering.num_to_abstract.len(), 0);
    }

    #[test]
    fn parse_numbering_loads_single_bullet_definition() {
        let xml = br#"
            <w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:abstractNum w:abstractNumId="7">
                    <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>
                </w:abstractNum>
                <w:num w:numId="42"><w:abstractNumId w:val="7"/></w:num>
            </w:numbering>
        "#;

        let numbering = parse_ok!(&xml[..]);
        let result = numbering.resolve(42, 0);

        assert_eq!(
            result,
            ListLookupResult {
                is_list: true,
                is_ordered: false,
                style_type: ListStyleType::Disc,
            }
        );
    }

    #[test]
    fn parse_numbering_handles_mixed_format_levels() {
        let xml = br#"
            <w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:abstractNum w:abstractNumId="3">
                    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
                    <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl>
                    <w:lvl w:ilvl="2"><w:numFmt w:val="lowerLetter"/></w:lvl>
                </w:abstractNum>
                <w:num w:numId="9"><w:abstractNumId w:val="3"/></w:num>
            </w:numbering>
        "#;

        let numbering = parse_ok!(&xml[..]);

        assert_eq!(
            numbering.resolve(9, 0),
            ListLookupResult {
                is_list: true,
                is_ordered: true,
                style_type: ListStyleType::Decimal,
            }
        );
        assert_eq!(
            numbering.resolve(9, 1),
            ListLookupResult {
                is_list: true,
                is_ordered: false,
                style_type: ListStyleType::Disc,
            }
        );
        assert_eq!(
            numbering.resolve(9, 2),
            ListLookupResult {
                is_list: true,
                is_ordered: true,
                style_type: ListStyleType::LowerAlpha,
            }
        );
    }

    #[test]
    fn parse_numbering_skips_malformed_elements() {
        let xml = br#"
            <w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:abstractNum>
                    <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>
                </w:abstractNum>
                <w:abstractNum w:abstractNumId="2">
                    <w:lvl w:ilvl="not-a-number"><w:numFmt w:val="bullet"/></w:lvl>
                </w:abstractNum>
                <w:num w:numId="5"></w:num>
            </w:numbering>
        "#;

        let numbering = parse_ok!(&xml[..]);

        assert_eq!(numbering.abstract_levels.len(), 0);
        assert_eq!(numbering.num_to_abstract.len(), 0);
        assert_eq!(
            numbering.resolve(5, 0),
            ListLookupResult {
                is_list: false,
                is_ordered: true,
                style_type: ListStyleType::Decimal,
            }
        );
    }

    #[test]
    fn parse_numbering_returns_err_on_truncated_xml() {
        let xml = br#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="1"><w:lvl"#;

        let result = parse_numbering(&xml[..]);

        assert!(matches!(result, Err(Error::Parse { .. })));
    }

    #[test]
    fn parse_numbering_defaults_missing_numfmt_to_decimal() {
        let xml = br#"
            <w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:abstractNum w:abstractNumId="1">
                    <w:lvl w:ilvl="0"></w:lvl>
                </w:abstractNum>
                <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
            </w:numbering>
        "#;

        let numbering = parse_ok!(&xml[..]);

        assert_eq!(
            numbering.resolve(1, 0),
            ListLookupResult {
                is_list: true,
                is_ordered: true,
                style_type: ListStyleType::Decimal,
            }
        );
    }

    #[test]
    fn parse_numbering_handles_multiple_abstract_nums_and_nums() {
        let xml = br#"
            <w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum>
                <w:abstractNum w:abstractNumId="2"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl></w:abstractNum>
                <w:abstractNum w:abstractNumId="3"><w:lvl w:ilvl="0"><w:numFmt w:val="upperRoman"/></w:lvl></w:abstractNum>
                <w:num w:numId="11"><w:abstractNumId w:val="1"/></w:num>
                <w:num w:numId="22"><w:abstractNumId w:val="2"/></w:num>
                <w:num w:numId="33"><w:abstractNumId w:val="3"/></w:num>
            </w:numbering>
        "#;

        let numbering = parse_ok!(&xml[..]);

        assert_eq!(
            numbering.resolve(11, 0),
            ListLookupResult {
                is_list: true,
                is_ordered: true,
                style_type: ListStyleType::Decimal,
            }
        );
        assert_eq!(
            numbering.resolve(22, 0),
            ListLookupResult {
                is_list: true,
                is_ordered: false,
                style_type: ListStyleType::Disc,
            }
        );
        assert_eq!(
            numbering.resolve(33, 0),
            ListLookupResult {
                is_list: true,
                is_ordered: true,
                style_type: ListStyleType::UpperRoman,
            }
        );
    }

    #[test]
    fn parse_numbering_ignores_lvl_outside_abstract_num() {
        let xml = br#"
            <w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>
            </w:numbering>
        "#;

        let numbering = parse_ok!(&xml[..]);

        assert_eq!(numbering.abstract_levels.len(), 0);
        assert_eq!(numbering.num_to_abstract.len(), 0);
    }

    #[test]
    fn parse_numbering_ignores_abstract_num_id_outside_num() {
        let xml = br#"
            <w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:abstractNumId w:val="9"/>
            </w:numbering>
        "#;

        let numbering = parse_ok!(&xml[..]);

        assert_eq!(numbering.abstract_levels.len(), 0);
        assert_eq!(numbering.num_to_abstract.len(), 0);
    }

    #[test]
    fn resolve_returns_decimal_when_abstract_num_id_not_in_abstract_levels() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:num w:numId="1">
    <w:abstractNumId w:val="99"/>
  </w:num>
</w:numbering>"#;
        let numbering = parse_ok!(std::io::Cursor::new(xml));
        let result = numbering.resolve(1, 0);
        assert!(result.is_list);
        assert!(result.is_ordered);
        assert_eq!(result.style_type, ListStyleType::Decimal);
    }

    #[test]
    fn resolve_returns_decimal_when_ilvl_not_defined_in_levels() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;
        let numbering = parse_ok!(std::io::Cursor::new(xml));
        let result = numbering.resolve(1, 5);
        assert!(result.is_list);
        assert!(result.is_ordered);
        assert_eq!(result.style_type, ListStyleType::Decimal);
    }

    #[test]
    fn default_creates_empty_numbering() {
        let numbering = MinimalNumbering::default();
        let result = numbering.resolve(1, 0);
        assert!(!result.is_list);
    }

    #[test]
    fn debug_format_includes_struct_name() {
        let numbering = MinimalNumbering::new();
        let debug_str = format!("{numbering:?}");
        assert!(debug_str.contains("MinimalNumbering"));
    }

    #[test]
    fn parse_numbering_handles_non_self_closing_num_fmt() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0">
      <w:numFmt w:val="bullet"></w:numFmt>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;
        let numbering = parse_ok!(std::io::Cursor::new(xml));
        let result = numbering.resolve(1, 0);
        assert!(result.is_list);
        assert!(!result.is_ordered);
        assert_eq!(result.style_type, ListStyleType::Disc);
    }

    #[test]
    fn parse_numbering_handles_non_self_closing_abstract_num_id() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1">
    <w:abstractNumId w:val="1"></w:abstractNumId>
  </w:num>
</w:numbering>"#;
        let numbering = parse_ok!(std::io::Cursor::new(xml));
        let result = numbering.resolve(1, 0);
        assert!(result.is_list);
        assert!(result.is_ordered);
    }

    #[test]
    fn parse_numbering_returns_err_on_close_tag_depth_underflow() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
</w:numbering>
</w:extra>"#;
        let result = parse_numbering(std::io::Cursor::new(xml));
        assert!(result.is_err());
    }

    #[test]
    fn parse_numbering_returns_err_on_unclosed_elements_at_eof() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">"#;
        let result = parse_numbering(std::io::Cursor::new(xml));
        assert!(result.is_err());
    }
}
