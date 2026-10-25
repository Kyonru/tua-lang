"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const Module = require("node:module");
const commands = new Map();
const views = new Map();
const executed = [];
let picks = 0;
const noop = { dispose() {} };
const vscode = {
  Disposable: { from: (...items) => ({ dispose: () => items.forEach(item => item.dispose()) }) },
  EventEmitter: class { constructor() { this.event = () => noop; } fire() {} dispose() {} },
  ThemeColor: class {}, ThemeIcon: class {}, TreeItem: class { constructor(label) { this.label = label; } },
  StatusBarAlignment: { Left: 1 },
  commands: { registerCommand: (id, callback) => { commands.set(id, callback); return noop; }, executeCommand: async (...args) => { executed.push(args); if (args[0] === "tua.activity.stop") return commands.get(args[0])?.(args[1]); } },
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


test("toolbar Stop targets a multiplayer group while player Stop stays independent", () => {
  const feature = registerRunActivity();
  let groupStops = 0;
  let playerStops = 0;
  const group = feature.store.begin({ title: "Players", cwd: "/game", kind: "run" });
  group.stop = () => groupStops++;
  const player = feature.store.begin({ title: "Client", cwd: "/game", kind: "run", groupId: group.id });
  player.stop = () => playerStops++;
  commands.get("tua.activity.stop")(editorUri);
  assert.equal(groupStops, 1);
  assert.equal(playerStops, 0);
  commands.get("tua.activity.stop")({ id: player.id });
  assert.equal(groupStops, 1);
  assert.equal(playerStops, 1);
  feature.dispose();
});


test("Play changes to Stop for the selected run, including groups, and back after exit", async () => {
  const feature = registerRunActivity();
  const state = new Map();
  const configurations = registerRunConfigurations({ workspaceState: { get: key => state.get(key), update: async (key, value) => state.set(key, value) } }, feature.store);
  const active = () => executed.filter(args => args[0] === "setContext" && args[1] === "tua.selectedRunActive").at(-1)[2];
  assert.equal(active(), false);
  let stops = 0;
  const group = feature.store.begin({ title: "Development", configurationName: "Development", cwd: "/game", kind: "run" });
  group.stop = () => stops++;
  assert.equal(active(), true);
  const player = feature.store.begin({ title: "Player", configurationName: "Development", groupId: group.id, cwd: "/game", kind: "run" });
  player.stop = () => assert.fail("primary Stop should target the group");
  await commands.get("tua.run.stopSelected")(editorUri);
  assert.equal(stops, 1);
  await commands.get("tua.run.selected")();
  assert.equal(stops, 2, "Run command also avoids launching duplicates");
  await commands.get("tua.run.selectConfiguration")({ name: "Debug" });
  assert.equal(active(), false, "an unrelated running configuration must not replace Play");
  await commands.get("tua.run.stopSelected")();
  assert.equal(stops, 2);
  await commands.get("tua.run.selectConfiguration")({ name: "Development" });
  assert.equal(active(), true);
  feature.store.finish(player, 0);
  feature.store.finish(group, 0);
  assert.equal(active(), false);
  configurations.dispose();
  feature.dispose();
});

test("primary editor and sidebar Play/Stop buttons have complementary visibility", () => {
  const manifest = require("../../package.json");
  for (const location of ["editor/title", "view/title"]) {
    const menu = manifest.contributes.menus[location];
    const play = menu.find(item => item.command === "tua.run.selected");
    const stop = menu.find(item => item.command === "tua.run.stopSelected");
    assert(play.when.includes("!tua.selectedRunActive"));
    assert(stop.when.includes("tua.selectedRunActive"));
    assert.equal(play.group, stop.group);
  }
});
