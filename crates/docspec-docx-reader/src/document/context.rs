//! Package-level context shared across a `document.xml` parse.
//!
//! [`DocxData`] is the constructor input assembled by the package layer.
//! [`PackageContext`] is the internal, read-only view the parser consults: it
//! owns the style, numbering, hyperlink and image lookups plus the shared ZIP
//! archive needed to stream embedded assets.
//!
//! Splitting this out of the reader keeps package lookups field-disjoint from
//! the mutable parse state, so a borrow of one never blocks a borrow of the
//! other.

use std::sync::{Arc, Mutex};

use docspec_core::ImageSource;

use crate::content_types::ContentTypes;
use crate::numbering::{ListLookupResult, MinimalNumbering};
use crate::package::ReadSeek;
use crate::rels::{HyperlinkMap, ImageMap};
use crate::styles::{StyleClassification, StyleList};

/// Package parts required to construct a document reader.
pub struct DocxData {
    /// Styles loaded from the styles part, used to classify paragraph and run styles.
    pub style_list: StyleList,
    /// Map of relationship Id → Target URL for every `<w:hyperlink>` `r:id` reference.
    /// Resolved from `word/_rels/document.xml.rels` at package-open time; empty if the
    /// rels file is absent or contains no hyperlink relationships.
    pub hyperlink_map: HyperlinkMap,
    /// Numbering definitions loaded from the numbering part, used to resolve list styles.
    pub numbering: MinimalNumbering,
    /// Map of relationship Id → [`crate::rels::ImageRel`] for every image relationship
    /// in the document part. Resolved from `word/_rels/document.xml.rels` at
    /// package-open time; empty if absent.
    pub image_map: ImageMap,
}

/// Read-only package data consulted while parsing `document.xml`.
///
/// Every lookup the parser needs from outside the main document part lives here,
/// so the parse state never has to reach through the reader to get at it.
pub(crate) struct PackageContext {
    style_list: StyleList,
    hyperlink_map: HyperlinkMap,
    numbering: MinimalNumbering,
    image_map: ImageMap,
    archive: Arc<Mutex<zip::ZipArchive<Box<dyn ReadSeek + 'static>>>>,
    content_types: Arc<ContentTypes>,
}

impl core::fmt::Debug for PackageContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PackageContext")
            .field("hyperlink_map", &self.hyperlink_map)
            .finish_non_exhaustive()
    }
}

impl PackageContext {
    /// Builds a context from package data plus the shared archive handles.
    pub(crate) fn new(
        data: DocxData,
        archive: Arc<Mutex<zip::ZipArchive<Box<dyn ReadSeek + 'static>>>>,
        content_types: Arc<ContentTypes>,
    ) -> Self {
        let DocxData {
            style_list,
            hyperlink_map,
            numbering,
            image_map,
        } = data;
        Self {
            style_list,
            hyperlink_map,
            numbering,
            image_map,
            archive,
            content_types,
        }
    }

    /// Classifies a paragraph or run style id, walking any `basedOn` chain.
    pub(crate) fn classify_style(&self, id: &str) -> Option<StyleClassification> {
        self.style_list.classify(id)
    }

    /// Resolves a `(numId, ilvl)` pair against the numbering part.
    pub(crate) fn resolve_numbering(&self, num_id: u32, ilvl: u32) -> ListLookupResult {
        self.numbering.resolve(num_id, ilvl)
    }

    /// Returns the target URL for a hyperlink relationship id, if present.
    pub(crate) fn hyperlink_target(&self, rid: &str) -> Option<String> {
        self.hyperlink_map.get(rid).cloned()
    }

    /// Resolves an image relationship id to an [`ImageSource`].
    ///
    /// External relationships become a URI. Internal ones become an asset handle
    /// that streams its bytes from the shared archive on demand. An unknown id is
    /// passed through as a raw asset id so the failure surfaces downstream rather
    /// than silently dropping the image.
    pub(crate) fn image_source_for_rid(&self, rid: &str) -> ImageSource {
        let asset_id = match self.image_map.get(rid) {
            Some(rel) if rel.is_external => {
                return ImageSource::Uri {
                    uri: rel.target.clone(),
                }
            }
            Some(rel) => format!("zip://{}", rel.target),
            None => rid.to_string(),
        };
        ImageSource::Asset(Arc::new(crate::asset_provider::DocxAssetHandle::new(
            Arc::clone(&self.archive),
            Arc::clone(&self.content_types),
            asset_id,
        )))
    }
}
