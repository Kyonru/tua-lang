"use strict";
const { test } = require("node:test");
const assert = require("node:assert/strict");
const { synchronizeShaderDocuments } = require("../../src/shader-documents");

test("shader overlays synchronize without registering language providers and dispose on restart", () => {
  const listeners = {};
  const sent = [];
  let disposed = 0;
  const document = { uri: { scheme: "file", path: "/effect.glsl", toString: () => "file:///effect.glsl" }, languageId: "glsl", version: 1, getText: () => "shader source" };
  const event = (name) => (listener) => { listeners[name] = listener; return { dispose() { disposed++; } }; };
  const vscode = { workspace: { textDocuments: [document],
    onDidOpenTextDocument: event("open"), onDidChangeTextDocument: event("change"), onDidCloseTextDocument: event("close"),
  } };
  const sync = synchronizeShaderDocuments(vscode, { sendNotification(method, params) { sent.push({ method, params }); return Promise.resolve(); } }, () => {});
  assert.equal(sent[0].method, "textDocument/didOpen");
  listeners.open(document);
  assert.equal(sent.length, 1);
  document.version = 2;
  listeners.change({ document });
  assert.equal(sent[1].params.textDocument.version, 2);
  assert.equal(sent[1].params.contentChanges[0].text, "shader source");
  listeners.close(document);
  assert.equal(sent[2].method, "textDocument/didClose");
  listeners.open({ ...document, uri: { ...document.uri, path: "/main.tua" } });
  assert.equal(sent.length, 3);
  sync.dispose();
  assert.equal(disposed, 3);
});
