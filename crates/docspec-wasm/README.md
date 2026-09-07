# docspec-wasm

`docspec-wasm` builds source-only WebAssembly packages for browser and Node.js
applications. It is not published to npm or crates.io, and it targets
`wasm32-unknown-unknown`; WASI support is deferred.

The package exposes a synchronous, bounded host-I/O conversion API. A browser
uses it from a Worker so document reads and conversion do not block the page.
Node can use the same API with positional synchronous file reads and writes.
One WASM instance processes one conversion at a time. Web Workers provide
isolation and responsiveness; shared-memory WASM threads are outside this API.

## Build a package

Install Rust's target and the `wasm-pack` version used by this repository's CI,
then build from the lockfile. The generated package includes TypeScript
declarations. Use a new output directory for every distributable build so a
previous package cannot leave stale exports or metadata behind.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --locked --version 0.14.0

wasm-pack build crates/docspec-wasm \
  --target web --profile wasm-release --out-dir pkg/web-docx-to-markdown \
  -- --locked --no-default-features --features docx-reader,markdown-writer

wasm-pack build crates/docspec-wasm \
  --target nodejs --profile wasm-release --out-dir pkg/node-docx-markdown \
  -- --locked --no-default-features --features docx,markdown
```

The `web` target emits an ESM package. The `nodejs` target emits a CommonJS
package. The repository keeps all generated package contents untracked.

The `just` recipes expose the same choices while retaining their old defaults:

```sh
just wasm
just wasm-release
just wasm-release nodejs docx-reader,markdown-writer pkg/node-docx-markdown
just wasm-release web '' pkg/web-empty
```

The last command intentionally builds an empty package. Empty, reader-only, and
writer-only feature selections are valid; `convert_stream` rejects unavailable
directions before calling either host callback. The generated `.d.ts` narrows
`InputFormat` and `OutputFormat` to the formats actually compiled in, so in
those packages one or both becomes `never` and TypeScript rejects the call
outright.

## Feature selection

The default package preserves the original Markdown to BlockNote conversion:
`markdown-reader,blocknote-writer`. Select each direction independently for a
smaller package.

| Features | Compiled capability |
| --- | --- |
| `docx-reader`, `html-reader`, `markdown-reader` | Individual document readers |
| `blocknote-writer`, `html-writer`, `markdown-writer`, `oxa-writer`, `pandoc-native-writer` | Individual document writers |
| `docx`, `html`, `markdown`, `blocknote`, `oxa`, `pandoc-native` | Every implemented direction for that format |
| `all-readers`, `all-writers`, `full` | Explicit aggregate selections |

`docx` is input-only today. Enabling a format controls the corresponding format
implementation; some shared dependencies can still be present when a selected
implementation needs them.

## JavaScript API

```ts
interface ReadAtSource {
  readonly size: number;
  readAt(offset: number, length: number): Uint8Array;
}

// Either a bare function or an object with a `write` method.
type WriteChunk =
  | ((chunk: Uint8Array) => void)
  | { write(chunk: Uint8Array): void };

// Generated per build. A package compiled with `docx-reader,markdown-writer`
// declares `InputFormat = "docx"` and `OutputFormat = "markdown"`; a package
// with no reader or no writer declares `never`.
type InputFormat = "docx" | "html" | "markdown";
type OutputFormat = "blocknote" | "html" | "markdown" | "oxa" | "pandoc-native";

// Also generated per build. `IO_ERROR` and `CONVERSION_ERROR` are declared only
// where a conversion can actually run; a reader-only, writer-only or empty
// package narrows to the first three, because it rejects the direction before
// reaching host I/O.
type DocspecErrorCode =
  | "INVALID_ARGUMENT"
  | "UNSUPPORTED_INPUT_FORMAT"
  | "UNSUPPORTED_OUTPUT_FORMAT"
  | "IO_ERROR"
  | "CONVERSION_ERROR";
interface DocspecError extends Error { readonly code: DocspecErrorCode }

export function input_formats(): string[];
export function output_formats(): string[];
export function convert_stream(
  from: InputFormat,
  to: OutputFormat,
  source: ReadAtSource,
  write: WriteChunk,
): void;
```

The sink may be a bare function or an object with a `write` method. The object
form lets a sink be passed the same way as a source, and means a method
reference such as an OPFS handle's `write` is invoked with its own receiver
rather than `undefined`.

`input_formats()` and `output_formats()` return sorted canonical names for only
the capabilities compiled into the package. Call them before presenting format
choices in an application.

`convert_stream` validates `from` and `to` before reading input or writing
output. It validates source size, offsets, and lengths as nonnegative safe
integers. Each `readAt` request and `write` chunk is at most 64 KiB. A read may
return fewer bytes than requested, but an oversized response or a premature EOF
fails the conversion. Both callbacks must return synchronously; a returned
`Promise` is an `INVALID_ARGUMENT` error. The binding copies bounded chunks
across the boundary and never gives a host callback a borrowed view into WASM
memory.

Failures throw JavaScript `Error` objects with one of these stable `code`
values: `INVALID_ARGUMENT`, `UNSUPPORTED_INPUT_FORMAT`,
`UNSUPPORTED_OUTPUT_FORMAT`, `IO_ERROR`, or `CONVERSION_ERROR`. A callback
exception stops conversion immediately. A package that compiled no reader or no
writer declares only the first three, since the last two are unreachable there.

The codes distinguish who is at fault. `IO_ERROR` means the host misbehaved or
its storage failed. A malformed document reports `CONVERSION_ERROR`, including
when the damage makes the reader ask for an offset past the end of the source --
a corrupt archive is attributable to the document, never to the host.

`convert_stream` cannot be called from inside a host callback; doing so raises
`INVALID_ARGUMENT` in the nested call, which the enclosing conversion then
reports as `IO_ERROR` because the exception arose inside its callback. An
exception escaping a callback leaves the instance usable: the next
`convert_stream` re-registers cleanly and releases whatever the previous one
retained. Output received before an error is
provisional, so write to a private staging destination and publish it only after
`convert_stream` returns successfully.

When both `markdown-reader` and `blocknote-writer` are compiled, the legacy
`convert_markdown_to_blocknote(markdown)` export remains available with its
existing string input, string output, and error behavior.

## Executable examples

Build the matching package before running an example. The Node example uses
the CommonJS package above and atomically renames a staged file only after
conversion and `fsync` succeed:

```sh
node crates/docspec-wasm/examples/node-staged-output.cjs input.docx output.md
```

[`examples/browser-worker.mjs`](examples/browser-worker.mjs) is a dedicated
Worker. It uses `FileReaderSync` to read bounded `File`/`Blob` slices and an
OPFS synchronous access handle for a private staged file. It sends the opaque
stage name to the main thread only after success; on failure it closes and
deletes that stage. `FileReaderSync` is unavailable in Service Workers, so use
a dedicated Worker or another Worker type that supports it.

[`examples/small-browser-input.mjs`](examples/small-browser-input.mjs) shows
the same API with an already-owned `Uint8Array`. It deliberately accumulates
output and therefore belongs only to small inputs; the input's existing storage
cost and the returned output are both application-owned allocations.

## Memory and fidelity

The host transfer buffers, DOCX decompression, and event flow are bounded. That
does not make every conversion universally constant-memory: DOCX ZIP indexes,
relationships, style tables, and parser state consume memory, while Markdown
input currently retains its complete source before parsing. The generic API
therefore avoids unbounded host transfer buffers without promising a universal
memory ceiling.

Every enabled reader can connect to every enabled writer through the event
stream. A successful conversion does not guarantee lossless representation:
experimental writers can omit events they do not represent. The repository's
[format matrix](../../README.md#what-it-reads-and-writes) records the current
fidelity limits.

## Related

- [Architecture](../../ARCHITECTURE.md) explains the streaming event pipeline.
- [Contributing](../../CONTRIBUTING.md) lists local build and verification commands.
