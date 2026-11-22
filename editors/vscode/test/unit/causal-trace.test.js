"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { causalTraceNodes } = require("../../src/inspector/causal-trace");
const event = (sequence, attackId, parentId, rootId = 1, phase = "queued") => ({
  sequence, attackId, parentId, rootId, phase, reason: phase === "suppressed" ? "root_limit" : "split", sourceKind: "split", sourceId: "example",
});
const trace = events => ({ inspectionKind: "causal-trace/v1", epoch: 2, enabled: true, events });
function flatten(node) { return [node, ...(node.children || []).flatMap(flatten)]; }

test("requires an explicit convention and groups observed ancestry and suppression", () => {
  assert.equal(causalTraceNodes({ events: [] }, "s"), null);
  const state = trace([event(3, 2, 1, 1, "suppressed"), event(2, 2, 1), event(1, 1, 0)]);
  const before = JSON.stringify(state);
  const view = causalTraceNodes(state, "s");
  const root = view.children[0];
  assert.equal(root.label, "Root #1");
  assert.equal(root.description, "2 attacks · 1 suppressed");
  assert.equal(root.children[0].label, "Attack #1");
  const child = root.children[0].children.find(node => node.label === "Attack #2");
  assert.equal(child.children[1].label, "suppressed: root_limit");
  assert.equal(JSON.stringify(state), before);
});

test("cycles, missing parents, conflicting records and malformed samples remain bounded", () => {
  const state = trace([event(1, 1, 2), event(2, 2, 1), event(3, 3, 99), event(4, 4, 0), event(5, 4, 3), {attackId: NaN}]);
  state.overwritten = 7;
  const rows = flatten(causalTraceNodes(state, "s"));
  assert.ok(rows.some(row => row.label === "Partial trace"));
  assert.ok(rows.some(row => row.description?.includes("cyclic/deep")));
  assert.ok(rows.some(row => row.description?.includes("parent #99 outside sample")));
  assert.ok(rows.some(row => row.description?.includes("conflicting parents")));
  assert.ok(rows.length < 30);
});

test("separate roots, stable IDs, reset epochs and stat provenance never invent relationships", () => {
  const state = trace([event(1, 1, 0), event(2, 2, 1, 2)]);
  Object.assign(state, {stats: ["damage: base 2 -> 3"], statRootId: 2, weaponId: "example"});
  const first = causalTraceNodes(state, "s");
  assert.equal(first.children[1].children[0].description, "split · parent #1 outside sample");
  assert.equal(first.children[2].description, "Root #2 · example");
  assert.equal(first.id, causalTraceNodes(state, "s").id);
  assert.notEqual(first.id, causalTraceNodes({...state, epoch: 3}, "s").id);
});

test("disabled, empty and large samples expose limits", () => {
  assert.equal(causalTraceNodes({...trace([]), enabled: false}, "s").children[0].label, "Trace disabled in game");
  const view = causalTraceNodes(trace(Array.from({length: 1000}, (_, index) => event(index, index + 1, 0))), "s");
  assert.equal(view.description, "128 sampled events");
  assert.match(view.children[0].description, /872 omitted/);
});
