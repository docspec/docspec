//! `BlockNote` default color palette and nearest-color snapping.
//!
//! Colors are snapped to the nearest named palette entry using squared Euclidean
//! distance in 8-bit sRGB space. No perceptual weighting, no gamma correction.
//! Pure black `(0, 0, 0)` returns `None` (`BlockNote` default — no key emitted).

/// Snaps an RGB color to the nearest `BlockNote` text-color palette entry.
///
/// Returns `None` for pure black `(0, 0, 0)` (`BlockNote` default).
#[inline]
#[must_use]
pub fn nearest_text_color(color: &docspec_core::Color) -> Option<&'static str> {
    nearest(color, TEXT_PALETTE)
}

/// Snaps an RGB color to the nearest `BlockNote` background-color palette entry.
///
/// Returns `None` for pure black `(0, 0, 0)` (`BlockNote` default).
#[inline]
#[must_use]
pub fn nearest_background_color(color: &docspec_core::Color) -> Option<&'static str> {
    nearest(color, BACKGROUND_PALETTE)
}

fn nearest(
    color: &docspec_core::Color,
    palette: &[(&'static str, u8, u8, u8)],
) -> Option<&'static str> {
    let docspec_core::Color::Rgb { r, g, b } = color else {
        return None;
    };
    if (*r, *g, *b) == (0, 0, 0) {
        return None;
    }
    palette
        .iter()
        .min_by_key(|(_, pr, pg, pb)| {
            let dr = u32::from(r.abs_diff(*pr));
            let dg = u32::from(g.abs_diff(*pg));
            let db = u32::from(b.abs_diff(*pb));
            dr.wrapping_mul(dr)
                .wrapping_add(dg.wrapping_mul(dg))
                .wrapping_add(db.wrapping_mul(db))
        })
        .map(|(name, _, _, _)| *name)
}

const TEXT_PALETTE: &[(&str, u8, u8, u8)] = &[
    ("gray", 155, 154, 151),
    ("brown", 100, 71, 58),
    ("red", 224, 62, 62),
    ("orange", 217, 115, 13),
    ("yellow", 223, 171, 1),
    ("green", 77, 100, 97),
    ("blue", 11, 110, 153),
    ("purple", 105, 64, 165),
    ("pink", 173, 26, 114),
];

const BACKGROUND_PALETTE: &[(&str, u8, u8, u8)] = &[
    ("gray", 235, 236, 237),
    ("brown", 233, 229, 227),
    ("red", 251, 228, 228),
    ("orange", 246, 233, 217),
    ("yellow", 251, 243, 219),
    ("green", 221, 237, 234),
    ("blue", 221, 235, 241),
    ("purple", 234, 228, 242),
    ("pink", 244, 223, 235),
];

#[cfg(test)]
mod tests {
    use super::*;
    use docspec_core::Color;

    macro_rules! snap_cases {
        ($snap_fn:ident; $(($r:literal, $g:literal, $b:literal) => $name:literal,)+) => {
            $(
                paste::paste! {
                    #[test]
                    fn [<r $r _g $g _b $b _matches_ $name>]() {
                        assert_eq!(
                            $snap_fn(&Color::Rgb { r: $r, g: $g, b: $b }),
                            Some($name),
                        );
                    }
                }
            )+
        };
    }

    snap_cases! {
        nearest_text_color;
        // Exact palette matches — one per palette entry
        (155, 154, 151) => "gray",
        (100,  71,  58) => "brown",
        (224,  62,  62) => "red",
        (217, 115,  13) => "orange",
        (223, 171,   1) => "yellow",
        ( 77, 100,  97) => "green",
        ( 11, 110, 153) => "blue",
        (105,  64, 165) => "purple",
        (173,  26, 114) => "pink",
        // Snap — pure red (255,0,0) is closest to text palette red (224,62,62)
        (255,   0,   0) => "red",
    }

    snap_cases! {
        nearest_background_color;
        // Exact palette matches — one per palette entry
        (235, 236, 237) => "gray",
        (233, 229, 227) => "brown",
        (251, 228, 228) => "red",
        (246, 233, 217) => "orange",
        (251, 243, 219) => "yellow",
        (221, 237, 234) => "green",
        (221, 235, 241) => "blue",
        (234, 228, 242) => "purple",
        (244, 223, 235) => "pink",
        // AC9: saturated red (224,62,62) snaps to background "orange" NOT "red".
        // Background red (251,228,228) is pastel; distance to pastel orange
        // (246,233,217) is shorter than to pastel red.
        (224,  62,  62) => "orange",
        // Pure yellow (255,255,0):
        //   distance to yellow (251,243,219) = 4² + 12² + 219² = 48121
        //   distance to orange (246,233,217) = 9² + 22² + 217² = 47654 → orange wins
        (255, 255,   0) => "orange",
    }

    #[test]
    fn nearest_text_color_black_returns_none() {
        assert_eq!(nearest_text_color(&Color::Rgb { r: 0, g: 0, b: 0 }), None);
    }

    #[test]
    fn nearest_background_color_black_returns_none() {
        assert_eq!(
            nearest_background_color(&Color::Rgb { r: 0, g: 0, b: 0 }),
            None
        );
    }
}
