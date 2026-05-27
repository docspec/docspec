# docspec-blocknote-writer

Converts a DocSpec event stream into [BlockNote](https://www.blocknotejs.org/) JSON. Implements the `EventSink` trait and emits JSON tokens directly to any `Write` target as events arrive — no intermediate document representation, constant memory regardless of file size.

## Supported Features

| Block type | BlockNote type |
|---|---|
| Paragraph | `paragraph` |
| Heading (levels 1–6) | `heading` |
| Block quote | `quote` |
| Preformatted / code block | `codeBlock` |
| Image | `image` |
| Table (with header and data cells) | `table` |
| Thematic break | `divider` |
| Unordered list item | `bulletListItem` |
| Ordered list item | `numberedListItem` |

Inline styles supported within text content: bold, italic, code, strikethrough, underline.

List items support arbitrary nesting via BlockNote's native `children: Block[]` arrays. The `start` prop on `numberedListItem` is preserved when the first item in a sequence carries an explicit start number.

## Not Yet Supported

- Links — `StartLink`/`EndLink` are silently ignored, so link metadata (`href`, `title`) is not emitted; `Text` events nested between them are still emitted as plain text in the surrounding inline content
- Footnotes — `StartFootnote`/`EndFootnote`/`FootnoteRef` are silently ignored; BlockNote's default schema has no equivalent
- Definition lists — `StartDefinitionList`/`StartDefinitionTerm`/`StartDefinitionDetail` (and their `End*` pairs) are silently ignored; BlockNote's default schema has no equivalent
- Captions — `StartCaption`/`EndCaption` are silently ignored
- `checkListItem` — requires upstream DocSpec event support not yet defined
- `toggleListItem` — no DocSpec event equivalent
- Custom list style markers — the `style_type` field on `StartOrderedListItem`/`StartUnorderedListItem` is always dropped (BlockNote's default schema has no equivalent); list kind is determined entirely by the event variant (ordered vs unordered)

## Usage

Wrap `BlockNoteWriter` in `StackTrackingSink` before feeding list events. The writer opens the list item's `content[]` array directly at `Start*ListItem`, and `StartParagraph` for the first paragraph of an item is a no-op. `StackTrackingSink` is still required because it normalizes paragraph boundaries (especially from sources that don't always emit an explicit `StartParagraph` inside list items), which is what lets the writer detect multi-paragraph items and transition emission into `children[]` for subsequent paragraphs.

```rust
use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::{Event, EventSink, ListStyleType, StackTrackingSink, TextStyle};

let mut buf = Vec::<u8>::new();
let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;

// Plain paragraph
writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
writer.handle_event(Event::Text {
    content: "Hello, world".to_string(),
    style: TextStyle::default(),
})?;
writer.handle_event(Event::EndParagraph)?;

// Bullet list item
writer.handle_event(Event::StartUnorderedListItem {
    id: None,
    level: 0,
    style_type: ListStyleType::Disc,
})?;
writer.handle_event(Event::Text {
    content: "First bullet".to_string(),
    style: TextStyle::default(),
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
    style: TextStyle::default(),
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
- Everything else (images, headings, code blocks, blockquotes, list items, nested tables) is dropped silently

**Non-paragraph children inside list items**: headings, images, code blocks, and blockquotes that appear as children of a list item are dropped. The first paragraph's inline content populates the item's `content[]` array; each subsequent paragraph becomes a child `paragraph` block in `children[]`.

**Blockquote + list interaction**: a list item encountered while inside a blockquote force-closes the blockquote and emits the list item at the top level as a sibling.

## Asset Streaming

When an `AssetProvider` is configured, image assets are encoded as `data:<mime>;base64,…` URIs and streamed directly into the JSON output — no intermediate `Vec<u8>` buffer. The `base64::write::EncoderWriter` uses a fixed 4 KB stack buffer. Heap usage is proportional to parser overhead, not asset size. Verified by `tests/asset_memory.rs`.

On `AssetProvider` failure mid-stream, the JSON string is closed (structurally valid) but the content is truncated (semantically incomplete). The error propagates — callers must discard partial output.

## API Documentation

See the [module-level docs](https://docs.rs/docspec-blocknote-writer) for the full API surface, including `BlockNoteWriter` and its `EventSink` implementation.

## License

See the [repository LICENSE](../../LICENSE).
