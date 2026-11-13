"use strict";

const assert = require("node:assert/strict");
const Module = require("node:module");
const test = require("node:test");

const document = { uri: { toString: () => "file:///game/flow.tua", fsPath: "/game/flow.tua" }, version: 1 };
let show;
let panel;
let onDocumentChange;
let onRuntimeUpdate;
const messages = [];
const vscode = {
  ViewColumn: { Beside: 2 },
  Disposable: { from: (...items) => ({ dispose() { for (const item of items) item?.dispose?.(); } }) },
  commands: { registerCommand(name, handler) { if (name === "tua.previewStateMachine") show = handler; return { dispose() {} }; } },
  workspace: {
    getConfiguration: () => ({ get: () => true }),
    onDidChangeTextDocument(handler) { onDocumentChange = handler; return { dispose() {} }; },
    asRelativePath: () => "flow.tua",
  },
  window: {
    activeTextEditor: { document },
    createWebviewPanel() {
      panel = {
        visible: true,
        webview: { cspSource: "test", onDidReceiveMessage() {}, postMessage(message) { messages.push(message); } },
        onDidDispose() {},
        onDidChangeViewState(handler) { panel.onViewState = handler; },
        reveal() {},
        dispose() {},
      };
      return panel;
    },
  },
};
const originalLoad = Module._load;
Module._load = function (name, ...args) {
  return name === "vscode" ? vscode : originalLoad.call(this, name, ...args);
};
const { registerStateMachinePreviewFeature } = require("../../src/state-machines/preview");
Module._load = originalLoad;

test("hidden state-machine preview defers inspection until reveal", async () => {
  let requests = 0;
  const client = { sendRequest: async () => {
    requests++;
    return { documentVersion: document.version, fallbackApplied: true, items: [] };
  } };
  const feature = registerStateMachinePreviewFeature({
    configurationTarget: () => undefined,
    getClient: () => client,
    inspector: {
      entries: new Map(),
      subscribe(handler) { onRuntimeUpdate = handler; return { dispose() {} }; },
    },
    isTuaDocument: () => true,
  });
  try {
    await show({ runtime: { sessionId: "game", objectId: 1 } });
    assert.equal(requests, 1);
    panel.visible = false;
    onRuntimeUpdate();
    assert.equal(messages.length, 0, "hidden panels should not receive runtime messages");
    document.version++;
    onDocumentChange({ document });
    await new Promise(resolve => setTimeout(resolve, 220));
    assert.equal(requests, 1);
    panel.visible = true;
    await panel.onViewState();
    assert.equal(requests, 2);
    onRuntimeUpdate();
    assert.equal(messages.length, 1, "visible panels continue receiving runtime state");
  } finally {
    feature.dispose();
    document.version = 1;
  }
});
