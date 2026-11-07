'use strict';
// Read-only project replay: edits are sent as unsaved LSP buffers.
const fs = require('fs');
const path = require('path');
const { LspClient, pathToFileURL } = require('../../../editors/vscode/scripts/lsp-profile-common');
const [server, project, relativeFile = 'src/core/state.tua', otherFile = 'src/main.tua'] = process.argv.slice(2);
if (!server || !project) throw new Error('Usage: node probe.js /abs/tua /abs/project [file] [other-open-file]');
const root = path.resolve(project);
const file = path.resolve(root, relativeFile);
const client = new LspClient(path.resolve(server), root, { requestTimeoutMs: 120000 });
const results = [];
function position(text, offset) {
  const lines = text.slice(0, offset).split('\n');
  return { line: lines.length - 1, character: lines.at(-1).length };
}
(async () => {
  await client.start();
  try {
    let start = Date.now();
    await client.request('initialize', {
      processId: process.pid, rootUri: pathToFileURL(root).href,
      capabilities: { general: { positionEncodings: ['utf-16'] }, textDocument: { completion: { completionItem: { snippetSupport: true } } } },
    });
    results.push({ phase: 'initialize', ms: Date.now() - start });
    client.requestTimeoutMs = 30000;
    client.notify('initialized', {});
    const other = path.resolve(root, otherFile);
    if (other !== file) client.notify('textDocument/didOpen', { textDocument: {
      uri: pathToFileURL(other).href, languageId: 'tua', version: 1, text: fs.readFileSync(other, 'utf8'),
    } });
    const original = fs.readFileSync(file, 'utf8');
    const returnOffset = original.lastIndexOf('\nreturn {');
    const offset = returnOffset < 0 ? original.length : returnOffset;
    const uri = pathToFileURL(file).href;
    for (const [index, expression] of ['love.gr', 'lo', 'love.g', 'love.gr', 'player:', 'player:get().', 'player:get().ai'].entries()) {
      const inserted = '\nlocal probe = ' + expression;
      const text = original.slice(0, offset) + inserted + '\n' + original.slice(offset);
      const version = index + 1;
      if (index === 0) client.notify('textDocument/didOpen', { textDocument: { uri, languageId: 'tua', version, text } });
      else client.notify('textDocument/didChange', { textDocument: { uri, version }, contentChanges: [{ text }] });
      start = Date.now();
      const response = await client.request('textDocument/completion', {
        textDocument: { uri }, position: position(text, offset + inserted.length), context: { triggerKind: 1 },
      });
      const items = Array.isArray(response) ? response : response?.items || [];
      results.push({ expression, ms: Date.now() - start, labels: items.map(item => item.label) });
    }
  } finally {
    client.child.kill('SIGKILL');
    console.log(JSON.stringify(results, null, 2));
  }
})().catch(error => { console.error(error.message); process.exitCode = 1; });
