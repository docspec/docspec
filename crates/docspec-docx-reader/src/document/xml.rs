//! XML helpers shared by every DOCX part parser.
//!
//! Every DOCX part parser (`document.xml`, `styles.xml`, `numbering.xml`,
//! `[Content_Types].xml` and the relationship parts) reports malformed input
//! through [`parse_error`]. `[Content_Types].xml` and `numbering.xml` also read
//! attributes through [`attr_string`]; `styles.xml` and the relationship parts
//! still decode attributes in their own code.

use std::io::Read;

use docspec_core::Error;
use quick_xml::events::BytesStart;
use quick_xml::{Reader, XmlVersion};

/// Builds a position-less [`Error::Parse`] carrying `message`.
pub(crate) fn parse_error(message: String) -> Error {
    Error::Parse {
        message,
        position: None,
    }
}

/// Returns the decoded, normalized value of the attribute whose local name is `name`.
///
/// Returns `Ok(None)` when the element has no such attribute. A malformed
/// attribute or value fails with `"{error_context}: {err}"`.
pub(crate) fn attr_string<R>(
    reader: &Reader<R>,
    element: &BytesStart<'_>,
    name: &[u8],
    error_context: &str,
) -> docspec_core::Result<Option<String>>
where
    R: Read,
{
    for attribute_result in element.attributes() {
        let attribute =
            attribute_result.map_err(|err| parse_error(format!("{error_context}: {err}")))?;
        if attribute.key.local_name().as_ref() == name {
            return attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                .map(|value| Some(value.into_owned()))
                .map_err(|err| parse_error(format!("{error_context}: {err}")));
        }
    }
    Ok(None)
}
