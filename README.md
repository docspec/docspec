# DocSpec

DocSpec is a streaming document conversion library. It converts DOCX, ODT, RTF, HTML, Markdown, and BlockNote JSON, event by event, byte by byte, without buffering the world. Built in Rust for memory-conscious systems, from microcontrollers to servers.

> **BREAKING CHANGE**: The CLI has been restructured to use subcommands.
> `docspec input.md -o out.json` is now `docspec convert input.md -o out.json`.

## Philosophy

See our [Manifesto](MANIFESTO.md) for what we stand for: memory extremism, streaming-first design, and the belief that software should earn every byte it uses.

## Supported Formats

| Format | Read | Write |
|--------|------|-------|
| DOCX | ✓ | — |
| ODT | ✓ | — |
| RTF | ✓ | ✓ |
| HTML | ✓ | ✓ |
| Markdown | ✓ | ✓ |
| BlockNote JSON | ✓ | ✓ |

## Quick Start

DocSpec works through a pipeline of readers and writers. A reader (EventSource) parses a document and emits events: StartParagraph, Text, EndParagraph, StartHeading, etc. A writer (EventSink) consumes these events and produces output in the target format.

The architecture is fully decoupled. Any reader connects to any writer. A DOCX reader can feed a Markdown writer. An HTML reader can feed BlockNote JSON. The events are the contract.

### CLI: Convert a file

```sh
docspec convert input.md -o out.json
```

Specify formats explicitly when the file extension is ambiguous:

```sh
docspec convert --from markdown --to blocknote input.md -o out.json
```

### HTTP API: Start the server

```sh
docspec http --host 127.0.0.1 --port 3000
```

Then convert a document over HTTP:

```sh
curl -X POST http://localhost:3000/convert \
  -H 'Content-Type: text/markdown' \
  --data-binary '# Hello'
```

Check server health:

```sh
curl -i http://localhost:3000/health
# HTTP/1.1 204 No Content
```

## HTTP API

### Endpoints

**`POST /convert`** — Convert a document. Send the source document as the request body.

**`GET /health`** — Returns `204 No Content` when the server is ready.

### MIME Types

| Direction | MIME Type |
|-----------|-----------|
| Input | `text/markdown` |
| Output | `application/vnd.docspec.blocknote+json` |
| Errors | `application/problem+json` |

### Error Format

Errors follow [RFC 7807](https://www.rfc-editor.org/rfc/rfc7807) problem+json. Each error includes a `type` URI of the form `https://docspec.dev/errors/{code}`, a human-readable `title`, and an HTTP `status` code.

Example:

```json
{
  "type": "https://docspec.dev/errors/unsupported-media-type",
  "title": "Unsupported Media Type",
  "status": 415,
  "detail": "Content-Type 'application/json' is not supported. Use text/markdown."
}
```

### Security Note

No body size limit or request timeout is enforced. Deploy behind a reverse proxy (nginx, Caddy, etc.) for production use.

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
- **Strict Quality**: 98% test coverage from day one. No unwrap. No expect. No warning suppressions.

## Why Rust

We chose Rust because it gives us control: memory layout, allocation, lifetimes — without a garbage collector making decisions for us. The borrow checker enforces at compile time what other languages discover at runtime through crashes. Ownership is not a feature; it is a discipline.

## Status

DocSpec is under active development. The architecture is stable. The event model is defined. Readers and writers are being implemented incrementally.

## License

See LICENSE file.
