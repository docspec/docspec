#![forbid(unsafe_code)]
//! XML helpers shared by the `DocSpec` readers for XML-based formats.
//!
//! DOCX and ODT are both made of XML parts read with `quick-xml`. This crate
//! holds what their readers have in common. Every helper that can fail takes a
//! `context` such as `"malformed content.xml"`, which starts the error message
//! and names the part being read.

use core::fmt::Display;
use std::io::Read;

use docspec_core::{Error, Result};
use quick_xml::events::{BytesRef, BytesStart};
use quick_xml::{Reader, XmlVersion};

/// Builds a position-less [`Error::Parse`] carrying `message`.
#[inline]
#[must_use]
pub fn parse_error(message: String) -> Error {
    Error::Parse {
        message,
        position: None,
    }
}

/// Builds a position-less [`Error::Parse`] reading `"{context}: {err}"`.
#[inline]
#[must_use]
pub fn malformed<E>(context: &str, err: E) -> Error
where
    E: Display,
{
    parse_error(format!("{context}: {err}"))
}

/// Converts a quick-xml failure: an I/O failure keeps its kind as
/// [`Error::Io`], anything else becomes [`malformed`].
#[inline]
#[must_use]
pub fn xml_error(context: &str, err: quick_xml::Error) -> Error {
    match err {
        quick_xml::Error::Io(source) => Error::Io {
            source: std::io::Error::new(source.kind(), source.to_string()),
        },
        other => malformed(context, other),
    }
}

/// Resolves an entity or character reference to the text it stands for.
///
/// # Errors
///
/// Returns [`malformed`] if the reference is not valid UTF-8 or names an
/// unknown entity.
#[inline]
pub fn resolve_reference(context: &str, reference: &BytesRef<'_>) -> Result<String> {
    let decoded = reference.decode().map_err(|err| malformed(context, err))?;
    // quick-xml hands over the name without its delimiters; restoring them
    // reuses its entity table.
    let escaped = format!("&{decoded};");
    let unescaped = quick_xml::escape::unescape(&escaped).map_err(|err| malformed(context, err))?;
    Ok(unescaped.into_owned())
}

/// Returns the decoded, normalized value of the attribute whose local name is `name`.
///
/// Returns `Ok(None)` when the element has no such attribute.
///
/// # Errors
///
/// Returns [`malformed`] if an attribute or its value is malformed.
#[inline]
pub fn attr_string<R>(
    reader: &Reader<R>,
    element: &BytesStart<'_>,
    name: &[u8],
    context: &str,
) -> Result<Option<String>>
where
    R: Read,
{
    for attribute_result in element.attributes() {
        let attribute = attribute_result.map_err(|err| malformed(context, err))?;
        if attribute.key.local_name().as_ref() == name {
            return attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                .map(|value| Some(value.into_owned()))
                .map_err(|err| malformed(context, err));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::unwrap_used)]

    use quick_xml::events::Event;

    use super::*;

    const CONTEXT: &str = "malformed part.xml";

    fn message(error: Error) -> String {
        match error {
            Error::Parse {
                message,
                position: None,
            } => message,
            other => panic!("expected a position-less parse error, got {other:?}"),
        }
    }

    /// Runs `check` on the first start or empty element of `xml`.
    fn with_element<T>(xml: &str, check: impl Fn(&Reader<&[u8]>, &BytesStart<'_>) -> T) -> T {
        let mut reader = Reader::from_str(xml);
        match reader.read_event().unwrap() {
            Event::Start(element) | Event::Empty(element) => check(&reader, &element),
            other => panic!("expected an element, got {other:?}"),
        }
    }

    fn resolve(name: &str) -> Result<String> {
        resolve_reference(CONTEXT, &BytesRef::new(name))
    }

    #[test]
    fn parse_error_carries_the_message() {
        assert_eq!(message(parse_error("boom".to_string())), "boom");
    }

    #[test]
    fn malformed_prefixes_the_context() {
        assert_eq!(
            message(malformed(CONTEXT, "unexpected end of file")),
            "malformed part.xml: unexpected end of file"
        );
    }

    #[test]
    fn xml_error_keeps_the_io_kind() {
        let io = std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "cut short");
        let Error::Io { source } = xml_error(CONTEXT, quick_xml::Error::Io(io.into())) else {
            panic!("expected an I/O error");
        };
        assert_eq!(source.kind(), std::io::ErrorKind::UnexpectedEof);
        assert_eq!(source.to_string(), "cut short");
    }

    #[test]
    fn xml_error_turns_syntax_errors_into_parse_errors() {
        let err = Reader::from_str("<!-x").read_event().unwrap_err();
        assert_eq!(
            message(xml_error(CONTEXT, err)),
            "malformed part.xml: syntax error: comment not closed: `-->` not found before end of input"
        );
    }

    #[test]
    fn resolve_reference_handles_entities_and_character_references() {
        assert_eq!(resolve("amp").unwrap(), "&");
        assert_eq!(resolve("#x41").unwrap(), "A");
    }

    #[test]
    fn resolve_reference_rejects_unknown_entities() {
        assert_eq!(
            message(resolve("nope").unwrap_err()),
            "malformed part.xml: at 1..5: unrecognized entity `nope`"
        );
    }

    #[test]
    fn attr_string_matches_on_local_name() {
        let read = |name: &[u8]| {
            with_element(r#"<a w:val="x &amp; y" other="z"/>"#, |reader, element| {
                attr_string(reader, element, name, CONTEXT)
            })
        };
        assert_eq!(read(b"val").unwrap(), Some("x & y".to_string()));
        assert_eq!(read(b"other").unwrap(), Some("z".to_string()));
        assert_eq!(read(b"missing").unwrap(), None);
    }

    #[test]
    fn attr_string_rejects_malformed_attributes() {
        let unquoted = with_element("<a val=x/>", |reader, element| {
            attr_string(reader, element, b"val", CONTEXT)
        });
        assert_eq!(
            message(unquoted.unwrap_err()),
            "malformed part.xml: position 6: attribute value must be enclosed in `\"` or `'`"
        );
        let bad_value = with_element(r#"<a val="&nope;"/>"#, |reader, element| {
            attr_string(reader, element, b"val", CONTEXT)
        });
        assert_eq!(
            message(bad_value.unwrap_err()),
            "malformed part.xml: at 1..5: unrecognized entity `nope`"
        );
    }
}
