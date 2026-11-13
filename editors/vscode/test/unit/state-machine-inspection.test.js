"use strict";

const assert = require("node:assert/strict");
const Module = require("node:module");
const test = require("node:test");

const originalLoad = Module._load;
Module._load = function (name, ...args) {
  return name === "vscode" ? {} : originalLoad.call(this, name, ...args);
};
const { inspectStateMachines } = require("../../src/state-machines/preview");
Module._load = originalLoad;

const document = { uri: { toString: () => "file:///game/flow.tua" }, version: 1 };
const position = { line: 2, character: 4 };
const machine = { kind: "stateMachine", name: "flow" };

test("state-machine preview uses the server's one-request fallback", async () => {
  const requests = [];
  const client = { sendRequest: async (method, params) => {
    requests.push({ method, params });
    return { documentVersion: 1, fallbackApplied: true, items: [machine] };
  } };
  const result = await inspectStateMachines({ client, document, position });
  assert.deepEqual(result.items, [machine]);
  assert.equal(requests.length, 1);
  assert.equal(requests[0].method, "tua/inspect");
  assert.deepEqual(requests[0].params, {
    textDocument: { uri: document.uri.toString() },
    kind: "stateMachine",
    position,
    fallbackToAll: true,
  });
});

test("state-machine preview retains fallback with an older server", async () => {
  const requests = [];
  const client = { sendRequest: async (_, params) => {
    requests.push(params);
    return { documentVersion: 1, items: requests.length === 1 ? [] : [machine] };
  } };
  const result = await inspectStateMachines({ client, document, position });
  assert.deepEqual(result.items, [machine]);
  assert.equal(requests.length, 2);
  assert.equal(requests[1].position, undefined);
});
