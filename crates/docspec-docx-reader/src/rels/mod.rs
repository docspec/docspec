mod hyperlinks;
mod images;
mod parts;
mod paths;

pub(crate) use hyperlinks::collect_hyperlink_map;
pub(crate) use images::collect_image_map;
pub(crate) use parts::{find_document_target, find_numbering_target, find_styles_target};
pub(crate) use paths::{derive_part_rels_path, resolve_relative_target};

use std::collections::HashMap;

use docspec_core::Error;

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

fn parse_error(message: String) -> Error {
    Error::Parse {
        message,
        position: None,
    }
}
