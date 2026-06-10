//! Symbol font codepoint dispatcher.

mod symbol;
mod webdings;
mod wingdings;
mod wingdings2;
mod wingdings3;

/// Symbol font dispatcher.
///
/// Maps font names to their codepoint conversion tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolFont {
    /// Wingdings font.
    Wingdings,
    /// Wingdings 2 font.
    Wingdings2,
    /// Wingdings 3 font.
    Wingdings3,
    /// Webdings font.
    Webdings,
    /// Symbol font.
    Symbol,
}

impl SymbolFont {
    /// Resolve a font name to a `SymbolFont` variant.
    ///
    /// Matching is case-insensitive. Returns `None` if the name does not match
    /// any known symbol font.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(SymbolFont::from_name("Wingdings"), Some(SymbolFont::Wingdings));
    /// assert_eq!(SymbolFont::from_name("wingdings"), Some(SymbolFont::Wingdings));
    /// assert_eq!(SymbolFont::from_name("Arial"), None);
    /// ```
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("Wingdings") {
            return Some(Self::Wingdings);
        }
        if name.eq_ignore_ascii_case("Wingdings 2") {
            return Some(Self::Wingdings2);
        }
        if name.eq_ignore_ascii_case("Wingdings 3") {
            return Some(Self::Wingdings3);
        }
        if name.eq_ignore_ascii_case("Webdings") {
            return Some(Self::Webdings);
        }
        if name.eq_ignore_ascii_case("Symbol") {
            return Some(Self::Symbol);
        }
        None
    }

    /// Convert a codepoint using this font's mapping table.
    ///
    /// Returns the Unicode character corresponding to the given codepoint,
    /// or `None` if the codepoint is not mapped in this font's table.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(SymbolFont::Wingdings.convert(0x4E), Some('☠'));
    /// assert_eq!(SymbolFont::Wingdings.convert(0x01), None);
    /// ```
    pub(crate) fn convert(self, cp: u8) -> Option<char> {
        let table = match self {
            Self::Wingdings => wingdings::TABLE,
            Self::Wingdings2 => wingdings2::TABLE,
            Self::Wingdings3 => wingdings3::TABLE,
            Self::Webdings => webdings::TABLE,
            Self::Symbol => symbol::TABLE,
        };
        let i = table.binary_search_by_key(&cp, |&(k, _)| k).ok()?;
        table.get(i).map(|&(_, c)| c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_canonical_forms() {
        assert_eq!(
            SymbolFont::from_name("Wingdings"),
            Some(SymbolFont::Wingdings)
        );
        assert_eq!(
            SymbolFont::from_name("Wingdings 2"),
            Some(SymbolFont::Wingdings2)
        );
        assert_eq!(
            SymbolFont::from_name("Wingdings 3"),
            Some(SymbolFont::Wingdings3)
        );
        assert_eq!(
            SymbolFont::from_name("Webdings"),
            Some(SymbolFont::Webdings)
        );
        assert_eq!(SymbolFont::from_name("Symbol"), Some(SymbolFont::Symbol));
    }

    #[test]
    fn from_name_case_insensitive() {
        assert_eq!(
            SymbolFont::from_name("WINGDINGS"),
            Some(SymbolFont::Wingdings)
        );
        assert_eq!(
            SymbolFont::from_name("wingdings"),
            Some(SymbolFont::Wingdings)
        );
        assert_eq!(
            SymbolFont::from_name("WiNgDiNgS"),
            Some(SymbolFont::Wingdings)
        );
        assert_eq!(
            SymbolFont::from_name("WINGDINGS 2"),
            Some(SymbolFont::Wingdings2)
        );
        assert_eq!(
            SymbolFont::from_name("webdings"),
            Some(SymbolFont::Webdings)
        );
    }

    #[test]
    fn from_name_unknown_returns_none() {
        assert_eq!(SymbolFont::from_name("Arial"), None);
        assert_eq!(SymbolFont::from_name("Comic Sans"), None);
        assert_eq!(SymbolFont::from_name(""), None);
    }

    #[test]
    fn from_name_close_but_not_match() {
        assert_eq!(SymbolFont::from_name("Wingdings3"), None); // no space
        assert_eq!(SymbolFont::from_name("Wingdings 4"), None); // wrong number
        assert_eq!(SymbolFont::from_name("Wingding"), None); // missing 's'
    }

    #[test]
    fn convert_known_mapping() {
        assert_eq!(SymbolFont::Wingdings.convert(0x4E), Some('\u{2620}'));
    }

    #[test]
    fn convert_unmapped_returns_none() {
        assert_eq!(SymbolFont::Wingdings.convert(0x01), None);
    }
}
