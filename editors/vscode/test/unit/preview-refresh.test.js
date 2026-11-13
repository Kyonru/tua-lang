"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const Module = require("node:module");

const commands = new Map();
let panel;
let onDocumentChange;
const vscode = {
  ViewColumn: { Beside: 2 },
  Disposable: { from: (...items) => ({ dispose: () => items.forEach(item => item?.dispose?.()) }) },
  commands: { registerCommand: (name, handler) => {
    commands.set(name, handler);
    return { dispose() { commands.delete(name); } };
  } },
  window: {
    activeTextEditor: undefined,
    createWebviewPanel: () => {
      panel = {
        visible: true,
        webview: { cspSource: "vscode-resource:", postMessage() {}, onDidReceiveMessage(handler) { panel.onMessage = handler; } },
        onDidDispose(handler) { panel.onDispose = handler; },
        onDidChangeViewState(handler) { panel.onViewState = handler; },
        reveal() {},
        dispose() { panel.onDispose?.(); },
      };
      return panel;
    },
  },
  workspace: {
    findFiles: async () => [],
    asRelativePath: uri => uri.fsPath,
    onDidChangeTextDocument: handler => { onDocumentChange = handler; return { dispose() {} }; },
    createFileSystemWatcher: () => ({ onDidChange: () => ({ dispose() {} }), onDidCreate: () => ({ dispose() {} }), onDidDelete: () => ({ dispose() {} }), dispose() {} }),
  },
  Uri: { parse: (uri) => ({ scheme: "file", fsPath: new URL(uri).pathname }) },
};
const load = Module._load;
Module._load = function (name, ...args) {
  return name === "vscode" ? vscode : load.call(this, name, ...args);
};
const { registerModuleGraphFeature } = require("../../src/modules/module-graph");
const { registerEcsExplorerFeature } = require("../../src/ecs/explorer");
const { registerObjectPoolExplorerFeature } = require("../../src/object-pools/explorer");
const { registerSignalGraphFeature } = require("../../src/signals/signal-graph");
const { registerServerHealthFeature } = require("../../src/health");
const { registerAudioDevex } = require("../../src/audio");
Module._load = load;

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

const cases = [
  ["tua.showModuleGraph", registerModuleGraphFeature, (n) => ({ analysisRevision: n, graph: { modules: [], edges: [], cycles: [] } })],
  ["tua.showEcsExplorer", registerEcsExplorerFeature, (n) => ({ analysisRevision: n, graph: {} })],
  ["tua.showObjectPoolExplorer", registerObjectPoolExplorerFeature, (n) => ({ analysisRevision: n, graph: {} })],
  ["tua.showSignalGraph", registerSignalGraphFeature, (n) => ({ events: [{ name: `event${n}`, calls: [] }] })],
  ["tua.showServerHealth", registerServerHealthFeature, (n) => ({ revision: n })],
  ["tua.showAudioExplorer", registerAudioDevex, (n) => ({ graph: { systems: [{ uri: `file:///game/audio${n}.tua`, buses: [], cues: [], musicStates: [], layers: [], effects: [] }] } })],
];

for (const [name, register, response] of cases) {
  test(`${name} keeps the newest result when server requests finish out of order`, async () => {
    const first = deferred();
    const second = deferred();
    let count = 0;
    const client = { sendRequest: () => (++count === 1 ? first.promise : second.promise) };
    const feature = register({ getClient: () => client });
    try {
      const initial = commands.get(name)();
      const reload = panel.onMessage({ command: "reload" });
      second.resolve(response(2));
      await reload;
      const current = panel.webview.html;
      assert(current.includes("2"), "the latest response rendered");
      first.resolve(response(1));
      await initial;
      assert.equal(panel.webview.html, current, "an older response must not replace the preview");
    } finally {
      feature.dispose();
    }
  });
}

test("ECS explorer command carries an inspected component name into its view", async () => {
  const client = { sendRequest: async () => ({ analysisRevision: 1, graph: { components: [], systems: [], queries: [], insights: [] } }) };
  const feature = registerEcsExplorerFeature({ getClient: () => client });
  try {
    await commands.get("tua.showEcsExplorer")({ componentName: "Position" });
    assert.match(panel.webview.html, /"focusComponentName":"Position"/);
  } finally {
    feature.dispose();
  }
});

test("hidden audio workbench defers graph and asset analysis until reveal", async () => {
  let requests = 0;
  const client = { sendRequest: async () => {
    requests++;
    return { analysisRevision: requests, graph: { systems: [
      { uri: "file:///game/audio.tua", buses: [], cues: [], musicStates: [], layers: [], effects: [] },
    ] } };
  } };
  const feature = registerAudioDevex({ getClient: () => client });
  try {
    await commands.get("tua.showAudioExplorer")();
    assert.equal(requests, 1);
    panel.visible = false;
    onDocumentChange({ document: { uri: { toString: () => "file:///game/audio.tua" } } });
    await new Promise(resolve => setTimeout(resolve, 280));
    assert.equal(requests, 1);
    panel.visible = true;
    await panel.onViewState();
    assert.equal(requests, 2);
  } finally {
    feature.dispose();
  }
});
