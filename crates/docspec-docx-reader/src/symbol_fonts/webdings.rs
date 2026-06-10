//! Webdings codepoint mapping table.
//!
//! Maps Webdings font codepoints (0x21–0xFF) to standard Unicode characters.
//!
//! Source: <https://en.wikipedia.org/wiki/Webdings>
//! Last updated: 2026-06-10.

/// Webdings codepoint to Unicode character mapping.
///
/// Sorted by key (u8 codepoint). Used for fast binary search lookup.
pub(super) const TABLE: &[(u8, char)] = &[
    (0x21, '\u{1F577}'), // SPIDER
    (0x22, '\u{1F578}'), // SPIDER WEB
    (0x23, '\u{1F572}'), // SKULL
    (0x24, '\u{1F576}'), // SKULL AND CROSSBONES
    (0x25, '\u{1F3C6}'), // TROPHY
    (0x26, '\u{1F396}'), // MILITARY MEDAL
    (0x27, '\u{1F587}'), // BALLOT BOX WITH CHECK
    (0x28, '\u{1F5E8}'), // LEFT SPEECH BUBBLE
    (0x29, '\u{1F5E9}'), // RIGHT SPEECH BUBBLE
    (0x2A, '\u{1F5F0}'), // MOOD BUBBLE
    (0x2B, '\u{1F5F1}'), // ANGRY FACE BUBBLE
    (0x2C, '\u{1F336}'), // HOT PEPPER
    (0x2D, '\u{1F397}'), // REMINDER RIBBON
    (0x2E, '\u{1F67E}'), // MOOSE FACE
    (0x2F, '\u{1F67C}'), // LLAMA FACE
    (0x30, '\u{1F5D5}'), // MEMO
    (0x31, '\u{1F5D6}'), // MEMO
    (0x32, '\u{1F5D7}'), // MEMO
    (0x33, '\u{23F4}'),  // BLACK MEDIUM LEFT-POINTING TRIANGLE
    (0x34, '\u{23F5}'),  // BLACK MEDIUM RIGHT-POINTING TRIANGLE
    (0x35, '\u{23F6}'),  // BLACK MEDIUM UP-POINTING TRIANGLE
    (0x36, '\u{23F7}'),  // BLACK MEDIUM DOWN-POINTING TRIANGLE
    (0x37, '\u{23EA}'),  // BLACK LEFT-POINTING DOUBLE TRIANGLE
    (0x38, '\u{23E9}'),  // BLACK RIGHT-POINTING DOUBLE TRIANGLE
    (0x39, '\u{23EE}'),  // BLACK UP-POINTING DOUBLE TRIANGLE
    (0x3A, '\u{23ED}'),  // BLACK DOWN-POINTING DOUBLE TRIANGLE
    (0x3B, '\u{23F8}'),  // BLACK PAUSE ICON
    (0x3C, '\u{23F9}'),  // BLACK STOP ICON
    (0x3D, '\u{23FA}'),  // BLACK RECORD ICON
    (0x3E, '\u{1F5DA}'), // DESKTOP COMPUTER
    (0x3F, '\u{1F5F3}'), // BALLOT BOX WITH BALLOT
    (0x40, '\u{1F6E0}'), // HAMMER AND WRENCH
    (0x41, '\u{1F3D7}'), // BUILDING CONSTRUCTION
    (0x42, '\u{1F3D8}'), // HOUSE BUILDING
    (0x43, '\u{1F3D9}'), // DERELICT HOUSE BUILDING
    (0x44, '\u{1F3DA}'), // CLASSICAL BUILDING
    (0x45, '\u{1F3DC}'), // DESERT
    (0x46, '\u{1F3ED}'), // FACTORY
    (0x47, '\u{1F3DB}'), // JAPANESE CASTLE
    (0x48, '\u{1F3E0}'), // HOUSE
    (0x49, '\u{1F3D6}'), // MOUNTAIN
    (0x4A, '\u{1F3DD}'), // CAMPING
    (0x4B, '\u{1F6E3}'), // MOTORWAY
    (0x4C, '\u{1F50D}'), // LEFT-POINTING MAGNIFYING GLASS
    (0x4D, '\u{1F3D4}'), // MOUNTAIN CABLEWAY
    (0x4E, '\u{1F441}'), // EYE
    (0x4F, '\u{1F442}'), // EAR
    (0x50, '\u{1F3DE}'), // NATIONAL PARK
    (0x51, '\u{1F3D5}'), // TENT
    (0x52, '\u{1F6E4}'), // RAILWAY TRACK
    (0x53, '\u{1F3DF}'), // STADIUM
    (0x54, '\u{1F6F3}'), // PASSENGER SHIP
    (0x55, '\u{1F56C}'), // HAMMER
    (0x56, '\u{1F56B}'), // HAMMER AND PICK
    (0x57, '\u{1F568}'), // COMET
    (0x58, '\u{1F508}'), // SPEAKER
    (0x59, '\u{1F394}'), // SNAIL
    (0x5A, '\u{1F395}'), // SNAIL
    (0x5B, '\u{1F5EC}'), // LEFT SPEECH BUBBLE
    (0x5C, '\u{1F67D}'), // GOAT FACE
    (0x5D, '\u{1F5ED}'), // RIGHT SPEECH BUBBLE
    (0x5E, '\u{1F5EA}'), // LEFT SPEECH BUBBLE
    (0x5F, '\u{1F5EB}'), // RIGHT SPEECH BUBBLE
    (0x60, '\u{2B94}'),  // RIGHTWARDS ARROW WITH PLUS BELOW
    (0x61, '\u{2714}'),  // HEAVY CHECK MARK
    (0x62, '\u{1F6B2}'), // BICYCLE
    (0x63, '\u{25A1}'),  // WHITE SQUARE
    (0x64, '\u{1F6E1}'), // SHIELD
    (0x65, '\u{1F4E6}'), // PACKAGE
    (0x66, '\u{1F6F1}'), // ROCKET
    (0x67, '\u{25A0}'),  // BLACK SQUARE
    (0x68, '\u{1F691}'), // AMBULANCE
    (0x69, '\u{1F6C8}'), // PASSENGER SHIP
    (0x6A, '\u{1F6E9}'), // SMALL AIRPLANE
    (0x6B, '\u{1F6F0}'), // SATELLITE
    (0x6C, '\u{1F7C8}'), // AXOLOTL
    (0x6D, '\u{1F574}'), // MAN IN SUIT LEVITATING
    (0x6E, '\u{26AB}'),  // MEDIUM BLACK CIRCLE
    (0x6F, '\u{1F6E5}'), // HELICOPTER
    (0x70, '\u{1F694}'), // ONCOMING POLICE CAR
    (0x71, '\u{1F5D8}'), // MEMO
    (0x72, '\u{1F5D9}'), // MEMO
    (0x73, '\u{2753}'),  // BLACK QUESTION MARK ORNAMENT
    (0x74, '\u{1F6F2}'), // HELICOPTER
    (0x75, '\u{1F687}'), // METRO
    (0x76, '\u{1F68D}'), // ONCOMING BUS
    (0x77, '\u{26F3}'),  // FLAG IN HOLE
    (0x78, '\u{1F6C7}'), // BABY SYMBOL
    (0x79, '\u{2296}'),  // CIRCLED MINUS
    (0x7A, '\u{1F6AD}'), // NO SMOKING SYMBOL
    (0x7B, '\u{1F5EE}'), // LEFT SPEECH BUBBLE
    (0x7C, '\u{007C}'),  // VERTICAL LINE
    (0x7D, '\u{1F5EF}'), // RIGHT SPEECH BUBBLE
    (0x7E, '\u{1F5F2}'), // BALLOT BOX WITH CHECK
    (0x80, '\u{1F6B9}'), // MENS SYMBOL
    (0x81, '\u{1F6BA}'), // WOMENS SYMBOL
    (0x82, '\u{1F6C9}'), // WATER CLOSET
    (0x83, '\u{1F6CA}'), // BABY SYMBOL
    (0x84, '\u{1F6BC}'), // BABY SYMBOL
    (0x85, '\u{1F47D}'), // ALIEN MONSTER
    (0x86, '\u{1F3CB}'), // PERSON LIFTING WEIGHTS
    (0x87, '\u{26F7}'),  // SKIER
    (0x88, '\u{1F3C2}'), // SNOWBOARDER
    (0x89, '\u{1F3CC}'), // GOLFER
    (0x8A, '\u{1F3CA}'), // SWIMMER
    (0x8B, '\u{1F3C4}'), // SURFER
    (0x8C, '\u{1F3CD}'), // PERSON MOUNTAIN BIKING
    (0x8D, '\u{1F3CE}'), // PERSON DOING CARTWHEEL
    (0x8E, '\u{1F698}'), // ONCOMING AUTOMOBILE
    (0x8F, '\u{1F5E0}'), // DESKTOP COMPUTER
    (0x90, '\u{1F6E2}'), // POLICE CAR LIGHT
    (0x91, '\u{1F4B0}'), // MONEY BAG
    (0x92, '\u{1F3F7}'), // LABEL
    (0x93, '\u{1F4B3}'), // CREDIT CARD
    (0x94, '\u{1F46A}'), // FAMILY
    (0x95, '\u{1F5E1}'), // DAGGER
    (0x96, '\u{1F5E2}'), // LIPS
    (0x97, '\u{1F5E3}'), // SPEAKING HEAD IN PROFILE
    (0x98, '\u{272F}'),  // PINWHEEL STAR
    (0x99, '\u{1F584}'), // LIPS
    (0x9A, '\u{1F585}'), // TONGUE
    (0x9B, '\u{1F583}'), // NOSE
    (0x9C, '\u{1F586}'), // EAR WITH HEARING AID
    (0x9D, '\u{1F5B9}'), // BALLOT BOX WITH CHECK
    (0x9E, '\u{1F5BA}'), // BALLOT BOX WITH CHECK
    (0x9F, '\u{1F5BB}'), // BALLOT BOX WITH CHECK
    (0xA1, '\u{1F570}'), // SKULL AND CROSSBONES
    (0xA2, '\u{1F5BD}'), // BALLOT BOX WITH CHECK
    (0xA3, '\u{1F5BE}'), // BALLOT BOX WITH CHECK
    (0xA4, '\u{1F4CB}'), // CLIPBOARD
    (0xA5, '\u{1F5D2}'), // MEMO
    (0xA6, '\u{1F5D3}'), // MEMO
    (0xA7, '\u{1F4D6}'), // OPEN BOOK
    (0xA8, '\u{1F4DA}'), // BOOKS
    (0xA9, '\u{1F5DE}'), // BALLOT BOX WITH CHECK
    (0xAA, '\u{1F5DF}'), // BALLOT BOX WITH CHECK
    (0xAB, '\u{1F5C3}'), // CARD INDEX DIVIDERS
    (0xAC, '\u{1F5C2}'), // CARD INDEX DIVIDERS
    (0xAD, '\u{1F5BC}'), // BALLOT BOX WITH CHECK
    (0xAE, '\u{1F3AD}'), // PERFORMING ARTS
    (0xAF, '\u{1F39C}'), // MUSICAL NOTE
    (0xB0, '\u{1F398}'), // MUSICAL NOTE
    (0xB1, '\u{1F399}'), // MUSICAL NOTE
    (0xB2, '\u{1F3A7}'), // HEADPHONE
    (0xB3, '\u{1F4BF}'), // SPEAKER
    (0xB4, '\u{1F39E}'), // MUSICAL NOTE
    (0xB5, '\u{1F4F7}'), // CAMERA
    (0xB6, '\u{1F39F}'), // MUSICAL NOTE
    (0xB7, '\u{1F3AC}'), // CLAPPER BOARD
    (0xB8, '\u{1F4FD}'), // FILM PROJECTOR
    (0xB9, '\u{1F4F9}'), // VIDEO CAMERA
    (0xBA, '\u{1F4FE}'), // VIDEOCASSETTE
    (0xBB, '\u{1F4FB}'), // RADIO
    (0xBC, '\u{1F39A}'), // MUSICAL NOTE
    (0xBD, '\u{1F39B}'), // MUSICAL NOTE
    (0xBE, '\u{1F4FA}'), // TELEVISION
    (0xBF, '\u{1F4BB}'), // LAPTOP
    (0xC0, '\u{1F5A5}'), // DESKTOP COMPUTER
    (0xC1, '\u{1F5A6}'), // KEYBOARD
    (0xC2, '\u{1F5A7}'), // KEYBOARD
    (0xC3, '\u{1F579}'), // JOYSTICK
    (0xC4, '\u{1F3AE}'), // VIDEO GAME CONTROLLER
    (0xC5, '\u{1F57B}'), // BALLOT BOX WITH CHECK
    (0xC6, '\u{1F57C}'), // BALLOT BOX WITH CHECK
    (0xC7, '\u{1F4DF}'), // TELEPHONE RECEIVER
    (0xC8, '\u{1F581}'), // TELEPHONE RECEIVER
    (0xC9, '\u{1F580}'), // TELEPHONE RECEIVER
    (0xCA, '\u{1F5A8}'), // BALLOT BOX WITH CHECK
    (0xCB, '\u{1F5A9}'), // BALLOT BOX WITH CHECK
    (0xCC, '\u{1F5BF}'), // BALLOT BOX WITH CHECK
    (0xCD, '\u{1F5AA}'), // BALLOT BOX WITH CHECK
    (0xCE, '\u{1F5DC}'), // BALLOT BOX WITH CHECK
    (0xCF, '\u{1F512}'), // LOCK
    (0xD0, '\u{1F513}'), // OPEN LOCK
    (0xD1, '\u{1F5DD}'), // BALLOT BOX WITH CHECK
    (0xD2, '\u{1F4E5}'), // INBOX TRAY
    (0xD3, '\u{1F4E4}'), // OUTBOX TRAY
    (0xD4, '\u{1F573}'), // PERSON IN SUIT LEVITATING
    (0xD5, '\u{1F323}'), // WHITE SUN WITH SMALL CLOUD
    (0xD6, '\u{1F324}'), // WHITE SUN BEHIND SMALL CLOUD
    (0xD7, '\u{1F325}'), // WHITE SUN BEHIND LARGE CLOUD
    (0xD8, '\u{1F326}'), // WHITE SUN BEHIND RAIN CLOUD
    (0xD9, '\u{2601}'),  // CLOUD
    (0xDA, '\u{1F327}'), // CLOUD WITH RAIN
    (0xDB, '\u{1F328}'), // CLOUD WITH SNOW
    (0xDC, '\u{1F329}'), // CLOUD WITH SNOW
    (0xDD, '\u{1F32A}'), // CLOUD WITH TORNADO
    (0xDE, '\u{1F32C}'), // CLOUD WITH TORNADO
    (0xDF, '\u{1F32B}'), // CLOUD WITH TORNADO
    (0xE0, '\u{1F31C}'), // LAST QUARTER MOON
    (0xE1, '\u{1F321}'), // THERMOMETER
    (0xE2, '\u{1F6CB}'), // COUCH AND LAMP
    (0xE3, '\u{1F6CF}'), // BED
    (0xE4, '\u{1F37D}'), // FORK AND KNIFE WITH PLATE
    (0xE5, '\u{1F378}'), // COCKTAIL GLASS
    (0xE6, '\u{1F6CE}'), // DOOR
    (0xE7, '\u{1F6CD}'), // SHOPPING BAGS
    (0xE8, '\u{24C5}'),  // CIRCLED LATIN CAPITAL LETTER C
    (0xE9, '\u{267F}'),  // WHEELCHAIR SYMBOL
    (0xEA, '\u{1F6C6}'), // BABY SYMBOL
    (0xEB, '\u{1F588}'), // BALLOT BOX WITH CHECK
    (0xEC, '\u{1F393}'), // GRADUATION CAP
    (0xED, '\u{1F5E4}'), // BALLOT BOX WITH CHECK
    (0xEE, '\u{1F5E5}'), // BALLOT BOX WITH CHECK
    (0xEF, '\u{1F5E6}'), // BALLOT BOX WITH CHECK
    (0xF0, '\u{1F5E7}'), // BALLOT BOX WITH CHECK
    (0xF1, '\u{1F6EA}'), // SMALL AIRPLANE
    (0xF2, '\u{1F43F}'), // RABBIT FACE
    (0xF3, '\u{1F426}'), // BIRD
    (0xF4, '\u{1F41F}'), // FISH
    (0xF5, '\u{1F415}'), // DOG FACE
    (0xF6, '\u{1F408}'), // CAT FACE
    (0xF7, '\u{1F66C}'), // MOOSE FACE
    (0xF8, '\u{1F66E}'), // LLAMA FACE
    (0xF9, '\u{1F66D}'), // LLAMA FACE
    (0xFA, '\u{1F66F}'), // MOOSE FACE
    (0xFB, '\u{1F5FA}'), // WORLD MAP
    (0xFC, '\u{1F30D}'), // EARTH GLOBE EUROPE-AFRICA
    (0xFD, '\u{1F30F}'), // EARTH GLOBE AMERICAS
    (0xFE, '\u{1F30E}'), // EARTH GLOBE ASIA-AUSTRALIA
    (0xFF, '\u{1F54A}'), // PRAYER BEADS
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
