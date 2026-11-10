# docspec-odt-reader

**ODT to DocSpec event stream reader**

Reads ODT (OpenDocument Text) files into DocSpec events. Only paragraphs are read for now: their text, repeated spaces, tabs, and line breaks. Paragraphs inside lists, table cells, and sections come out as plain paragraphs. Headings, styles, link targets, images, notes, and metadata are dropped.

`OdtReader::from_path` streams `content.xml` in constant memory; `OdtReader::from_reader` buffers it.

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
