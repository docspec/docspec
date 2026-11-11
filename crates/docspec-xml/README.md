# docspec-xml

**XML helpers shared by the DocSpec readers**

DOCX and ODT are both made of XML parts read with `quick-xml`. This crate holds what their readers have in common: turning XML failures into DocSpec errors that name the part being read, reading an attribute, and resolving entity and character references.

It is a building block for reader crates. To convert documents, use [`docspec`](https://docs.rs/docspec) or a reader such as [`docspec-docx-reader`](https://docs.rs/docspec-docx-reader).

## Add it

```toml
[dependencies]
docspec-xml = "2"
```

## Use it

```rust
use docspec_xml::malformed;

let error = malformed("malformed content.xml", "unexpected end of file");
assert_eq!(
    error.to_string(),
    docspec_core::Error::Parse {
        message: "malformed content.xml: unexpected end of file".to_string(),
        position: None,
    }
    .to_string()
);
```

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-xml` on docs.rs](https://docs.rs/docspec-xml)
