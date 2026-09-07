import init, { convert_stream, input_formats, output_formats } from '/pkg/docspec_wasm.js';

const inputNames = ['docx', 'html', 'markdown'];
const outputNames = ['blocknote', 'html', 'markdown', 'oxa', 'pandoc-native'];

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function equal(actual, expected, message) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${message}: ${JSON.stringify(actual)}`);
}

function sourceFor(bytes, shortRead = 29) {
  const blob = new Blob([bytes]);
  const reader = new FileReaderSync();
  const calls = [];
  return {
    calls,
    source: {
      size: blob.size,
      readAt(offset, length) {
        calls.push({ offset, length });
        assert(length <= 65_536, `host read exceeded 64 KiB: ${length}`);
        return new Uint8Array(reader.readAsArrayBuffer(blob.slice(offset, offset + Math.min(length, shortRead))));
      },
    },
  };
}

function convert(from, to, bytes) {
  const { source, calls } = sourceFor(bytes);
  const chunks = [];
  // The chunk as the callback received it. `chunks` holds copies, so asserting
  // ownership against them would compare a copy with a copy and could never
  // observe reused WASM memory or a detached view.
  const callbackChunks = [];
  convert_stream(from, to, source, chunk => {
    assert(chunk instanceof Uint8Array, 'writer did not receive Uint8Array');
    assert(chunk.byteLength <= 65_536, `output chunk exceeded 64 KiB: ${chunk.byteLength}`);
    callbackChunks.push(chunk);
    chunks.push(new Uint8Array(chunk));
  });
  const size = chunks.reduce((total, chunk) => total + chunk.byteLength, 0);
  const output = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    output.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return { output, chunks, callbackChunks, calls };
}

async function digest(bytes) {
  const hash = await crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, '0')).join('');
}

self.onmessage = async event => {
  try {
    await init();
    equal(input_formats(), inputNames, 'input capabilities changed');
    equal(output_formats(), outputNames, 'output capabilities changed');

    const inputs = Object.fromEntries(
      Object.entries(event.data.inputs).map(([name, bytes]) => [name, new Uint8Array(bytes)]),
    );
    const matrix = {};
    for (const from of inputNames) {
      for (const to of outputNames) {
        const key = `${from}->${to}`;
        const result = convert(from, to, inputs[from]);
        matrix[key] = { bytes: result.output.byteLength, sha256: await digest(result.output) };
        equal(matrix[key], event.data.expected[key], `${key} exact output changed`);
      }
    }
    assert(Object.keys(matrix).length === 15, 'full conversion matrix did not contain 3 x 5 pairings');

    const first = convert('markdown', 'html', inputs.markdown);
    const ownedChunk = first.callbackChunks[0];
    const retained = new Uint8Array(ownedChunk);
    convert('markdown', 'html', inputs.markdown);
    assert(ownedChunk.byteLength > 0, 'retained output chunk was detached by a later conversion');
    equal(Array.from(ownedChunk), Array.from(retained), 'output chunk ownership changed');

    let error;
    try {
      convert_stream('markdown', 'html', sourceFor(inputs.markdown).source, () => {
        throw new Error('worker sink failure');
      });
    } catch (caught) {
      error = caught;
    }
    assert(error instanceof Error && error.code === 'IO_ERROR', 'worker callback error code changed');
    assert(convert('markdown', 'html', inputs.markdown).output.byteLength > 0, 'conversion did not recover');

    self.postMessage({ ok: true, matrix });
  } catch (error) {
    self.postMessage({ ok: false, message: error?.stack || String(error) });
  }
};
