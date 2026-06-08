# `docspec-pandoc-native-writer`

Streaming [Pandoc native](https://pandoc.org/MANUAL.html#native-pandoc) (block-list) writer for DocSpec events.

Converts a DocSpec event stream into compact Pandoc native block-list syntax, suitable for being read by Pandoc's `-f native` reader. Implements the `EventSink` trait and emits output directly to any `Write` target as events arrive — no intermediate document representation, constant memory regardless of file size.

## Supported Events

| DocSpec Event | Pandoc native output |
| --- | --- |
| `StartDocument` / `EndDocument` | `[` / `]` (block list framing) |
| `StartParagraph` / `EndParagraph` | `Para [` / `]` |
| `Text` (styles dropped) | `Str "..."` |

## Not Supported

The following DocSpec events are silently ignored:

- Headings — `StartHeading` / `EndHeading`
- Block quotes — `StartBlockQuote` / `EndBlockQuote`
- Preformatted / code blocks — `StartPreformatted` / `EndPreformatted`
- Images — `Image`
- Tables — `StartTable` / `EndTable` and related events
- Thematic breaks — `ThematicBreak`
- List items — `StartOrderedListItem` / `StartUnorderedListItem` and related events
- Inline links — `StartLink` / `EndLink`
- Footnotes — `StartFootnote` / `EndFootnote` / `FootnoteRef`
- Definition lists — `StartDefinitionList` / `StartDefinitionTerm` / `StartDefinitionDetail`
- Captions — `StartCaption` / `EndCaption`
- Line breaks — `LineBreak` / `SoftBreak`
- Text formatting styles (bold, italic, etc.) — styles are accepted but silently dropped

## Usage

```rust
use docspec_pandoc_native_writer::PandocNativeWriter;
use docspec_core::{Event, EventSink, TextStyle};

let mut buf = Vec::<u8>::new();
let mut writer = PandocNativeWriter::new(&mut buf);

writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;
writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
writer.handle_event(Event::Text {
    content: "Hello, world".to_string(),
    style: TextStyle::default(),
})?;
writer.handle_event(Event::EndParagraph)?;
writer.handle_event(Event::EndDocument)?;
writer.finish()?;

let output = String::from_utf8(buf)?;
// output == "[Para [Str \"Hello, world\"]]"
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Limitations

- **Compact output only**: no pretty-printing, no indentation.
- **No metadata wrapper**: emits block-list form `[Para [...]]`, not `Pandoc (Meta ...) [...]`.
- **No `Space` inlines**: adjacent `Text` events produce adjacent `Str` constructors with no auto-inserted `Space`.
- **Targets pandoc-types >= 1.23**: uses `Str Text` (not `Str String`).

## License

See the [repository LICENSE](../../LICENSE).
