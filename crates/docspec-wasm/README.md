# docspec-wasm

WebAssembly bindings for DocSpec document conversion.

## Exports

### `convert_markdown_to_blocknote(markdown: string): string`

Converts Markdown to BlockNote JSON. Returns the complete JSON string. Simple API for small documents.

### `convert_markdown_to_blocknote_streaming(markdown: string, on_chunk: (chunk: Uint8Array) => void): void`

Converts Markdown to BlockNote JSON, calling `on_chunk` with each chunk of output as it is produced. Use this for large documents where you want to process output incrementally.

```javascript
import init, { convert_markdown_to_blocknote_streaming } from './docspec_wasm.js';

await init();

const chunks = [];
convert_markdown_to_blocknote_streaming(
  markdownInput,
  (chunk) => chunks.push(chunk)
);
const json = new TextDecoder().decode(
  new Uint8Array(chunks.flatMap(c => [...c]))
);
```

If the callback throws, the error is caught and the conversion returns an error (not a WASM trap).

## Building

```bash
wasm-pack build crates/docspec-wasm
```

## Testing

```bash
wasm-pack test --node crates/docspec-wasm
```
