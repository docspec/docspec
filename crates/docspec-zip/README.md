# docspec-zip

**ZIP container access shared by the DocSpec readers**

DOCX and ODT are both ZIP archives of XML parts. This crate holds what their readers have in common: opening the archive with DocSpec errors, reading a small entry into memory, and streaming one large entry straight from the file in constant memory.

It is a building block for reader crates. To convert documents, use [`docspec`](https://docs.rs/docspec) or a reader such as [`docspec-docx-reader`](https://docs.rs/docspec-docx-reader).

## Add it

```toml
[dependencies]
docspec-zip = "2"
```

## Use it

```rust,no_run
use std::io::Read as _;
use docspec_zip::StreamingArchive;

let mut entry = StreamingArchive::open("document.odt".as_ref(), "content.xml")?;
let mut xml = String::new();
entry.read_to_string(&mut xml)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-zip` on docs.rs](https://docs.rs/docspec-zip)
