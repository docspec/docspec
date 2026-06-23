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

List items support arbitrary nesting via BlockNote's native `children: Block[]` arrays. The `start` prop on `numberedListItem` is preserved when the first item in a sequence carries an explicit start number.

### Color Styles

Two color-bearing style kinds emit JSON keys in the inline `styles` object:

- **`textColor`** — emitted when the reader sends `StartTextStyle { kind: TextColor(Color) }`. The RGB value is snapped to the nearest BlockNote palette color and written as a string, e.g. `"textColor":"red"`.
- **`backgroundColor`** — emitted when the reader sends `StartTextStyle { kind: Mark(Color) }`. Same palette snap, written as e.g. `"backgroundColor":"blue"`.

Pure black `(0, 0, 0)` is treated as BlockNote's default and produces no key for either style. Non-RGB colors are also omitted. Any other RGB color is snapped to one of the 9 named palette entries.

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

**Table cell content**: BlockNote's `tableCell.content` is `InlineContent[]` and cannot hold block-level types. Block-level events inside a cell are handled as follows:

- `StartParagraph` / `EndParagraph` boundaries are absorbed silently (adjacent paragraphs concatenate without separator)
- `Text` and `LineBreak` are preserved
- Headings, code blocks, blockquotes, and list items are dropped silently
- Images are lifted: each `Image` event encountered inside a cell is buffered and replayed as a top-level sibling block immediately after the enclosing outermost table closes, preserving document order with any other lifted content from the same table. Inline text adjacent to a lifted image in the same cell stays in that cell — only the image moves out.
- Nested tables are lifted: their events are buffered between the inner `StartTable` and matching `EndTable`, then replayed as top-level sibling blocks immediately after the enclosing outermost table closes. Deep nesting (`A` containing `B` containing `C`) flattens to a top-level sequence (`A, B, C`) in document order. Inline text adjacent to a nested table in the same outer cell stays in that outer cell. Images inside a nested cell lift through the recursive drain — they emit as siblings of the already-lifted nested table.

**Non-paragraph children inside list items**: headings, images, code blocks, and blockquotes that appear as children of a list item are dropped. The first paragraph's inline content populates the item's `content[]` array; each subsequent paragraph becomes a child `paragraph` block in `children[]`.

**Blockquote + list interaction**: a list item encountered while inside a blockquote force-closes the blockquote and emits the list item at the top level as a sibling.

**Image-in-link**: BlockNote does not allow block-level images inside inline links. The reader closes the link before emitting the image as a sibling block, and the writer serialises that sequence directly. Content preceding the image stays inside the link; the image becomes a sibling block after the link closes; content following the image appears outside the link, losing its link wrapper. The link is empty only when the image is the sole link label, e.g. `[![alt](img.png)](url)`. All variants are lossy mappings of the original "image-inside-link" structure.

## API Documentation

See the [module-level docs](https://docs.rs/docspec-blocknote-writer) for the full API surface, including `BlockNoteWriter` and its `EventSink` implementation.

## License

See the [repository LICENSE](../../LICENSE).
