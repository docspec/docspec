//! OPC (Open Packaging Convention) and `WordprocessingML` namespace URIs and helpers.
//!
//! This module provides constants for XML namespace URIs used in DOCX files
//! and helper functions for namespace and relationship type matching.

#![allow(dead_code, clippy::pub_with_shorthand, clippy::redundant_pub_crate)]

/// `WordprocessingML` namespace URI (transitional format).
///
/// Used in DOCX files with the transitional Office Open XML schema.
pub(crate) const WORDPROC_NS_TRANSITIONAL: &[u8] =
    b"http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// `WordprocessingML` namespace URI (strict format).
///
/// Used in DOCX files with the strict Office Open XML schema.
pub(crate) const WORDPROC_NS_STRICT: &[u8] = b"http://purl.oclc.org/ooxml/wordprocessingml/main";

/// OPC relationships namespace URI.
///
/// Used in `.rels` files to define relationships between package parts.
pub(crate) const RELATIONSHIPS_NS: &[u8] =
    b"http://schemas.openxmlformats.org/package/2006/relationships";

/// Office document relationship type (transitional format).
///
/// Identifies the relationship from the package root to the main document part.
pub(crate) const OFFICE_DOC_REL_TRANSITIONAL: &[u8] =
    b"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";

/// Office document relationship type (strict format).
///
/// Identifies the relationship from the package root to the main document part
/// in strict Office Open XML format.
pub(crate) const OFFICE_DOC_REL_STRICT: &[u8] =
    b"http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument";

/// Path to the package relationships file within the ZIP archive.
///
/// This file defines relationships between the package root and all top-level parts.
pub(crate) const PACKAGE_RELS_PATH: &str = "_rels/.rels";

/// Checks if a namespace URI is a valid `WordprocessingML` namespace.
///
/// Returns `true` if the namespace matches either the transitional or strict
/// `WordprocessingML` schema, `false` otherwise.
pub(crate) fn is_wordprocessingml(ns: &[u8]) -> bool {
    ns == WORDPROC_NS_TRANSITIONAL || ns == WORDPROC_NS_STRICT
}

/// Checks if a relationship type URI is a valid office document relationship.
///
/// Returns `true` if the relationship type matches either the transitional or strict
/// office document relationship type, `false` otherwise.
pub(crate) fn is_office_document_rel(rel_type: &[u8]) -> bool {
    rel_type == OFFICE_DOC_REL_TRANSITIONAL || rel_type == OFFICE_DOC_REL_STRICT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_wordprocessingml_matches_transitional() {
        assert!(is_wordprocessingml(WORDPROC_NS_TRANSITIONAL));
    }

    #[test]
    fn is_wordprocessingml_matches_strict() {
        assert!(is_wordprocessingml(WORDPROC_NS_STRICT));
    }

    #[test]
    fn is_wordprocessingml_rejects_garbage() {
        assert!(!is_wordprocessingml(b"random"));
    }

    #[test]
    fn is_wordprocessingml_rejects_near_miss() {
        assert!(!is_wordprocessingml(
            b"http://schemas.openxmlformats.org/wordprocessingml/2006/maim"
        ));
    }

    #[test]
    fn is_office_document_rel_matches_transitional() {
        assert!(is_office_document_rel(OFFICE_DOC_REL_TRANSITIONAL));
    }

    #[test]
    fn is_office_document_rel_rejects_garbage() {
        assert!(!is_office_document_rel(b"random"));
    }
}
