# `docspec-oxa-writer`

Converts a DocSpec event stream into [oxa.dev](https://oxa.dev/) JSON. Implements the `EventSink` trait and emits JSON tokens directly to any `Write` target as events arrive — no intermediate document representation, constant memory regardless of file size.

## Supported Features

| Block type | oxa.dev type |
| --- | --- |
| Paragraph | `Paragraph` |
| Text | `Text` |

## Not Supported

The following DocSpec events are not yet supported and will be silently ignored:

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

## Usage

```rust
use docspec_oxa_writer::OxaWriter;
use docspec_core::{Event, EventSink};

let mut buf = Vec::<u8>::new();
let mut writer = OxaWriter::new(&mut buf);

writer.handle_event(Event::StartDocument { id: None, language: None, metadata: None })?;
writer.handle_event(Event::StartParagraph { alignment: None, id: None })?;
writer.handle_event(Event::Text {
    content: "Hello, world".to_string(),
})?;
writer.handle_event(Event::EndParagraph)?;
writer.handle_event(Event::EndDocument)?;
writer.finish()?;

let json = String::from_utf8(buf)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Limitations

This is a minimal scaffold. Full implementation will follow in subsequent tasks.

## License

See the [repository LICENSE](../../LICENSE).
