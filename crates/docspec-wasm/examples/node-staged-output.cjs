const fs = require("node:fs");
const path = require("node:path");
const { randomUUID } = require("node:crypto");
const { convert_stream, input_formats, output_formats } = require("../pkg/node-docx-markdown");

const MAX_TRANSFER_BYTES = 64 * 1024;

class PositionalFile {
    constructor(filename) {
        this.descriptor = fs.openSync(filename, "r");
        this.size = fs.fstatSync(this.descriptor).size;
    }

    readAt(offset, length) {
        const buffer = Buffer.allocUnsafe(Math.min(length, MAX_TRANSFER_BYTES));
        const count = fs.readSync(this.descriptor, buffer, 0, buffer.length, offset);
        return new Uint8Array(buffer.buffer, buffer.byteOffset, count).slice();
    }

    close() {
        fs.closeSync(this.descriptor);
    }
}

function main(inputPath, outputPath) {
    const stagedPath = path.join(
        path.dirname(outputPath),
        `.${path.basename(outputPath)}.${randomUUID()}.docspec-stage`,
    );
    const source = new PositionalFile(inputPath);
    let destination;
    let sourceClosed = false;
    let published = false;

    try {
        destination = fs.openSync(stagedPath, "wx");
        convert_stream("docx", "markdown", source, (chunk) => {
            let offset = 0;
            while (offset < chunk.byteLength) {
                const written = fs.writeSync(destination, chunk, offset, chunk.byteLength - offset);
                if (written === 0) {
                    throw new Error("file write made no progress");
                }
                offset += written;
            }
        });
        fs.fsyncSync(destination);
        fs.closeSync(destination);
        destination = undefined;
        sourceClosed = true;
        source.close();
        fs.renameSync(stagedPath, outputPath);
        published = true;
    } finally {
        try {
            if (destination !== undefined) {
                fs.closeSync(destination);
            }
        } finally {
            try {
                if (!sourceClosed) {
                    source.close();
                }
            } finally {
                if (!published && fs.existsSync(stagedPath)) {
                    fs.unlinkSync(stagedPath);
                }
            }
        }
    }

    console.log(`wrote ${outputPath}`);
}

if (process.argv.length !== 4) {
    throw new Error(
        `usage: ${path.basename(process.argv[1])} INPUT.docx OUTPUT.md\n` +
            `enabled inputs: ${input_formats().join(", ")}\n` +
            `enabled outputs: ${output_formats().join(", ")}`,
    );
}

main(process.argv[2], process.argv[3]);
