"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const Module = require("node:module");
const commands = new Map();
const views = new Map();
let picks = 0;
const noop = { dispose() {} };
const vscode = {
  Disposable: { from: (...items) => ({ dispose: () => items.forEach(item => item.dispose()) }) },
  EventEmitter: class { constructor() { this.event = () => noop; } fire() {} dispose() {} },
  ThemeColor: class {}, ThemeIcon: class {}, TreeItem: class { constructor(label) { this.label = label; } },
  StatusBarAlignment: { Left: 1 },
  commands: { registerCommand: (id, callback) => { commands.set(id, callback); return noop; }, executeCommand: async () => {} },
  languages: { getDiagnostics: () => [] },
  window: {
    createTreeView: (id, options) => { views.set(id, options.treeDataProvider); return { dispose() {} }; },
    createStatusBarItem: () => ({ show() {}, hide() {}, dispose() {} }),
    showQuickPick: async items => { picks++; return items.find(item => item.name === "Debug"); },
  },
  workspace: { getConfiguration: () => ({ get: (_key, fallback) => fallback }), onDidChangeConfiguration: () => noop },
};
const original = Module._load;
Module._load = function(id, ...args) { return id === "vscode" ? vscode : original.call(this, id, ...args); };
const { registerRunConfigurations } = require("../../src/workspace/run-configurations");
const { registerRunActivity } = require("../../src/workspace/activity");
Module._load = original;
const editorUri = { scheme: "file", fsPath: "/game/main.tua", toString: () => "file:///game/main.tua" };

test("toolbar configuration picker ignores implicit URI and focused-tree arguments", async () => {
  const state = new Map();
  const feature = registerRunConfigurations({ workspaceState: { get: key => state.get(key), update: async (key, value) => state.set(key, value) } });
  const before = picks;
  await commands.get("tua.run.select")(editorUri);
  assert.equal(picks, before + 1);
  assert.equal(state.get("tua.run.selectedConfiguration"), "Debug");
  await commands.get("tua.run.select")({ name: "Development", mode: "run" });
  assert.equal(picks, before + 2, "a view-title picker must not silently select its focused row");
  const provider = views.get("tua.run");
  const node = provider.getChildren().find(item => item.name === "Hot reload");
  const command = provider.getTreeItem(node).command;
  await commands.get(command.command)(...command.arguments);
  assert.equal(state.get("tua.run.selectedConfiguration"), "Hot reload");
  assert.equal(picks, before + 2, "tree selection remains direct");
  feature.dispose();
});

test("Stop uses live sessions, ignores editor URIs, and does not stop unrelated sessions", () => {
  const feature = registerRunActivity();
  let firstStops = 0;
  let secondStops = 0;
  const first = feature.store.begin({ title: "First", cwd: "/game", kind: "run" });
  first.stop = () => firstStops++;
  const second = feature.store.begin({ title: "Second", cwd: "/game", kind: "run" });
  second.stop = () => secondStops++;
  commands.get("tua.activity.stop")(editorUri);
  assert.equal(secondStops, 1);
  assert.equal(firstStops, 0);
  commands.get("tua.activity.stop")({ id: first.id });
  assert.equal(firstStops, 1, "resolve the original callback by session ID");
  commands.get("tua.activity.stop")({ id: "expired-session" });
  assert.equal(secondStops, 1, "a stale explicit session must not stop a different run");
  feature.dispose();
});
