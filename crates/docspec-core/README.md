# docspec-core

**Core event types and traits for DocSpec streaming document conversion**

`docspec-core` is the foundation of DocSpec: the `Event` type that documents stream as,
the `EventSource` and `EventSink` traits that decouple readers from writers, and the
`pipe` helper that drives one into the other. It reads and writes nothing itself — it
defines the contract every other crate speaks. Streaming, like the rest of DocSpec.
(See the [Manifesto](https://github.com/docspec/docspec/blob/main/MANIFESTO.md) for why.)

## Add it

```toml
[dependencies]
docspec-core = "1"
```

## Speak the contract

A reader is an `EventSource`, a writer is an `EventSink`, and `pipe` moves events from one
into the other until the source runs dry. Implementing a sink is this small — here we
count the events flowing through:

```rust
use docspec_core::{Event, EventSink, Result};

/// A sink that counts the events flowing through it.
struct CountEvents(usize);

impl EventSink for CountEvents {
    fn handle_event(&mut self, _event: Event) -> Result<()> {
        self.0 += 1;
        Ok(())
    }
    fn finish(self) -> Result<()> {
        Ok(())
    }
}
```

Pair it with any `EventSource` — a DOCX, HTML, or Markdown reader from a sibling crate —
and drive them with `pipe(source, sink)`. Nothing is buffered; events flow one at a time.

## What's inside

- **`Event`** — every document structure DocSpec understands. The [`event`](https://docs.rs/docspec-core/latest/docspec_core/event/index.html) module documents each variant and its well-formedness rules.
- **`EventSource` / `EventSink` / `AssetHandle`** — the reader, writer, and streamed-asset contracts.
- **`pipe`** — connect a source to a sink with no buffering.
- **Adapters** — `SkipEmptyBlocks` drops empty heading/blockquote/paragraph pairs with O(1) look-back; `StackTrackingSink` validates nesting. Each wraps a source or sink without holding the document.

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec-core` on docs.rs](https://docs.rs/docspec-core) — every event variant, field, and rule
