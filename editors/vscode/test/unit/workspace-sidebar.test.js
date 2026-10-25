"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const Module = require("node:module");
const vscode = {
  EventEmitter: class { constructor() { this.event = () => {}; } fire() {} dispose() {} },
  Uri: { parse: value => ({ fsPath: new URL(value).pathname }) },
};
const original = Module._load;
Module._load = function(id, ...args) { return id === "vscode" ? vscode : original.call(this, id, ...args); };
const { WorkspaceTree, graphNodes } = require("../../src/workspace/sidebar");
Module._load = original;

test("workspace graphs retain declaration locations and stable identities", () => {
  const range = { start: { line: 2, character: 0 }, end: { line: 2, character: 6 } };
  const result = graphNodes("modules", { graph: { modules: [{ uri: "file:///game/player.tua", exports: [{ name: "Player", range }] }] } });
  assert.equal(result[0].label, "player.tua");
  assert.equal(result[0].children[0].uri, "file:///game/player.tua");
  assert.deepEqual(result[0].children[0].range, range);
  assert.equal(graphNodes("signals", { events: [] })[0].label, "No declarations found");
});

test("workspace requests are lazy and invalidate on refresh or server change", async () => {
  let calls = 0;
  let client = { sendRequest: async () => { calls++; return { events: [] }; } };
  const tree = new WorkspaceTree(() => client);
  const node = (await tree.getChildren()).find(item => item.kind === "signals");
  assert.equal(calls, 0);
  await tree.getChildren(node);
  await tree.getChildren(node);
  assert.equal(calls, 1);
  tree.refresh();
  await tree.getChildren(node);
  assert.equal(calls, 2);
  client = { ...client };
  await tree.getChildren(node);
  assert.equal(calls, 3);
});
