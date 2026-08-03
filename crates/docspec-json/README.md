# docspec-json

**JSON writing primitives for docspec writers**

`docspec-json` is the toolkit the JSON-shaped DocSpec writers are built on. Its
`JsonEmitter` drives a pluggable `JsonBackend` through valid JSON shapes only — write a
key outside an object, or two values for one key, and you get an error before a single
byte reaches the backend. The default `StrusonBackend` streams to any `io::Write`.
Streaming, like the rest of DocSpec. (See the [Manifesto](https://github.com/docspec/docspec/blob/main/MANIFESTO.md) for why.)

Reach for this crate when you are building your own JSON writer; the
[BlockNote](https://docs.rs/docspec-blocknote-writer) and [oxa.dev](https://docs.rs/docspec-oxa-writer) writers are
built on it.

## Add it

```toml
[dependencies]
docspec-json = "1"
```

## Emit some JSON

Wrap a writer in `StrusonBackend`, hand it to a `JsonEmitter`, and build the document with
the fluent closure API — objects, arrays, keys, and scalars:

```rust
use docspec_json::{JsonEmitter, StrusonBackend};

let mut emitter = JsonEmitter::new(StrusonBackend::new(Vec::new()));
emitter.object(|doc| {
    doc.key("type").value("heading")?;
    doc.key("level").value(1u32)?;
    doc.key("content").array(|c| c.value("Hello"))
})?;
let json: Vec<u8> = emitter.finish()?;

assert_eq!(json, br#"{"type":"heading","level":1,"content":["Hello"]}"#);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Prefer to open and close frames yourself? The streaming form — `open_object`,
`close_object`, `open_array`, `close_array` — shares the same state machine.

## What's inside

- **`JsonEmitter`** — the fluent and streaming API, with stack-based shape validation.
- **`JsonBackend`** — the trait a backend implements; swap in your own.
- **`StrusonBackend`** — the default backend, streaming to any `io::Write` via `struson`.
- **`WriteVal`** — the scalar values the emitter accepts (`&str`, `bool`, `u8`, `u32`, `Null`).

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-json` on docs.rs](https://docs.rs/docspec-json)
