"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const Module = require("node:module");
const originalLoad = Module._load;
Module._load = function(id, ...args) {
  return id === "vscode" ? { Uri: { parse: value => ({ fsPath: new URL(value).pathname }) }, workspace: {} } : originalLoad.call(this, id, ...args);
};
const { ecsExplorerViewModel } = require("../../src/ecs/view-model");
Module._load = originalLoad;

test("ECS readers retain their component links and unknown mutation access", () => {
  const range = { start: { line: 2, character: 0 }, end: { line: 2, character: 30 } };
  const view = ecsExplorerViewModel({ graph: { queries: [{
    id: "read", uri: "file:///src/main.tua", range, name: "readPosition", kind: "reader",
    components: [{ reference: "Position", componentId: "position", access: "unknown", range }],
  }] } });
  assert.equal(view.queries[0].kind, "reader");
  assert.equal(view.queries[0].components[0].componentId, "position");
  assert.equal(view.queries[0].components[0].access, "unknown");
});
