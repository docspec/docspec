# DocSpec Architecture

This document explains how DocSpec works architecturally. For the philosophy behind our design decisions, see the companion [Manifesto](MANIFESTO.md).

---

## 1. The Problem With Traditional Document Conversion

Most document conversion libraries buffer everything: load the entire file into memory, parse it into a tree, walk the tree to transform it, then serialize the result. This fails catastrophically at scale. A 100 MB document becomes 500 MB in heap allocation. Memory consumption grows linearly with document size, often worse due to tree node overhead. Production systems crash when users upload documents larger than expected.

The buffering approach also creates architectural coupling. Readers and writers know about each other. Each conversion direction is written separately with its own bugs. Adding a new format requires modifying every existing conversion path. The combinatorial explosion becomes unmanageable.

Large allocations cause heap fragmentation, garbage collection overhead, and poor cache locality. A system that buffers everything might work for most documents but fails unpredictably under memory pressure.

DocSpec rejects all of this. Documents are streams of content, not static structures. A streaming model presents a simple interface: receive events, process them, emit output. As the system grows, the streaming model scales gracefully. New formats add linear complexity, not quadratic.

---

## 2. The Event-Based Pipeline

DocSpec treats every document as a stream of events. Instead of building a tree, we emit events as we parse. A heading becomes StartHeading and EndHeading events. A paragraph becomes StartParagraph, Text events, then EndParagraph. A table becomes StartTable, StartTableRow, StartTableCell, cell content, EndTableCell, EndTableRow, EndTable.

Events flow in strict document order. Nothing accumulates. Nothing buffers. The document enters as bytes and exits as bytes. In between, it is a stream of events flowing through a pipeline.

This architecture rests on two core abstractions: sources and sinks.

A source takes a document in some format and produces events. It walks through the input and emits events as it encounters structural elements. It knows nothing about what happens to those events.

A sink takes events and produces a document in some format. It receives events one at a time and writes output bytes incrementally. It knows nothing about where those events came from.

This decoupling is the key architectural insight. Sources and sinks are fully independent. They communicate only through the event protocol. Any source can connect to any sink. A system with four readers and four writers needs only eight implementations instead of sixteen. Complexity grows linearly, not quadratically.

The pipeline is transparent and inspectable. You can insert adapters between source and sink without either knowing the adapter exists. An adapter is a sink that consumes events and a source that emits events. This composability enables powerful transformation chains: filter events, remap event types, validate events, count events.

The event model is universal across all formats. A document is a sequence of structural elements: headings, paragraphs, lists, tables, text runs with styling, links, images. By separating structure from encoding, we achieve true interoperability.

The event flow is predictable. StartDocument begins the stream. EndDocument terminates it. Between these, events arrive in document order. Block-level elements use paired Start and End events. Inline content uses flat events with styling. This consistency makes reasoning about event sequences straightforward.

---

## 3. Why Streaming Beats Trees

The dominant alternative to streaming is building an abstract syntax tree. Parse the whole document, construct a tree in memory, walk the tree recursively, emit output. These advantages come at a steep cost that grows with document size.

Memory usage in a tree-based system is O(document size) at best, often worse. A 100 MB document typically needs 100 MB for content storage, plus overhead for tree node structures: pointers, sibling links, type tags, metadata. A 100 MB document can easily become 200-300 MB in memory.

Streaming uses O(1) memory regardless of document size. Events flow through and are immediately consumed. Only a small, fixed-size buffer is needed for the current event. A 1 KB document and a 1 GB document use essentially the same amount of memory.

The tradeoff is that streaming is strictly one-pass. You cannot go back to a previous event. You cannot look ahead. We accept this constraint because it delivers correctness at scale. Documents are fundamentally sequential structures.

When you truly need multi-pass processing, you have options: write an adapter that buffers only the specific events you need, run multiple pipelines sequentially, or externalize state to temporary storage. These are deliberate choices with explicit resource costs.

Tree structures complicate error handling and resource management. With streaming, errors occur in context. You know exactly where you are in the document. Resource management is simple: events are processed and discarded.

Performance characteristics differ dramatically. Trees cause cache misses as the processor jumps between scattered nodes. Streaming keeps data in cache-friendly sequences. Trees allocate frequently. Streaming reuses buffers. Trees fragment the heap. Streaming keeps allocation minimal and predictable.

For document conversion, the tree advantages rarely matter. You are translating from one format to another, not modifying the document structure arbitrarily. The transformations are largely local. The global context you need is minimal and can be tracked with simple state variables.

The streaming model enables processing of documents larger than available memory. A tree-based system must load the entire document before processing. A streaming system processes as it reads.

---

## 4. The Image Problem — Flagship Example

Images are where the memory abuse of traditional approaches is most visible. Traditional converters load the full image bytes before deciding what to do with them. A document containing a 50 MB embedded image requires the entire 50 MB in heap memory. A document with twenty such images requires 1 GB of image data in memory simultaneously.

DocSpec never does this. Images are represented as references in the event stream, not as data payloads. When the source encounters an image, it emits an Image event containing a reference. The event itself is tiny, just a few dozen bytes.

When the sink needs the actual image bytes, it requests them through an asset provider. The asset provider is an abstraction that can serve bytes on demand. The sink asks for the image by its reference, and the provider streams it piece by piece. The sink writes each chunk directly to its output buffer without ever holding the complete image.

A 50 MB image flows through DocSpec using a 32 KB buffer. The image size is completely irrelevant to memory usage. Whether the image is 1 MB or 500 MB, the memory footprint stays constant at the buffer size.

This approach also enables lazy loading and conditional processing. If a sink does not need the image bytes, the image data is never fetched. The reference flows through, the sink ignores it, and no memory is allocated.

---

## 5. Error Philosophy

Events flow until they cannot. When a reader encounters a malformed byte sequence, corrupted structure, or unsupported feature, it surfaces an error immediately. There is no attempt to recover and continue. There is no partial output. There is no "best effort" conversion that produces garbage.

Fail fast means the caller receives one of exactly two outcomes: a complete, correct conversion that processed the entire document successfully, or a clear error describing precisely what went wrong and where. Never a partial conversion. Never truncated output. Never corrupted output that propagates downstream.

Error information is structured, typed, and contextual. Each error carries relevant context: what operation was being performed, where in the document the problem occurred, what values were expected and what was actually found.

The fail-fast approach simplifies reasoning about the system. You do not need to wonder if a conversion succeeded partially. The result is binary: success or failure. This clarity is valuable in production systems where reliability matters.

---

## 6. The Sync-First Model

The conversion pipeline is synchronous and pull-based. The consumer calls `next_event()` on the source to request each event, then passes it to the sink. This pull model provides natural backpressure — the source only produces when the consumer is ready, which is what enables constant memory usage. There are no background threads, message channels, or async machinery.

This synchronous design is intentional. Document conversion is fundamentally CPU-bound work. Async machinery adds overhead without benefit for CPU-bound work. Task scheduling requires coordination. Context switching costs CPU cycles. Memory allocation for futures and task states adds pressure to the allocator.

The synchronous model is simpler in every way. Execution flows predictably through function calls. It is faster because there is no scheduling overhead. It uses less memory because there are no task structures. Error propagation is straightforward. Stack traces are meaningful and point directly to the source of problems.

If you need concurrency, add it at the caller level, not inside the pipeline. Run multiple conversion pipelines simultaneously in separate threads. Each individual pipeline stays synchronous and single-threaded.

The sync-first design makes the code more portable. It runs identically on all targets: native servers, WebAssembly in browsers, embedded systems. No runtime dependencies on thread pools or async executors.

---

## 7. Extension Points

The architecture is designed for extension from the ground up. Adding new formats, transformations, and behaviors is straightforward because of the clear, minimal interfaces between components.

To add a new reader, implement the source trait for that format. Parse the format according to its specification. Emit events in document order as you encounter structural elements.

To add a new writer, implement the sink trait for that format. Consume events in document order as they arrive. Produce bytes in your target format incrementally.

To add a new adapter, implement both traits. Consume events as a sink, transform or analyze them, emit new events as a source. Chain adapters together in any order: source into adapter into adapter into sink.

Common adapter patterns include:

- **Filtering adapters** remove events matching certain criteria. Strip all images to create a text-only version. Remove hidden text. Skip metadata sections.
- **Remapping adapters** change event types to transform document structure. Convert all headings down by one level. Turn block quotations into indented paragraphs.
- **Counting adapters** track statistics without modifying the event stream. Count words, paragraphs, images, tables.
- **Validating adapters** check invariants and fail fast if violated. Ensure every StartParagraph has a corresponding EndParagraph. Verify heading levels are within valid ranges.

The trait interfaces are intentionally minimal. A source has one main method: return the next event when requested. A sink has two methods: receive an event, and finish. This minimal surface area makes implementations straightforward.

Testing is simplified by the clear interfaces. To test a source, connect it to a test sink that collects events. To test a sink, create a test source that emits known events.

---

## 8. Portability

The streaming design that makes DocSpec memory-efficient on a server also makes it portable to every other computing environment. The same source code compiles for native targets, WebAssembly modules, and embedded microcontrollers.

On native servers, DocSpec runs as a library embedded in larger applications or as a standalone command-line tool. It processes documents using minimal memory, leaving resources available for other work.

In WebAssembly, DocSpec runs directly in web browsers and Node.js server environments. The streaming design is absolutely essential here. WebAssembly modules typically have limited memory available, often 2 GB or less shared with the host JavaScript environment. Streaming keeps memory usage constant and small regardless of document size.

On embedded systems, DocSpec runs on microcontrollers with kilobytes of RAM available. If the software works on a microcontroller with only 512 KB of heap available, it will work anywhere.

This portability is not an accident. It is the direct consequence of the streaming design. Zero runtime dependencies on heavy frameworks. No reliance on garbage collection. No async executor required. No assumptions about the operating system beyond basic byte input and output.

The same code path executes regardless of target. There are no conditional compilation blocks for different platforms. Just straightforward, sequential function calls that work everywhere.

---

## Summary

DocSpec converts documents through a streaming event pipeline. Documents enter as bytes, become events flowing through the pipeline, exit as bytes in the target format. Nothing accumulates in memory. Everything flows through and is immediately processed.

The event-based architecture decouples readers from writers through a shared event protocol. Any reader connects to any writer. Combinatorial power emerges from simple, composable components. Adding a new format instantly enables conversion to and from all existing formats.

Streaming uses O(1) memory regardless of document size. A 50 MB image flows through the pipeline using only a 32 KB buffer. Documents of any size convert reliably without exhausting system resources.

Errors surface immediately with full context. Fail fast. No partial output. No silent corruption. Clear, structured error information enables appropriate responses from calling code.

The pipeline is synchronous with direct function calls. No async machinery. No thread pools. No scheduling overhead. Simple, fast, and portable to every execution environment.

Extension is straightforward through well-defined interfaces. New formats implement the source or sink traits. Adapters chain between source and sink for transformation, filtering, validation, and analysis. Composable, testable, maintainable.

The same code runs natively on servers, in WebAssembly in browsers, on embedded microcontrollers. Portability is a consequence of good design. Constraints force clarity. Clarity produces quality that lasts.

For the philosophy behind these architectural choices, see the [Manifesto](MANIFESTO.md).

---

## HTTP Server (`docspec-http`)

The `docspec-http` crate exposes the DocSpec streaming pipeline over HTTP using [Axum 0.8](https://docs.rs/axum/0.8) and [tokio](https://docs.rs/tokio).

### Sync/Async Bridge

The core challenge is bridging DocSpec's synchronous `EventSource`/`EventSink` pipeline to Axum's async request handling. The solution uses a bounded channel:

```
HTTP request → convert_handler (async)
                    │
                    ├─ Validate Content-Type, Accept, UTF-8
                    │
                    ├─ Create mpsc::channel::<Result<Bytes, io::Error>>(32)
                    │
                    ├─ spawn_blocking (fire-and-forget, NO .await):
                    │       MarkdownReader::new(&body_str)
                    │       → StackTrackingSink<BlockNoteWriter<ChannelWriter>>
                    │       → ChannelWriter::blocking_send(chunk) per 8 KB
                    │
                    └─ Return Response immediately:
                            Body::from_stream(ReceiverStream::new(rx))
```

The response is returned **before** conversion completes. Axum streams the response body as the blocking task produces chunks. The bounded channel (capacity 32) provides backpressure without deadlock because the receiver is actively drained by Axum's writer.

**Critical**: Never `.await` the `JoinHandle` from `spawn_blocking` before returning the response. Doing so causes a deadlock: the handle waits for the blocking task, the blocking task fills the channel and blocks on `blocking_send`, and the channel never drains because the response hasn't started.

### Why Request Body Is Buffered

`MarkdownReader<'a>` holds a `pulldown-cmark` `Parser<'a>` that borrows from `&'a str`. This makes it impossible to own the `String` and hold a borrow from it simultaneously in safe Rust (self-referential struct). The HTTP handler buffers the request body to a `String` and passes `&body_str` to `MarkdownReader::new`. This is an accepted memory trade-off for the HTTP use case.

### Accepted Risk Profile

By design, `docspec-http` enforces:
- **No body size limit** — unlimited request bodies accepted
- **No request timeout** — conversions run to completion
- **No shutdown timeout** — in-flight requests drain unbounded on SIGINT/SIGTERM

These risks are accepted and documented. Deploy behind a reverse proxy (nginx, Caddy, etc.) for production use.

### MIME Types

| Direction | MIME Type |
|-----------|-----------|
| Input | `text/markdown` (with optional `; charset=...` parameter) |
| Output | `application/vnd.docspec.blocknote+json` |
| Errors | `application/problem+json` (RFC 7807) |

### Error Responses

All errors return [RFC 7807](https://www.rfc-editor.org/rfc/rfc7807) problem+json with:
- `type`: `https://docspec.dev/errors/{code}` URI
- `title`: Human-readable error category
- `status`: HTTP status code
- `detail`: Occurrence-specific description
