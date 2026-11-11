# docspec-odt-reader

**ODT to DocSpec event stream reader**

Reads ODT (OpenDocument Text) files into DocSpec events. Only paragraphs and headings are read for now: their text, repeated spaces, tabs, and line breaks. Paragraphs and headings inside lists, table cells, and sections come out flat. Styles, link targets, images, notes, and metadata are dropped.

`OdtReader::from_path` streams `content.xml`, so memory follows the largest single XML node rather than the file; `OdtReader::from_reader` buffers it. Neither size is capped yet ([#582](https://github.com/docspec/docspec/issues/582)).

## Add it

```toml
[dependencies]
docspec-odt-reader = "2"
```

## Use it

```rust,no_run
use docspec_odt_reader::{EventSource, OdtReader};

let mut reader = OdtReader::from_path("document.odt")?;
while let Some(event) = reader.next_event()? {
    println!("{event:?}");
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-odt-reader` on docs.rs](https://docs.rs/docspec-odt-reader)
