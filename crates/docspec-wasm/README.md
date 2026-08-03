# docspec-wasm

**WebAssembly bindings for the DocSpec document conversion library.**

WebAssembly bindings for DocSpec, built with `wasm-bindgen`. Today they expose one entry
point — Markdown to BlockNote JSON — so a browser can run the same streaming conversion
the native crates do, with no server round-trip. Streaming, like the rest of DocSpec.
(See the [Manifesto](https://github.com/docspec/docspec/blob/main/MANIFESTO.md) for why.)

This crate is not published to crates.io; you build it from the workspace.

## Build it

```sh
wasm-pack build crates/docspec-wasm --target web
```

## Use it

Import the generated package and call the converter. It returns the BlockNote JSON string,
or throws a JavaScript error if the Markdown can't be parsed or serialized:

```js
import init, { convert_markdown_to_blocknote } from "./pkg/docspec_wasm.js";

await init();
const blocknote = convert_markdown_to_blocknote("# Hello\n\nWorld");
```

## What it handles today

One conversion: Markdown (CommonMark + GFM) to BlockNote JSON. Every other format pair
runs in the native crates and the [HTTP server](https://docs.rs/docspec-http) — they are not yet
exposed through the WebAssembly surface.

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [`docspec`](https://docs.rs/docspec) — the native facade these bindings wrap
