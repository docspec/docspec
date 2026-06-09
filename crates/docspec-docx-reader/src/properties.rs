//! OOXML value parsers for run and paragraph properties.

use docspec_core::TextAlignment;

/// Parses an `ST_OnOff` value (ECMA-376 §22.9.2.7).
///
/// Element-present with no `w:val` = ON. Unknown values treated as ON (lenient).
pub fn parse_on_off(val: Option<&str>) -> bool {
    !matches!(val, Some("false" | "0" | "off"))
}

/// Vertical alignment for text (subscript, superscript, or baseline).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertAlign {
    /// Baseline (default, no vertical shift).
    None,
    /// Subscript (below baseline).
    Subscript,
    /// Superscript (above baseline).
    Superscript,
}

/// Parses a `ST_VerticalAlignRun` value. `baseline` and unknown values map to `None`.
pub fn parse_vert_align(val: Option<&str>) -> VertAlign {
    match val {
        Some("subscript") => VertAlign::Subscript,
        Some("superscript") => VertAlign::Superscript,
        _ => VertAlign::None,
    }
}

/// Parses `w:u` underline state. Unlike `ST_OnOff`, element-present with no `w:val` = OFF.
/// Only a non-`"none"` `w:val` enables underline (ECMA-376 §17.3.2.40).
pub fn parse_underline_on(val: Option<&str>) -> bool {
    match val {
        None | Some("none") => false,
        Some(_) => true,
    }
}

/// Parses `w:jc` paragraph alignment. `start`→Left, `end`→Right (no `BiDi` tracking).
pub fn parse_alignment(val: &str) -> Option<TextAlignment> {
    match val {
        "left" | "start" => Some(TextAlignment::Left),
        "right" | "end" => Some(TextAlignment::Right),
        "center" => Some(TextAlignment::Center),
        "both" | "distribute" => Some(TextAlignment::Justify),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // parse_on_off tests (8 cases)
    // ============================================================================

    #[test]
    fn parse_on_off_none_returns_true() {
        assert!(parse_on_off(None));
    }

    #[test]
    fn parse_on_off_true_returns_true() {
        assert!(parse_on_off(Some("true")));
    }

    #[test]
    fn parse_on_off_one_returns_true() {
        assert!(parse_on_off(Some("1")));
    }

    #[test]
    fn parse_on_off_on_returns_true() {
        assert!(parse_on_off(Some("on")));
    }

    #[test]
    fn parse_on_off_false_returns_false() {
        assert!(!parse_on_off(Some("false")));
    }

    #[test]
    fn parse_on_off_zero_returns_false() {
        assert!(!parse_on_off(Some("0")));
    }

    #[test]
    fn parse_on_off_off_returns_false() {
        assert!(!parse_on_off(Some("off")));
    }

    #[test]
    fn parse_on_off_unknown_returns_true() {
        assert!(parse_on_off(Some("unknown")));
    }

    // ============================================================================
    // parse_underline_on tests (4 cases)
    // ============================================================================

    #[test]
    fn parse_underline_on_none_returns_false() {
        assert!(!parse_underline_on(None));
    }

    #[test]
    fn parse_underline_on_none_value_returns_false() {
        assert!(!parse_underline_on(Some("none")));
    }

    #[test]
    fn parse_underline_on_single_returns_true() {
        assert!(parse_underline_on(Some("single")));
    }

    #[test]
    fn parse_underline_on_double_returns_true() {
        assert!(parse_underline_on(Some("double")));
    }

    // ============================================================================
    // parse_vert_align tests (5 cases)
    // ============================================================================

    #[test]
    fn parse_vert_align_subscript_returns_subscript() {
        assert_eq!(parse_vert_align(Some("subscript")), VertAlign::Subscript);
    }

    #[test]
    fn parse_vert_align_superscript_returns_superscript() {
        assert_eq!(
            parse_vert_align(Some("superscript")),
            VertAlign::Superscript
        );
    }

    #[test]
    fn parse_vert_align_baseline_returns_none() {
        assert_eq!(parse_vert_align(Some("baseline")), VertAlign::None);
    }

    #[test]
    fn parse_vert_align_none_returns_none() {
        assert_eq!(parse_vert_align(None), VertAlign::None);
    }

    #[test]
    fn parse_vert_align_unknown_returns_none() {
        assert_eq!(parse_vert_align(Some("unknown")), VertAlign::None);
    }

    // ============================================================================
    // parse_alignment tests (8 cases)
    // ============================================================================

    #[test]
    fn parse_alignment_left_returns_left() {
        assert_eq!(parse_alignment("left"), Some(TextAlignment::Left));
    }

    #[test]
    fn parse_alignment_start_returns_left() {
        assert_eq!(parse_alignment("start"), Some(TextAlignment::Left));
    }

    #[test]
    fn parse_alignment_right_returns_right() {
        assert_eq!(parse_alignment("right"), Some(TextAlignment::Right));
    }

    #[test]
    fn parse_alignment_end_returns_right() {
        assert_eq!(parse_alignment("end"), Some(TextAlignment::Right));
    }

    #[test]
    fn parse_alignment_center_returns_center() {
        assert_eq!(parse_alignment("center"), Some(TextAlignment::Center));
    }

    #[test]
    fn parse_alignment_both_returns_justify() {
        assert_eq!(parse_alignment("both"), Some(TextAlignment::Justify));
    }

    #[test]
    fn parse_alignment_distribute_returns_justify() {
        assert_eq!(parse_alignment("distribute"), Some(TextAlignment::Justify));
    }

    #[test]
    fn parse_alignment_unknown_returns_none() {
        assert_eq!(parse_alignment("mediumKashida"), None);
    }
}
