#!/usr/bin/env node
'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const { chromium, firefox } = require('playwright');

const repositoryRoot = path.resolve(__dirname, '../..');
const packageRoot = path.resolve(process.argv[2] || path.join(repositoryRoot, 'target/wasm-pack'));
const webPackage = path.join(packageRoot, 'web-docx-to-markdown');
const examplePath = path.join(repositoryRoot, 'crates/docspec-wasm/examples/browser-worker.mjs');
const docx = Array.from(fs.readFileSync(path.join(repositoryRoot, 'tests/fixtures/docx/apache-tika/testWORD_1img.docx')));
const expected = JSON.parse(fs.readFileSync(path.join(__dirname, 'runtime-expected.json'), 'utf8')).matrix['docx->markdown'];

function serveFile(response, file, contentType) {
  response.writeHead(200, { 'Content-Type': contentType });
  fs.createReadStream(file).pipe(response);
}

const server = http.createServer((request, response) => {
  const pathname = new URL(request.url, 'http://127.0.0.1').pathname;
  if (pathname === '/') {
    response.writeHead(200, { 'Content-Type': 'text/html' });
    response.end('<!doctype html><title>DocSpec OPFS example test</title>');
  } else if (pathname === '/examples/browser-worker.mjs') {
    serveFile(response, examplePath, 'text/javascript');
  } else if (pathname.startsWith('/pkg/web-docx-to-markdown/')) {
    const file = path.join(webPackage, path.basename(pathname));
    serveFile(response, file, pathname.endsWith('.wasm') ? 'application/wasm' : 'text/javascript');
  } else {
    response.writeHead(404);
    response.end();
  }
});
async function runBrowser(browserType, name) {
  const browser = await browserType.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    const result = await page.evaluate(async inputBytes => {
      async function entries() {
        const root = await navigator.storage.getDirectory();
        const names = [];
        for await (const name of root.keys()) names.push(name);
        return names.sort();
      }
      function request(file) {
        return new Promise((resolve, reject) => {
          const worker = new Worker('/examples/browser-worker.mjs', { type: 'module' });
          worker.onmessage = event => { worker.terminate(); resolve(event.data); };
          worker.onerror = event => { worker.terminate(); reject(new Error(event.message)); };
          worker.postMessage({ file, from: 'docx', to: 'markdown' });
        });
      }

      const before = await entries();
      const failure = await request(new Blob([new TextEncoder().encode('invalid DOCX')]));
      const afterFailure = await entries();
      const success = await request(new Blob([new Uint8Array(inputBytes)]));
      const root = await navigator.storage.getDirectory();
      const handle = await root.getFileHandle(success.stageName);
      const output = new Uint8Array(await (await handle.getFile()).arrayBuffer());
      await root.removeEntry(success.stageName);
      return { before, afterFailure, failure, success, output: Array.from(output), afterSuccess: await entries() };
    }, docx);

    assert.equal(result.failure.ok, false);
    assert.equal(result.failure.code, 'CONVERSION_ERROR');
    assert.deepEqual(result.afterFailure, result.before, `${name} retained failed staging output`);
    assert.equal(result.success.ok, true);
    assert.deepEqual(result.success.inputFormats, ['docx']);
    assert.deepEqual(result.success.outputFormats, ['markdown']);
    assert.deepEqual(result.afterSuccess, result.before, `${name} published output cleanup failed`);
    const output = Buffer.from(result.output);
    assert.equal(output.byteLength, expected.bytes);
    assert.equal(crypto.createHash('sha256').update(output).digest('hex'), expected.sha256);
    fs.writeFileSync(
      path.join(packageRoot, `${name}-opfs-example-results.json`),
      `${JSON.stringify({ failure: result.failure, success: result.success, outputBytes: output.byteLength }, null, 2)}\n`,
    );
  } finally {
    await browser.close();
  }
}

server.listen(0, '127.0.0.1', async () => {
  try {
    await runBrowser(chromium, 'chromium');
    await runBrowser(firefox, 'firefox');
    console.log('Chromium and Firefox passed the OPFS staging example');
  } catch (error) {
    console.error(error?.stack || String(error));
    process.exitCode = 1;
  } finally {
    server.close();
  }
});
