# `docspec-core`

Core types and traits for the DocSpec streaming document pipeline. Documents are streams of typed events flowing from `EventSource` readers to `EventSink` writers.

## Documentation

Run `just doc-open` to browse the full API documentation including all event types, traits, and field descriptions.

## Adapters

- `SkipEmptyBlocks` — source adapter that suppresses empty `Heading`, `BlockQuote`, and `Paragraph` Start/End pairs via 1-event look-back (O(1) memory). Used by `docspec-cli` and `docspec-http` by default.
