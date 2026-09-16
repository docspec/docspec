#!/usr/bin/env node
'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const { chromium, firefox } = require('playwright');

const repositoryRoot = path.resolve(__dirname, '../..');
const packageRoot = path.resolve(process.argv[2] || path.join(repositoryRoot, 'target/wasm-pack'));
const webPackage = path.join(packageRoot, 'web-full');
const workerPath = path.join(__dirname, 'runtime-worker.js');
const expected = JSON.parse(fs.readFileSync(path.join(__dirname, 'runtime-expected.json'), 'utf8')).matrix;
const inputs = {
  docx: Array.from(fs.readFileSync(path.join(repositoryRoot, 'tests/fixtures/docx/apache-tika/testWORD_1img.docx'))),
  html: Array.from(Buffer.from('<!doctype html><html><body><h1>Hello</h1><p>World</p></body></html>')),
  markdown: Array.from(Buffer.from('# Hello\n\nWorld\n')),
};

function serveFile(response, file, contentType) {
  response.writeHead(200, { 'Content-Type': contentType, 'Cross-Origin-Opener-Policy': 'same-origin' });
  fs.createReadStream(file).pipe(response);
}

const server = http.createServer((request, response) => {
  const pathname = new URL(request.url, 'http://127.0.0.1').pathname;
  if (pathname === '/') {
    response.writeHead(200, { 'Content-Type': 'text/html' });
    response.end('<!doctype html><title>DocSpec WASM runtime test</title>');
  } else if (pathname === '/runtime-worker.js') {
    serveFile(response, workerPath, 'text/javascript');
  } else if (pathname.startsWith('/pkg/')) {
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
    const result = await page.evaluate(({ inputs: testInputs, expected: expectedMatrix }) => new Promise((resolve, reject) => {
      const worker = new Worker('/runtime-worker.js', { type: 'module' });
      const timeout = setTimeout(() => reject(new Error('worker timed out')), 120_000);
      worker.onmessage = event => {
        clearTimeout(timeout);
        worker.terminate();
        resolve(event.data);
      };
      worker.onerror = event => {
        clearTimeout(timeout);
        worker.terminate();
        reject(new Error(event.message));
      };
      worker.postMessage({ inputs: testInputs, expected: expectedMatrix });
    }), { inputs, expected });
    assert.equal(result.ok, true, result.message);
    fs.writeFileSync(
      path.join(packageRoot, `${name}-worker-results.json`),
      `${JSON.stringify(result, null, 2)}\n`,
    );
  } finally {
    await browser.close();
  }
}

server.listen(0, '127.0.0.1', async () => {
  try {
    await runBrowser(chromium, 'chromium');
    await runBrowser(firefox, 'firefox');
    console.log('Chromium and Firefox module Workers passed the full conversion matrix');
  } catch (error) {
    console.error(error?.stack || String(error));
    process.exitCode = 1;
  } finally {
    server.close();
  }
});
