# docspec-odt-reader

**ODT to DocSpec event stream reader — placeholder**

This crate reserves the ODT (OpenDocument Text) input slot in DocSpec. It does not parse ODT yet: the input is ignored and `OdtReader` always emits an empty document, `StartDocument` followed by `EndDocument`. Every writer turns that into its normal empty output.

## Add it

```toml
[dependencies]
docspec-odt-reader = "2"
```

## Use it

```rust
use docspec_odt_reader::{EventSource, OdtReader};

let mut reader = OdtReader::new();
while let Some(event) = reader.next_event()? {
    println!("{event:?}");
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-odt-reader` on docs.rs](https://docs.rs/docspec-odt-reader)
