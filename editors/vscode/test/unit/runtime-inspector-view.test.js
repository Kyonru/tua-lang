"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const Module = require("node:module");
const original = Module._load;
Module._load = function(id, ...args) {
  if (id === "vscode") return {
    EventEmitter: class { constructor() { this.event = () => {}; } dispose() {} },
    TreeItem: class { constructor(label) { this.label = label; } },
    TreeItemCollapsibleState: { Collapsed: 1, None: 0 }, ThemeIcon: class {},
  };
  return original.call(this, id, ...args);
};
const { InspectorTree, valueNodes } = require("../../src/inspector/view");
Module._load = original;

test("runtime view switches player data and preserves object identities across snapshots", () => {
  const entry = (id, state) => ({ session: { id, title: id, active: true }, snapshot: {
    objects: [{ id: 1, kind: "states", label: "Movement", data: { current: state } }], truncated: false,
  } });
  const entries = new Map([["host", entry("host", "idle")], ["client", entry("client", "walking")]]);
  const tree = new InspectorTree({ entries });
  const state = () => tree.getChildren().find(node => node.label === "State machines").children[0];
  const originalId = state().id;
  assert.equal(state().description, "idle");
  entries.get("host").snapshot.objects[0].data.current = "running";
  assert.equal(state().id, originalId);
  tree.selected = "client";
  assert.equal(state().description, "walking");
  assert.notEqual(state().id, originalId);
  entries.get("client").session.active = false;
  assert.equal(state().description, "walking", "retain the selected player's final snapshot");
  assert.equal(tree.getChildren().find(node => node.label === "Entities and components").children[0].label, "No live objects registered");
});

test("runtime values are displayed as data, with bounded depth and stable escaped keys", () => {
  const data = { "a/b": { "<script>": "command:untrusted", value: false } };
  const nodes = valueNodes(data, "root");
  assert.equal(nodes[0].id, "root/a%2Fb");
  assert.equal(nodes[0].children[0].label, "<script>");
  assert.equal(nodes[0].children[0].command, undefined);
  assert.deepEqual(valueNodes(data, "root", 8), []);
});

test("store state marks nested changes, additions and removals without flagging unchanged values", () => {
  const { storeNodes } = require("../../src/inspector/view");
  const old = { state: { score: 1, player: { health: 10 }, removed: true, stable: false }, updates: 1 };
  const now = { state: { score: 2, player: { health: 8, ammo: 3 }, stable: false }, updates: 2 };
  const state = storeNodes(now, "store", old).find(node => node.label === "state");
  const nodes = Object.fromEntries(state.children.map(node => [node.label, node]));
  assert.equal(nodes.score.change, "changed");
  assert.match(nodes.score.tooltip, /Previous: 1/);
  assert.equal(nodes.removed.change, "removed");
  assert.equal(nodes.stable.change, undefined);
  assert.equal(nodes.player.change, "changed");
  assert.equal(nodes.player.children.find(node => node.label === "ammo").change, "added");
  assert.equal(storeNodes(now, "store").find(node => node.label === "state").children[0].change, undefined);
});

test("inspector pause/resume icons follow the selected player's state and connection", () => {
  const { inspectorToolbarState } = require("../../src/inspector/view");
  const entry = { session: { active: true }, connection: {}, status: "connected", paused: false };
  assert.deepEqual(inspectorToolbarState(entry), { canControl: true, paused: false });
  entry.paused = true;
  assert.deepEqual(inspectorToolbarState(entry), { canControl: true, paused: true });
  assert.deepEqual(inspectorToolbarState({ ...entry, paused: false }), { canControl: true, paused: false });
  entry.status = "disconnected";
  assert.equal(inspectorToolbarState(entry).canControl, false);
  assert.equal(inspectorToolbarState(undefined).canControl, false);
  const manifest = require("../../package.json");
  const menus = manifest.contributes.menus["view/title"];
  const pause = menus.find(item => item.command === "tua.inspector.pause");
  const resume = menus.find(item => item.command === "tua.inspector.resume");
  assert(pause.when.includes("!tua.inspectorPaused"));
  assert(resume.when.includes("tua.inspectorPaused"));
  assert.equal(pause.group, resume.group);
  assert.notEqual(manifest.contributes.commands.find(item => item.command === pause.command).icon,
    manifest.contributes.commands.find(item => item.command === resume.command).icon);
});

test("named runtime objects expose creation-site actions and retain separate runtime identities", () => {
  const origin = { path: "/game/src/state.tua", file: "src/state.tua", line: 2, character: 6, name: "player", generated: false };
  const entry = { session: { id: "host", active: true }, snapshot: { objects: [1, 2].map(id => ({ id, kind: "stores", label: `stores ${id}`, data: { state: {}, updates: 0 }, origin })) } };
  const tree = new InspectorTree({ entries: new Map([["host", entry]]) });
  const stores = tree.getChildren().find(node => node.label === "Stores").children;
  assert.equal(tree.getParent(stores[0]).label, "Stores");
  assert.equal(tree.getParent(tree.getParent(stores[0])), undefined);
  assert.equal(stores[0].label, "player"); assert.notEqual(stores[0].id, stores[1].id);
  assert.match(stores[0].description, /#1/);
  const item = tree.getTreeItem(stores[0]);
  assert.equal(item.contextValue, "tuaRuntimeObject.source");
  const link = tree.getTreeItem(stores[0].children[0]);
  assert.equal(link.command.command, "tua.inspector.goToSource");
  assert.deepEqual(link.command.arguments, [{ origin }]);
});


test("inspector can be re-enabled from its toolbar while a retained snapshot hides welcome content", () => {
  const entry = { session: { id: "host", active: false }, status: "disabled", snapshot: { objects: [] } };
  const tree = new InspectorTree({ entries: new Map([["host", entry]]) });
  assert(tree.getChildren().length > 0, "retained snapshot keeps the welcome screen hidden");
  const menus = require("../../package.json").contributes.menus["view/title"];
  const enable = menus.find(item => item.command === "tua.inspector.enable");
  const disable = menus.find(item => item.command === "tua.inspector.disable");
  assert.equal(enable.when, "view == tua.inspector && !config.tua.runtimeInspector.enabled");
  assert.equal(disable.when, "view == tua.inspector && config.tua.runtimeInspector.enabled");
  assert.equal(enable.group, disable.group);
  assert(enable.group.startsWith("navigation"), "enable is visible directly in the toolbar");
});
