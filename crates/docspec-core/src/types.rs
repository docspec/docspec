//! Core types for document events and metadata.

/// Text alignment options for paragraphs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextAlignment {
    /// Center-aligned text.
    Center,
    /// Justified text.
    Justify,
    /// Left-aligned text.
    Left,
    /// Right-aligned text.
    Right,
}

/// Visual style for ordered list markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderedListStyle {
    /// Decimal numbering (1, 2, 3, ...).
    Decimal,
    /// Lowercase alphabetic (a, b, c, ...).
    LowerAlpha,
    /// Lowercase Roman numerals (i, ii, iii, ...).
    LowerRoman,
    /// Uppercase alphabetic (A, B, C, ...).
    UpperAlpha,
    /// Uppercase Roman numerals (I, II, III, ...).
    UpperRoman,
}

/// Visual style for unordered list markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnorderedListStyle {
    /// Hollow circle bullet.
    Circle,
    /// Filled circle bullet (default).
    Disc,
    /// Square bullet.
    Square,
}

/// Scope of a table header cell.
///
/// Column: header describes cells below; Row: header describes cells to the right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableHeaderScope {
    /// Header describes cells in the column below.
    Column,
    /// Header describes cells in the row to the right.
    Row,
}

/// An RGB color value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color {
    /// RGB color with red, green, and blue components (0-255).
    Rgb {
        /// Blue component (0-255).
        b: u8,
        /// Green component (0-255).
        g: u8,
        /// Red component (0-255).
        r: u8,
    },
}

/// A reference to an image asset, either embedded or external.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageSource {
    /// An embedded asset resolved through [`crate::AssetProvider`].
    Asset {
        /// The asset identifier.
        asset_id: String,
    },
    /// An external URI.
    Uri {
        /// The external resource URI.
        uri: String,
    },
}

/// An author with a name and optional email address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    /// Author's email address, if provided.
    pub email: Option<String>,
    /// Author's display name.
    pub name: String,
}

/// Metadata attached to the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentMeta {
    /// Document authors, if present.
    pub authors: Option<Vec<Author>>,
    /// Short description or abstract, if present.
    pub description: Option<String>,
    /// Document title, if present.
    pub title: Option<String>,
}
