//! Wingdings font codepoint mapping table.
//!
//! Maps Wingdings font indices (u8) to Unicode codepoints.
//! Source: <https://www.alanwood.net/demos/wingdings.html>
//! Last updated: 2026-06-10.

/// Wingdings codepoint mapping table.
///
/// Maps font indices (u8) to their corresponding Unicode characters.
/// Sorted by key for binary search compatibility.
pub(super) const TABLE: &[(u8, char)] = &[
    (0x21, '\u{1F589}'), // pen nib
    (0x22, '\u{2702}'),  // scissors
    (0x23, '\u{2701}'),  // upper blade scissors
    (0x24, '\u{1F453}'), // dress
    (0x25, '\u{1F56D}'), // scroll
    (0x26, '\u{1F56E}'), // page with curl
    (0x27, '\u{1F56F}'), // page facing up
    (0x28, '\u{1F57F}'), // document
    (0x29, '\u{2706}'),  // telephone
    (0x2A, '\u{1F582}'), // microphone
    (0x2B, '\u{1F583}'), // headphone
    (0x2C, '\u{1F4EA}'), // closed mailbox with lowered flag
    (0x2D, '\u{1F4EB}'), // closed mailbox with raised flag
    (0x2E, '\u{1F4EC}'), // open mailbox with raised flag
    (0x2F, '\u{1F4ED}'), // open mailbox with lowered flag
    (0x30, '\u{1F4C1}'), // file folder
    (0x31, '\u{1F4C2}'), // open file folder
    (0x32, '\u{1F4C4}'), // page facing up
    (0x33, '\u{1F5CF}'), // page
    (0x34, '\u{1F5D0}'), // pages
    (0x35, '\u{1F5C4}'), // file cabinet
    (0x36, '\u{231B}'),  // hourglass
    (0x37, '\u{1F5AE}'), // desktop computer
    (0x38, '\u{1F5B0}'), // printer
    (0x39, '\u{1F5B2}'), // old personal computer
    (0x3A, '\u{1F5B3}'), // old personal computer
    (0x3B, '\u{1F5B4}'), // trackball
    (0x3C, '\u{1F5AB}'), // card index
    (0x3D, '\u{1F5AC}'), // card file box
    (0x3E, '\u{2707}'),  // tape drive
    (0x3F, '\u{270D}'),  // writing hand
    (0x40, '\u{1F58E}'), // lower left fountain pen
    (0x41, '\u{270C}'),  // victory hand
    (0x42, '\u{1F44C}'), // folded hands
    (0x43, '\u{1F44D}'), // thumbs up
    (0x44, '\u{1F44E}'), // thumbs down
    (0x45, '\u{261C}'),  // white left pointing index
    (0x46, '\u{261E}'),  // white right pointing index
    (0x47, '\u{261D}'),  // white up pointing index
    (0x48, '\u{261F}'),  // white down pointing index
    (0x49, '\u{1F590}'), // raised hand with fingers splayed
    (0x4A, '\u{263A}'),  // white smiling face
    (0x4B, '\u{1F610}'), // neutral face
    (0x4C, '\u{2639}'),  // white frowning face
    (0x4D, '\u{1F4A3}'), // bomb
    (0x4E, '\u{2620}'),  // skull and crossbones
    (0x4F, '\u{1F3F3}'), // waving white flag
    (0x50, '\u{1F3F1}'), // waving black flag
    (0x51, '\u{2708}'),  // airplane
    (0x52, '\u{263C}'),  // white sun with rays
    (0x53, '\u{1F4A7}'), // water droplets
    (0x54, '\u{2744}'),  // snowflake
    (0x55, '\u{1F546}'), // helmet with white cross
    (0x56, '\u{271E}'),  // outlined greek cross
    (0x57, '\u{1F548}'), // om
    (0x58, '\u{2720}'),  // maltese cross
    (0x59, '\u{2721}'),  // star of david
    (0x5A, '\u{262A}'),  // star and crescent
    (0x5B, '\u{262F}'),  // yin yang
    (0x5C, '\u{0950}'),  // devanagari danda
    (0x5D, '\u{2638}'),  // white florette
    (0x5E, '\u{2648}'),  // aries
    (0x5F, '\u{2649}'),  // taurus
    (0x60, '\u{264A}'),  // gemini
    (0x61, '\u{264B}'),  // cancer
    (0x62, '\u{264C}'),  // leo
    (0x63, '\u{264D}'),  // virgo
    (0x64, '\u{264E}'),  // libra
    (0x65, '\u{264F}'),  // scorpius
    (0x66, '\u{2650}'),  // sagittarius
    (0x67, '\u{2651}'),  // capricornus
    (0x68, '\u{2652}'),  // aquarius
    (0x69, '\u{2653}'),  // pisces
    (0x6A, '\u{1F670}'), // abcd
    (0x6B, '\u{1F675}'), // abcd
    (0x6C, '\u{25CF}'),  // black circle
    (0x6D, '\u{1F53E}'), // black small diamond
    (0x6E, '\u{25A0}'),  // black square
    (0x6F, '\u{25A1}'),  // white square
    (0x70, '\u{1F790}'), // white square
    (0x71, '\u{2751}'),  // lower half diamond
    (0x72, '\u{2752}'),  // upper half diamond
    (0x73, '\u{2B27}'),  // white diamond with rounded corners
    (0x74, '\u{29EB}'),  // black lozenge
    (0x75, '\u{25C6}'),  // black diamond
    (0x76, '\u{2756}'),  // black diamond minus white x
    (0x77, '\u{2B25}'),  // black large diamond
    (0x78, '\u{2327}'),  // x in a rectangle box
    (0x79, '\u{2BB9}'),  // circle with horizontal bar
    (0x7A, '\u{2318}'),  // place of interest sign
    (0x7B, '\u{1F3F5}'), // flag in hole
    (0x7C, '\u{1F3F6}'), // flag in hole
    (0x7D, '\u{1F676}'), // baby
    (0x7E, '\u{1F677}'), // person
    (0x80, '\u{24EA}'),  // circled digit zero
    (0x81, '\u{2460}'),  // circled digit one
    (0x82, '\u{2461}'),  // circled digit two
    (0x83, '\u{2462}'),  // circled digit three
    (0x84, '\u{2463}'),  // circled digit four
    (0x85, '\u{2464}'),  // circled digit five
    (0x86, '\u{2465}'),  // circled digit six
    (0x87, '\u{2466}'),  // circled digit seven
    (0x88, '\u{2467}'),  // circled digit eight
    (0x89, '\u{2468}'),  // circled digit nine
    (0x8A, '\u{2469}'),  // circled digit ten
    (0x8B, '\u{24FF}'),  // negative circled digit zero
    (0x8C, '\u{2776}'),  // dingbat negative circled digit one
    (0x8D, '\u{2777}'),  // dingbat negative circled digit two
    (0x8E, '\u{2778}'),  // dingbat negative circled digit three
    (0x8F, '\u{2779}'),  // dingbat negative circled digit four
    (0x90, '\u{277A}'),  // dingbat negative circled digit five
    (0x91, '\u{277B}'),  // dingbat negative circled digit six
    (0x92, '\u{277C}'),  // dingbat negative circled digit seven
    (0x93, '\u{277D}'),  // dingbat negative circled digit eight
    (0x94, '\u{277E}'),  // dingbat negative circled digit nine
    (0x95, '\u{277F}'),  // dingbat negative circled digit ten
    (0x96, '\u{1F662}'), // face with open mouth
    (0x97, '\u{1F660}'), // grinning face
    (0x98, '\u{1F661}'), // beaming face with smiling eyes
    (0x99, '\u{1F663}'), // face with open mouth
    (0x9A, '\u{1F65E}'), // sad face
    (0x9B, '\u{1F65C}'), // weary face
    (0x9C, '\u{1F65D}'), // loudly crying face
    (0x9D, '\u{1F65F}'), // crying face
    (0x9E, '\u{00B7}'),  // middle dot
    (0x9F, '\u{2022}'),  // bullet
    (0xA1, '\u{26AA}'),  // white circle
    (0xA2, '\u{1F786}'), // white circle
    (0xA3, '\u{1F788}'), // white circle
    (0xA4, '\u{25C9}'),  // fisheye
    (0xA5, '\u{25CE}'),  // bullseye
    (0xA6, '\u{1F53F}'), // white small diamond
    (0xA7, '\u{25AA}'),  // black small square
    (0xA8, '\u{25FB}'),  // white medium square
    (0xA9, '\u{1F7C2}'), // white square
    (0xAA, '\u{2726}'),  // four pointed star
    (0xAB, '\u{2605}'),  // black star
    (0xAC, '\u{2736}'),  // six pointed star
    (0xAD, '\u{2734}'),  // eight pointed star
    (0xAE, '\u{2739}'),  // twelve pointed star
    (0xAF, '\u{2735}'),  // eight pointed pinwheel star
    (0xB0, '\u{2BD0}'),  // rotated square
    (0xB1, '\u{2316}'),  // electric arrow
    (0xB2, '\u{27E1}'),  // white concave-sided diamond
    (0xB3, '\u{2311}'),  // arc
    (0xB4, '\u{2BD1}'),  // rotated square
    (0xB5, '\u{272A}'),  // circled white star
    (0xB6, '\u{2730}'),  // shadowed white star
    (0xB7, '\u{1F550}'), // one o'clock
    (0xB8, '\u{1F551}'), // two o'clock
    (0xB9, '\u{1F552}'), // three o'clock
    (0xBA, '\u{1F553}'), // four o'clock
    (0xBB, '\u{1F554}'), // five o'clock
    (0xBC, '\u{1F555}'), // six o'clock
    (0xBD, '\u{1F556}'), // seven o'clock
    (0xBE, '\u{1F557}'), // eight o'clock
    (0xBF, '\u{1F558}'), // nine o'clock
    (0xC0, '\u{1F559}'), // ten o'clock
    (0xC1, '\u{1F55A}'), // eleven o'clock
    (0xC2, '\u{1F55B}'), // twelve o'clock
    (0xC3, '\u{2BB0}'),  // circle with left half black
    (0xC4, '\u{2BB1}'),  // circle with right half black
    (0xC5, '\u{2BB2}'),  // circle with lower half black
    (0xC6, '\u{2BB3}'),  // circle with upper half black
    (0xC7, '\u{2BB4}'),  // circle with small white circle to the right
    (0xC8, '\u{2BB5}'),  // circle with two horizontal strokes to the right
    (0xC9, '\u{2BB6}'),  // circle with small white circle to the right
    (0xCA, '\u{2BB7}'),  // circle with two horizontal strokes to the right
    (0xCB, '\u{1F66A}'), // smiling face
    (0xCC, '\u{1F66B}'), // smiling face
    (0xCD, '\u{1F655}'), // grinning face with smiling eyes
    (0xCE, '\u{1F654}'), // grinning face
    (0xCF, '\u{1F657}'), // smiling face with open mouth
    (0xD0, '\u{1F656}'), // grinning face with smiling eyes
    (0xD1, '\u{1F650}'), // grinning face
    (0xD2, '\u{1F651}'), // beaming face with smiling eyes
    (0xD3, '\u{1F652}'), // face with open mouth
    (0xD4, '\u{1F653}'), // smiling face with open mouth and cold sweat
    (0xD5, '\u{232B}'),  // erase to the left
    (0xD6, '\u{2326}'),  // erase to the right
    (0xD7, '\u{2B98}'),  // right arrow through x
    (0xD8, '\u{2B9A}'),  // leftwards arrow through x
    (0xD9, '\u{2B99}'),  // upwards arrow through x
    (0xDA, '\u{2B9B}'),  // downwards arrow through x
    (0xDB, '\u{2B88}'),  // leftwards triangle-headed arrow with double vertical stroke
    (0xDC, '\u{2B8A}'),  // rightwards triangle-headed arrow with double vertical stroke
    (0xDD, '\u{2B89}'),  // upwards triangle-headed arrow with double vertical stroke
    (0xDE, '\u{2B8B}'),  // downwards triangle-headed arrow with double vertical stroke
    (0xDF, '\u{1F868}'), // leftwards white arrow
    (0xE0, '\u{1F86A}'), // rightwards white arrow
    (0xE1, '\u{1F869}'), // upwards white arrow
    (0xE2, '\u{1F86B}'), // downwards white arrow
    (0xE3, '\u{1F86C}'), // leftwards white arrow
    (0xE4, '\u{1F86D}'), // rightwards white arrow
    (0xE5, '\u{1F86F}'), // downwards white arrow
    (0xE6, '\u{1F86E}'), // upwards white arrow
    (0xE7, '\u{1F878}'), // leftwards white arrow
    (0xE8, '\u{1F87A}'), // rightwards white arrow
    (0xE9, '\u{1F879}'), // upwards white arrow
    (0xEA, '\u{1F87B}'), // downwards white arrow
    (0xEB, '\u{1F87C}'), // leftwards white arrow
    (0xEC, '\u{1F87D}'), // rightwards white arrow
    (0xED, '\u{1F87F}'), // downwards white arrow
    (0xEE, '\u{1F87E}'), // upwards white arrow
    (0xEF, '\u{21E6}'),  // leftwards white arrow
    (0xF0, '\u{21E8}'),  // rightwards white arrow
    (0xF1, '\u{21E7}'),  // upwards white arrow
    (0xF2, '\u{21E9}'),  // downwards white arrow
    (0xF3, '\u{2B04}'),  // left right black arrow
    (0xF4, '\u{21F3}'),  // up down white arrow
    (0xF5, '\u{2B00}'),  // leftwards curved arrow
    (0xF6, '\u{2B01}'),  // rightwards curved arrow
    (0xF7, '\u{2B03}'),  // downwards curved arrow
    (0xF8, '\u{2B02}'),  // upwards curved arrow
    (0xF9, '\u{1F8AC}'), // rightwards arrow
    (0xFA, '\u{1F8AD}'), // leftwards arrow
    (0xFB, '\u{1F5F6}'), // document with picture
    (0xFC, '\u{2714}'),  // heavy check mark
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_sorted() {
        for (prev, curr) in TABLE.iter().zip(TABLE.iter().skip(1)) {
            assert!(
                prev.0 < curr.0,
                "table not sorted: {:#04X} >= {:#04X}",
                prev.0,
                curr.0
            );
        }
    }

    #[test]
    fn table_no_duplicates() {
        for (prev, curr) in TABLE.iter().zip(TABLE.iter().skip(1)) {
            assert_ne!(prev.0, curr.0, "duplicate key: {:#04X}", curr.0);
        }
    }
}
