#!/usr/bin/env node
'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const Module = require('node:module');
const path = require('node:path');

const repositoryRoot = path.resolve(__dirname, '../..');
const packageRoot = path.resolve(process.argv[2] || path.join(repositoryRoot, 'target/wasm-pack'));
const expectedPath = path.join(__dirname, 'runtime-expected.json');
const recordExpected = process.env.DOCSPEC_RECORD_WASM_EXPECTED === '1';
const markdown = new TextEncoder().encode('# Hello\n\nWorld\n');
const html = new TextEncoder().encode('<!doctype html><html><body><h1>Hello</h1><p>World</p></body></html>');
const docx = fs.readFileSync(path.join(repositoryRoot, 'tests/fixtures/docx/apache-tika/testWORD_1img.docx'));
const inputs = { docx, html, markdown };
const inputNames = ['docx', 'html', 'markdown'];
const outputNames = ['blocknote', 'html', 'markdown', 'oxa', 'pandoc-native'];

function load(name) {
  const directory = path.join(packageRoot, name);
  const modulePath = path.join(directory, 'docspec_wasm.js');
  const source = fs.readFileSync(modulePath, 'utf8').replace(
    'let wasm = wasmInstance.exports;',
    'let wasm = wasmInstance.exports; exports.__test_memory = wasm.memory;',
  );
  const instrumented = new Module(modulePath, module);
  instrumented.filename = modulePath;
  instrumented.paths = Module._nodeModulePaths(directory);
  instrumented._compile(source, modulePath);
  return {
    api: instrumented.exports,
    memory: instrumented.exports.__test_memory,
  };
}

function sourceFor(bytes, options = {}) {
  const calls = [];
  return {
    calls,
    source: {
      size: options.size ?? bytes.byteLength,
      readAt(offset, length) {
        calls.push({ offset, length });
        assert.ok(length <= 65_536, `host read exceeded 64 KiB: ${length}`);
        if (options.throwRead) throw new Error('host read failed');
        if (options.promiseRead) return Promise.resolve(new Uint8Array());
        if (options.oversizedRead) return new Uint8Array(length + 1);
        if (options.emptyRead) return new Uint8Array();
        const end = Math.min(bytes.byteLength, offset + length, offset + (options.shortRead || length));
        return new Uint8Array(bytes.buffer, bytes.byteOffset + offset, Math.max(0, end - offset));
      },
    },
  };
}

function convert(api, from, to, bytes, options = {}) {
  const { source, calls } = sourceFor(bytes, options);
  const chunks = [];
  // The chunk the callback was handed, kept as-is. `chunks` holds copies, so an
  // ownership assertion made against it would compare a copy with a copy and
  // could never observe WASM memory being reused or the view being detached.
  const callbackChunks = [];
  api.convert_stream(from, to, source, chunk => {
    assert.ok(chunk instanceof Uint8Array);
    assert.ok(chunk.byteLength <= 65_536, `output chunk exceeded 64 KiB: ${chunk.byteLength}`);
    if (options.throwWrite) throw new Error('host write failed');
    if (options.promiseWrite) return Promise.resolve();
    callbackChunks.push(chunk);
    chunks.push(new Uint8Array(chunk));
  });
  return { bytes: Buffer.concat(chunks.map(chunk => Buffer.from(chunk))), calls, chunks, callbackChunks };
}

function convertDiscardingOutput(api, from, to, bytes) {
  const { source } = sourceFor(bytes);
  let outputBytes = 0;
  let writes = 0;
  api.convert_stream(from, to, source, chunk => {
    assert.ok(chunk.byteLength <= 65_536, `output chunk exceeded 64 KiB: ${chunk.byteLength}`);
    writes += 1;
    outputBytes += chunk.byteLength;
  });
  return { outputBytes, writes };
}

function errorCode(operation, expected) {
  assert.throws(operation, error => {
    assert.ok(error instanceof Error, `expected Error, received ${String(error)}`);
    assert.equal(error.code, expected);
    return true;
  });
}

function digest(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

const minimal = load('nodejs-minimal');
assert.deepEqual(minimal.api.input_formats(), ['docx']);
assert.deepEqual(minimal.api.output_formats(), ['markdown']);

let inputTouches = 0;
let outputTouches = 0;
const untouchedSource = {
  get size() { inputTouches += 1; return 0; },
  readAt() { inputTouches += 1; return new Uint8Array(); },
};
errorCode(
  () => minimal.api.convert_stream('html', 'markdown', untouchedSource, () => { outputTouches += 1; }),
  'UNSUPPORTED_INPUT_FORMAT',
);
assert.equal(inputTouches, 0);
assert.equal(outputTouches, 0);
errorCode(
  () => minimal.api.convert_stream('docx', 'html', untouchedSource, () => { outputTouches += 1; }),
  'UNSUPPORTED_OUTPUT_FORMAT',
);
assert.equal(inputTouches, 0);
assert.equal(outputTouches, 0);

const minimalDocx = convert(minimal.api, 'docx', 'markdown', docx, { shortRead: 17 });
assert.ok(minimalDocx.bytes.byteLength > 0);
assert.ok(minimalDocx.calls.length > 1);
errorCode(() => convert(minimal.api, 'docx', 'markdown', docx, { size: -1 }), 'INVALID_ARGUMENT');
errorCode(
  () => convert(minimal.api, 'docx', 'markdown', docx, { size: Number.MAX_SAFE_INTEGER + 1 }),
  'INVALID_ARGUMENT',
);
errorCode(() => convert(minimal.api, 'docx', 'markdown', docx, { promiseRead: true }), 'INVALID_ARGUMENT');
errorCode(() => convert(minimal.api, 'docx', 'markdown', docx, { promiseWrite: true }), 'INVALID_ARGUMENT');
errorCode(() => convert(minimal.api, 'docx', 'markdown', docx, { oversizedRead: true }), 'INVALID_ARGUMENT');
errorCode(() => convert(minimal.api, 'docx', 'markdown', docx, { emptyRead: true }), 'IO_ERROR');
errorCode(() => convert(minimal.api, 'docx', 'markdown', docx, { throwRead: true }), 'IO_ERROR');
errorCode(() => convert(minimal.api, 'docx', 'markdown', docx, { throwWrite: true }), 'IO_ERROR');
errorCode(
  () => convert(minimal.api, 'docx', 'markdown', Buffer.from('not a ZIP archive')),
  'CONVERSION_ERROR',
);
assert.deepEqual(convert(minimal.api, 'docx', 'markdown', docx).bytes, minimalDocx.bytes);

// --- Regression tests for the host-I/O contract ------------------------------

// A corrupt archive must be attributed to the DOCUMENT, not to host storage.
// A ZIP central directory carries a local-header offset that is never bounded
// against file length, so a corrupt file can drive the reader past EOF. That
// used to fall through to a zero-length read past `size`, surfacing as an
// IO_ERROR (or whatever the host's own bounds check threw) instead of a
// CONVERSION_ERROR -- and the resulting code varied by host implementation.
function docxWithLocalHeaderOffsetPastEof() {
  const patched = Buffer.from(docx);
  const eocd = patched.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  assert.ok(eocd > 0, 'end of central directory not found');
  // Byte 42 of a central directory record is the relative offset of its local
  // header. Point every one far past EOF, but inside the safe-integer range so
  // the seek itself is accepted and the failure lands on the read.
  let cursor = patched.readUInt32LE(eocd + 16);
  let records = 0;
  while (cursor + 46 <= patched.byteLength && patched.readUInt32LE(cursor) === 0x0201_4b50) {
    patched.writeUInt32LE(0x7fff_ffff, cursor + 42);
    records += 1;
    cursor +=
      46 +
      patched.readUInt16LE(cursor + 28) +
      patched.readUInt16LE(cursor + 30) +
      patched.readUInt16LE(cursor + 32);
  }
  assert.ok(records > 0, 'no central directory records were patched');
  return patched;
}
{
  const corrupt = docxWithLocalHeaderOffsetPastEof();
  const { source, calls } = sourceFor(corrupt);
  errorCode(
    () => minimal.api.convert_stream('docx', 'markdown', source, () => {}),
    'CONVERSION_ERROR',
  );
  // This is the fixture that actually drives the reader past `size`, so it is
  // where the EOF decision is observable. Reaching the end must be decided
  // inside the binding: a read handed to the host at or past `size` surfaces as
  // whatever that host's own bounds check throws, which is exactly the
  // host-dependent code this attribution exists to avoid.
  assert.ok(calls.length > 0, 'corrupt fixture never reached the host at all');
  assert.ok(
    calls.every(call => call.offset < corrupt.byteLength),
    'a read at or past source.size was handed to the host instead of returning EOF',
  );
}

// An exception escaping through a shim that wasm-bindgen does NOT wrap (here
// the `length` getter, read after readAt has already returned) skips Rust
// destructors. That must not strand the instance: a conversion-wide guard flag
// used to be left set, and every later call failed with a false "recursively"
// error until the module was re-instantiated.
let escaped = false;
try {
  minimal.api.convert_stream(
    'docx',
    'markdown',
    {
      size: docx.byteLength,
      readAt(offset, length) {
        const end = Math.min(docx.byteLength, offset + length);
        const real = new Uint8Array(docx.buffer, docx.byteOffset + offset, Math.max(0, end - offset));
        return new Proxy(real, {
          get(target, property, receiver) {
            if (property === 'length') throw new Error('boom from length getter');
            return Reflect.get(target, property, receiver);
          },
        });
      },
    },
    () => {},
  );
} catch {
  escaped = true;
}
assert.ok(escaped, 'a throwing length getter should surface an exception');
assert.deepEqual(
  convert(minimal.api, 'docx', 'markdown', docx).bytes,
  minimalDocx.bytes,
  'instance was left unusable after an exception escaped a host callback',
);

// The sink may be an object, invoked with itself as the receiver. Passing a
// bare method reference (`sink.write`, an OPFS handle's `handle.write`) is a
// natural spelling that would otherwise run with `this === undefined`.
const objectSink = {
  chunks: [],
  write(chunk) {
    assert.ok(this === objectSink, 'sink method must receive the sink as `this`');
    this.chunks.push(Buffer.from(chunk));
  },
};
minimal.api.convert_stream('docx', 'markdown', sourceFor(docx).source, objectSink);
assert.deepEqual(Buffer.concat(objectSink.chunks), minimalDocx.bytes);

errorCode(
  () => minimal.api.convert_stream('docx', 'markdown', sourceFor(docx).source, {}),
  'INVALID_ARGUMENT',
);
errorCode(
  () => minimal.api.convert_stream('docx', 'markdown', sourceFor(docx).source, 42),
  'INVALID_ARGUMENT',
);

// A genuine Uint8Array that happens to carry a `then` property is data, not a
// promise. The synchronicity check used to run before the type check and
// rejected it.
{
  const { source } = sourceFor(docx);
  const inner = source.readAt.bind(source);
  const thenableSource = {
    size: docx.byteLength,
    readAt(offset, length) {
      const bytes = new Uint8Array(inner(offset, length));
      bytes.then = () => {};
      return bytes;
    },
  };
  const chunks = [];
  minimal.api.convert_stream('docx', 'markdown', thenableSource, chunk => {
    chunks.push(Buffer.from(chunk));
  });
  assert.deepEqual(Buffer.concat(chunks), minimalDocx.bytes);
}

// Re-entering from inside a host callback is still rejected. The inner call's
// INVALID_ARGUMENT is thrown inside the callback, so the outer conversion sees
// it as a host I/O failure.
errorCode(
  () =>
    minimal.api.convert_stream('docx', 'markdown', sourceFor(docx).source, () => {
      minimal.api.convert_stream('docx', 'markdown', sourceFor(docx).source, () => {});
    }),
  'IO_ERROR',
);

// The rejected re-entrant call must not have disturbed the outer instance.
assert.deepEqual(convert(minimal.api, 'docx', 'markdown', docx).bytes, minimalDocx.bytes);

const full = load('nodejs-full');
assert.deepEqual(full.api.input_formats(), inputNames);
assert.deepEqual(full.api.output_formats(), outputNames);
assert.equal(
  full.api.convert_markdown_to_blocknote('# Hello\n\nWorld'),
  '[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Hello","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"World","styles":{}}],"children":[]}]',
);

const expected = JSON.parse(fs.readFileSync(expectedPath, 'utf8'));
const matrix = {};
for (const from of inputNames) {
  for (const to of outputNames) {
    const key = `${from}->${to}`;
    const result = convert(full.api, from, to, inputs[from], { shortRead: 31 });
    matrix[key] = { bytes: result.bytes.byteLength, sha256: digest(result.bytes) };
    if (!recordExpected) {
      assert.deepEqual(matrix[key], expected.matrix[key], `${key} exact output changed`);
    }
    assert.ok(result.calls.every(call => call.length <= 65_536));
    assert.ok(result.calls.every(call => Number.isSafeInteger(call.offset) && call.offset >= 0));
  }
}
assert.equal(Object.keys(matrix).length, 15);
assert.match(convert(full.api, 'docx', 'blocknote', docx).bytes.toString(), /data:image\/png;base64,/);

const largeMarkdown = new TextEncoder().encode('bounded text '.repeat(20_000));
const owned = convert(full.api, 'markdown', 'html', largeMarkdown);
assert.ok(owned.calls.length > 1);
assert.ok(owned.calls.some(call => call.length === 65_536), 'large input never exercised the 64 KiB bound');
// The chunk handed to the callback is the binding's to give away, and it must
// survive a later conversion untouched. Asserted against `callbackChunks`, not
// `chunks`: the latter are copies taken inside the callback, so comparing them
// would pass even if the callback had been handed a live view into WASM memory.
const ownedChunk = owned.callbackChunks[0];
const retainedChunk = Buffer.from(ownedChunk);
const cleanMarkdownHtml = convert(full.api, 'markdown', 'html', markdown).bytes;
assert.ok(cleanMarkdownHtml.byteLength > 0);
assert.ok(ownedChunk.byteLength > 0, 'retained output chunk was detached by a later conversion');
assert.deepEqual(Buffer.from(ownedChunk), retainedChunk, 'retained output chunk changed after return');

// Output emitted before a failure is provisional. What the binding owes the
// caller is that discarding it is *sufficient*: the next conversion must be
// byte-identical to one run on a clean instance, with no residue from the
// aborted attempt.
const provisional = [];
errorCode(() => {
  const { source } = sourceFor(largeMarkdown);
  full.api.convert_stream('markdown', 'html', source, chunk => {
    provisional.push(new Uint8Array(chunk));
    throw new Error('reject staged output');
  });
}, 'IO_ERROR');
assert.ok(provisional.length > 0, 'the failing conversion emitted no provisional output to discard');
assert.deepEqual(
  convert(full.api, 'markdown', 'html', markdown).bytes,
  cleanMarkdownHtml,
  'provisional output from a failed conversion leaked into the next one',
);

const memoryResults = [];
for (const count of [100, 1000, 10000, 100000]) {
  const fixture = fs.readFileSync(path.join(repositoryRoot, 'target/wasm-memory-fixtures', `${count}.docx`));
  const before = full.memory.buffer.byteLength;
  const { outputBytes, writes } = convertDiscardingOutput(full.api, 'docx', 'markdown', fixture);
  const after = full.memory.buffer.byteLength;
  assert.ok(writes <= Math.ceil(outputBytes / 65_536) + 1, 'writer emitted unbatched host callbacks');
  memoryResults.push({ paragraphs: count, archiveBytes: fixture.byteLength, outputBytes, writes, before, after });
}
const memorySizes = memoryResults.flatMap(result => [result.before, result.after]);
const memoryGrowthBytes = Math.max(...memorySizes) - Math.min(...memorySizes);

const report = {
  capabilities: { input: inputNames, output: outputNames },
  matrix,
  memory: { growthBytes: memoryGrowthBytes, samples: memoryResults },
};
fs.writeFileSync(path.join(packageRoot, 'node-runtime-results.json'), `${JSON.stringify(report, null, 2)}\n`);
if (recordExpected) {
  fs.writeFileSync(path.join(packageRoot, 'runtime-expected.proposed.json'), `${JSON.stringify({ matrix }, null, 2)}\n`);
}
console.log(JSON.stringify(report, null, 2));
