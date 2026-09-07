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
  api.convert_stream(from, to, source, chunk => {
    assert.ok(chunk instanceof Uint8Array);
    assert.ok(chunk.byteLength <= 65_536, `output chunk exceeded 64 KiB: ${chunk.byteLength}`);
    if (options.throwWrite) throw new Error('host write failed');
    if (options.promiseWrite) return Promise.resolve();
    chunks.push(new Uint8Array(chunk));
  });
  return { bytes: Buffer.concat(chunks.map(chunk => Buffer.from(chunk))), calls, chunks };
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
const retainedChunk = Buffer.from(owned.chunks[0]);
convert(full.api, 'markdown', 'html', markdown);
assert.deepEqual(Buffer.from(owned.chunks[0]), retainedChunk, 'retained output chunk changed after return');

let provisional = [];
errorCode(() => {
  const { source } = sourceFor(largeMarkdown);
  full.api.convert_stream('markdown', 'html', source, chunk => {
    provisional.push(new Uint8Array(chunk));
    throw new Error('reject staged output');
  });
}, 'IO_ERROR');
assert.ok(provisional.length > 0);
provisional = [];
assert.equal(provisional.length, 0, 'caller must discard provisional output after failure');
assert.ok(convert(full.api, 'markdown', 'html', markdown).bytes.byteLength > 0);

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
