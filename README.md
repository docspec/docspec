# DocSpec

DocSpec is a streaming document conversion library. It converts DOCX, ODT, RTF, HTML, Markdown, and BlockNote JSON — event by event, byte by byte, without buffering the world. Built in Rust for memory-conscious systems, from microcontrollers to servers.

## Philosophy

See our [Manifesto](MANIFESTO.md) for what we stand for: memory extremism, streaming-first design, and the belief that software should earn every byte it uses.

## Supported Formats

| Format | Read | Write |
|--------|------|-------|
| DOCX | ✓ | — |
| ODT | ✓ | — |
| RTF | ✓ | ✓ |
| HTML | ✓† | — |
| Markdown | ✓ | ✓ |
| BlockNote JSON | ✓ | ✓ |

† HTML reader currently supports only `<p>` paragraph elements; other elements are silently dropped.

## Quick Start

DocSpec works through a pipeline of readers and writers. A reader (EventSource) parses a document and emits events: StartParagraph, Text, EndParagraph, StartHeading, etc. A writer (EventSink) consumes these events and produces output in the target format.

The architecture is fully decoupled. Any reader connects to any writer. A DOCX reader can feed a Markdown writer. An HTML reader can feed BlockNote JSON. The events are the contract.

To convert a document:

1. Create a reader for your input format
2. Create a writer for your output format
3. Connect them through the event pipeline
4. Let the events flow

No buffering. No intermediate representations. No loading the entire document into memory. The document streams through, event by event.

## Documentation

- **[Manifesto](MANIFESTO.md)** — Philosophy and values: memory extremism, streaming design, quality standards
- **[Architecture](ARCHITECTURE.md)** — Streaming pipeline design, reader/writer contracts
- **[Events](EVENTS.md)** — Streaming event types, well-formedness rules
- **[Coding Standards](CODING_STANDARDS.md)** — Code style rules, formatting conventions, review checklist
- **[Contributing](CONTRIBUTING.md)** — How to contribute, PR process, development workflow
- **[Testing](TESTING.md)** — Test philosophy, coverage requirements, testing patterns
- **[Security](SECURITY.md)** — Security principles, vulnerability reporting, safe practices
- **[Agents](AGENTS.md)** — Guidance for AI agents analyzing or contributing to this codebase

## Core Principles

- **Memory Conscious**: Every byte allocated must justify its existence. We measure, profile, and optimize relentlessly.
- **Streaming First**: Data flows event by event. Nothing accumulates. Everything moves.
- **Fail Fast**: On corruption or error, surface it immediately. No partial output. No silent truncation.
- **No Unsafe Code**: The workspace forbids unsafe entirely. Safety is not a limitation; it is a foundation.
- **Strict Quality**: 98% coverage for new and changed executable Rust lines in covered crates. No unwrap. No expect. No warning suppressions.

## Why Rust

We chose Rust because it gives us control: memory layout, allocation, lifetimes — without a garbage collector making decisions for us. The borrow checker enforces at compile time what other languages discover at runtime through crashes. Ownership is not a feature; it is a discipline.

## Status

DocSpec is under active development. The architecture is stable. The event model is defined. Readers and writers are being implemented incrementally.

## License

See LICENSE file.
