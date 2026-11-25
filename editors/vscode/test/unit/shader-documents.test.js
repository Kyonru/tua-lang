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


test("workspace Lua dependency overlays synchronize without duplicate language-client ownership", () => {
  const { synchronizeLuaDocuments } = require("../../src/lua-documents");
  const listeners = {};
  const sent = [];
  let disposed = 0;
  const event = name => listener => {
    listeners[name] = listener;
    return { dispose() { disposed++; } };
  };
  const document = {
    uri: { scheme: "file", path: "/project/en.lua", toString: () => "file:///project/en.lua" },
    languageId: "lua", version: 1, getText: () => "return { en = {} }",
  };
  const vscode = {
    workspace: {
      textDocuments: [document],
      getWorkspaceFolder: uri => uri.path.startsWith("/project/") ? {} : undefined,
      onDidOpenTextDocument: event("open"), onDidChangeTextDocument: event("change"), onDidCloseTextDocument: event("close"),
    },
    languages: { match: (_, doc) => doc.languageId === "tua" ? 1 : 0 },
  };
  const client = { sendNotification(method, params) { sent.push({ method, params }); return Promise.resolve(); } };
  const sync = synchronizeLuaDocuments(vscode, client, () => {});
  assert.equal(sent.length, 1);
  listeners.open(document);
  listeners.open({ ...document, languageId: "tua" });
  listeners.open({ ...document, uri: { ...document.uri, path: "/outside/en.lua" } });
  assert.equal(sent.length, 1);
  document.version = 2;
  listeners.change({ document });
  assert.equal(sent[1].params.textDocument.version, 2);
  listeners.close(document);
  assert.equal(sent[2].method, "textDocument/didClose");
  sync.dispose();
  assert.equal(disposed, 3);
  const restarted = synchronizeLuaDocuments(vscode, client, () => {});
  assert.equal(sent[3].method, "textDocument/didOpen");
  assert.equal(sent[3].params.textDocument.version, 2);
  restarted.dispose();
});
