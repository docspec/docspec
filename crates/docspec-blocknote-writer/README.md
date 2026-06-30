# `docspec-blocknote-writer`

Converts a DocSpec event stream into [BlockNote](https://www.blocknotejs.org/) JSON. Implements the `EventSink` trait and emits JSON tokens directly to any `Write` target as events arrive. Conversion is streaming-first, with local buffering only for bounded conversion details such as asset data URI encoding and lifting nested table event substreams after their enclosing table closes.

## Supported Features

| Block type                         | BlockNote type                                    |
| ---------------------------------- | ------------------------------------------------- |
| Paragraph                          | `paragraph`                                       |
| Heading (levels 1–6)               | `heading`                                         |
| Block quote                        | `quote`                                           |
| Preformatted / code block          | `codeBlock`                                       |
| Image                              | `image`                                           |
| Table (with header and data cells) | `table`                                           |
| Thematic break                     | `divider`                                         |
| Unordered list item                | `bulletListItem`                                  |
| Ordered list item                  | `numberedListItem`                                |
| Inline link                        | `link` inline type with `href` (title is dropped) |

Inline styles are consumed from `StartTextStyle`/`EndTextStyle` spans around `Text` events. Bold, italic, code, strikethrough, and underline render as BlockNote style flags. Subscript and superscript are accepted but omitted because BlockNote's default schema has no equivalent representation.
Inline links (`StartLink`/`EndLink`) emit a `link` inline type with the `href`. The optional `title` field is dropped (BlockNote's default link schema has no title).

List items support arbitrary nesting via BlockNote's native `children: Block[]` arrays. The `start` prop on `numberedListItem` is preserved when the first item in a sequence carries an explicit start number. The `id` field on `Start*ListItem` events is dropped — list items never emit an `id` key, regardless of source. Upstream readers such as `docspec-docx-reader` set this field to the OOXML `numId`, which is shared by every item in the same list and so is not a per-item identifier.

### Color Styles

Two color-bearing style kinds emit JSON keys in the inline `styles` object:

- **`textColor`** — emitted when the reader sends `StartTextStyle { kind: TextColor(Color) }`. The RGB value is snapped to the nearest BlockNote palette color and written as a string, e.g. `"textColor":"red"`.
- **`backgroundColor`** — emitted when the reader sends `StartTextStyle { kind: Mark(Color) }`. Same palette snap, written as e.g. `"backgroundColor":"blue"`.

Pure black `(0, 0, 0)` and pure white `(255, 255, 255)` are both treated as BlockNote's default and produce no key for either style. White is filtered because it is the standard page-default color and the OOXML idiom for "no visible shading" (`<w:shd w:val="clear" w:fill="FFFFFF"/>`); without this guard, every such no-op shading would snap to the pastel `"gray"` palette entry. Non-RGB colors are also omitted. Any other RGB color is snapped to one of the 9 named palette entries.

#### Palette

Each style type has its own 9-entry palette of named colors. The names are the same for both, but the RGB values differ because BlockNote uses distinct pastel shades for backgrounds versus richer tones for text:

| Name     |
| -------- |
| `gray`   |
| `brown`  |
| `red`    |
| `orange` |
| `yellow` |
| `green`  |
| `blue`   |
| `purple` |
| `pink`   |

#### Palette snap algorithm

The palette snap uses squared Euclidean distance in 8-bit sRGB space. No perceptual weighting, no gamma correction. Ties are broken by iteration order. This mirrors the reference Elixir implementation.

The two palettes use different RGB values, so the same input color can snap to different names depending on whether it's a text color or a background color. For instance, the saturated red `(224, 62, 62)` snaps to background palette `"orange"` (not `"red"`) because the background palette uses pastel colors and the Euclidean distance to pastel orange is shorter than to pastel red. This is intentional and matches the reference implementation.

See [COLOR_SNAPPING.md](./COLOR_SNAPPING.md) for an example of how this snap can rename a source color end-to-end (OOXML `<w:highlight w:val="yellow"/>` → BlockNote `"backgroundColor":"orange"`) and the mechanism behind it.

## Not Yet Supported

- Footnotes — `StartFootnote`/`EndFootnote`/`FootnoteRef` are silently ignored; BlockNote's default schema has no equivalent
- Definition lists — `StartDefinitionList`/`StartDefinitionTerm`/`StartDefinitionDetail` (and their `End*` pairs) are silently ignored; BlockNote's default schema has no equivalent
- Captions — `StartCaption`/`EndCaption` are silently ignored
- `checkListItem` — requires upstream DocSpec event support not yet defined
- `toggleListItem` — no DocSpec event equivalent
- Custom list style markers — the `style_type` field on `StartOrderedListItem`/`StartUnorderedListItem` is always dropped (BlockNote's default schema has no equivalent); list kind is determined entirely by the event variant (ordered vs unordered)

## Usage

Wrap `BlockNoteWriter` in `StackTrackingSink` before feeding list events. The writer lazily opens the list item's `content[]` array when the first paragraph or inline content arrives, which lets it preserve non-default first-paragraph alignment on the list item while omitting BlockNote's default props. `StackTrackingSink` is still required because it normalizes paragraph boundaries (especially from sources that don't always emit an explicit `StartParagraph` inside list items), which is what lets the writer detect multi-paragraph items and transition emission into `children[]` for subsequent paragraphs.

```rust
use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::{Event, EventSink, ListStyleType, StackTrackingSink, TextStyleKind};

let mut buf = Vec::<u8>::new();
let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;

// Plain paragraph
writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
writer.handle_event(Event::Text {
    content: "Hello, world".to_string(),
})?;
writer.handle_event(Event::EndParagraph)?;

// Styled text uses wrapper events.
writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
writer.handle_event(Event::StartTextStyle { kind: TextStyleKind::Bold, id: None })?;
writer.handle_event(Event::Text {
    content: "Bold text".to_string(),
})?;
writer.handle_event(Event::EndTextStyle)?;
writer.handle_event(Event::EndParagraph)?;

// Bullet list item
writer.handle_event(Event::StartUnorderedListItem {
    id: None,
    level: 0,
    style_type: ListStyleType::Disc,
})?;
writer.handle_event(Event::Text {
    content: "First bullet".to_string(),
})?;
writer.handle_event(Event::EndUnorderedListItem)?;

// Numbered list item
writer.handle_event(Event::StartOrderedListItem {
    id: None,
    level: 0,
    start: Some(1),
    style_type: ListStyleType::Decimal,
})?;
writer.handle_event(Event::Text {
    content: "Step one".to_string(),
})?;
writer.handle_event(Event::EndOrderedListItem)?;

writer.handle_event(Event::EndDocument)?;
writer.finish()?;

let json = String::from_utf8(buf)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Limitations

### Table cell content

BlockNote's `tableCell.content` is `InlineContent[]` and cannot hold block-level types. Block-level events inside a cell — headings, lists, blockquotes, code blocks, dividers, images, nested tables — are buffered and **lifted and replayed as siblings** of the enclosing table.

Lift destination: the buffered blocks are drained into the nearest still-open ancestor that accepts block children — a list item or blockquote when one is open, otherwise document root. List item levels are rebased during drain to preserve nesting consistency.

Inline text adjacent to lifted content stays in the cell. Paragraph boundaries are absorbed silently.

### List item children

Headings, blockquotes, code blocks, tables, dividers, images, and nested lists inside a list item are emitted as blocks in the item's `children[]` array. The first paragraph's inline content populates the item's `content[]`; subsequent paragraphs become child `paragraph` blocks in `children[]`.

### Blockquote children

Headings, lists, code blocks, images, dividers, and nested quotes inside a blockquote are emitted as blocks in the quote's `children[]` array. Inline content (text, formatted runs) populates `content[]` until a block child arrives, at which point the writer transitions to children-only mode.

### Image in links

Images inside a link span are emitted as plain text using the alt-text of the image. This is a BlockNote limitation: BlockNote's inline content cannot contain image nodes.

## API Documentation

See the [module-level docs](https://docs.rs/docspec-blocknote-writer) for the full API surface, including `BlockNoteWriter` and its `EventSink` implementation.

## License

See the [repository LICENSE](../../LICENSE).
