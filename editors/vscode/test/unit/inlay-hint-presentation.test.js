"use strict";

const assert = require("node:assert/strict");
const Module = require("node:module");
const test = require("node:test");

const disposable = { dispose() {} };
const document = { uri: { toString: () => "file:///game/main.tua" } };
const editor = { document, selections: [{ isEmpty: true }] };
const vscode = {
  ConfigurationTarget: { Global: 1 },
  window: {
    activeTextEditor: editor,
    onDidChangeTextEditorSelection: () => disposable,
    onDidChangeActiveTextEditor: () => disposable,
  },
  workspace: { getWorkspaceFolder: () => undefined },
  debug: { activeDebugSession: undefined, onDidChangeActiveDebugSession: () => disposable },
  commands: { registerCommand: () => disposable },
};
const originalLoad = Module._load;
Module._load = function (name, ...args) {
  return name === "vscode" ? vscode : originalLoad.call(this, name, ...args);
};
const { registerInlayHintPresentation } = require("../../src/editor/inlay-hints");
Module._load = originalLoad;

test("contextual mode skips server hints when nothing can be shown", async () => {
  const state = { get: () => "contextual" };
  const presentation = registerInlayHintPresentation({
    context: { globalState: state, workspaceState: state },
    configurationTarget: () => 2,
    isTuaDocument: () => true,
    getClient: () => undefined,
  });
  let requests = 0;
  const next = async () => {
    requests += 1;
    return [{ position: { line: 1 } }, { position: { line: 2 } }];
  };

  try {
    assert.deepEqual(await presentation.middleware.provideInlayHints(document, {}, {}, next), []);
    assert.equal(requests, 0);

    editor.selections = [{ isEmpty: false, start: { line: 1, character: 0 }, end: { line: 1, character: 4 } }];
    assert.deepEqual(await presentation.middleware.provideInlayHints(document, {}, {}, next), [{ position: { line: 1 } }]);
    assert.equal(requests, 1);

    editor.selections = [{ isEmpty: true }];
    presentation.inspect(document, { line: 2 });
    assert.deepEqual(await presentation.middleware.provideInlayHints(document, {}, {}, next), [{ position: { line: 2 } }]);
    assert.equal(requests, 2);

    vscode.debug.activeDebugSession = { type: "lua-local" };
    assert.equal((await presentation.middleware.provideInlayHints(document, {}, {}, next)).length, 2);
    assert.equal(requests, 3);
  } finally {
    presentation.dispose();
    editor.selections = [{ isEmpty: true }];
    vscode.debug.activeDebugSession = undefined;
  }
});
