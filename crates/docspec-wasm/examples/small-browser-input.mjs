import init, { convert_stream } from "../pkg/docspec_wasm.js";

const MAX_TRANSFER_BYTES = 64 * 1024;

function concatenate(chunks) {
    const length = chunks.reduce((total, chunk) => total + chunk.byteLength, 0);
    const output = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) {
        output.set(chunk, offset);
        offset += chunk.byteLength;
    }
    return output;
}

export async function convertSmallMarkdown(markdownBytes) {
    await init();
    const chunks = [];
    const source = {
        size: markdownBytes.byteLength,
        readAt(offset, length) {
            return markdownBytes.slice(offset, offset + Math.min(length, MAX_TRANSFER_BYTES));
        },
    };

    convert_stream("markdown", "blocknote", source, (chunk) => {
        chunks.push(chunk.slice());
    });
    return concatenate(chunks);
}
