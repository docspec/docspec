import init, {
    convert_stream,
    input_formats,
    output_formats,
} from "../pkg/web-docx-to-markdown/docspec_wasm.js";

const MAX_TRANSFER_BYTES = 64 * 1024;

self.addEventListener("message", async ({ data }) => {
    const { file, from, to } = data;
    let directory;
    let stageName;
    let destination;
    let converted = false;
    let failure;

    try {
        await init();
        const source = {
            size: file.size,
            readAt(offset, length) {
                const reader = new FileReaderSync();
                const bytes = reader.readAsArrayBuffer(
                    file.slice(offset, offset + Math.min(length, MAX_TRANSFER_BYTES)),
                );
                return new Uint8Array(bytes);
            },
        };

        directory = await navigator.storage.getDirectory();
        stageName = `docspec-${crypto.randomUUID()}.stage`;
        const stageHandle = await directory.getFileHandle(stageName, { create: true });
        destination = await stageHandle.createSyncAccessHandle();
        convert_stream(from, to, source, (chunk) => {
            if (destination.write(chunk) !== chunk.byteLength) {
                throw new Error("OPFS write was short");
            }
        });
        destination.flush();
        destination.close();
        destination = undefined;
        converted = true;
    } catch (error) {
        failure = error;
    }

    if (!converted) {
        if (destination !== undefined) {
            try {
                destination.close();
            } catch (closeError) {
                failure ??= closeError;
            }
        }
        if (directory !== undefined && stageName !== undefined) {
            await directory.removeEntry(stageName).catch(() => undefined);
        }
        self.postMessage({
            ok: false,
            code: failure?.code ?? "IO_ERROR",
            message: failure instanceof Error ? failure.message : String(failure),
        });
        return;
    }

    self.postMessage({
        ok: true,
        stageName,
        inputFormats: input_formats(),
        outputFormats: output_formats(),
    });
});
