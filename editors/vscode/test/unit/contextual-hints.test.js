"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const { contextualHints } = require("../../src/editor/contextual-hints");
const hints = [1, 2, 3, 4].map(line => ({ position: { line, character: 8 }, label: `type${line}` }));

test("contextual hints stay quiet except selected or explicitly inspected lines", () => {
  assert.deepEqual(contextualHints(hints), []);
  assert.deepEqual(contextualHints(hints, { inspectedLine: 2 }), [hints[1]]);
  assert.deepEqual(contextualHints(hints, { selections: [{ isEmpty: false, start: { line: 1, character: 2 }, end: { line: 3, character: 0 } }] }), hints.slice(0, 2));
  assert.deepEqual(contextualHints(hints, { selections: [{ isEmpty: true }] }), []);
});

test("debugging reveals the full hint objects without altering labels or tooltips", () => {
  assert.equal(contextualHints(hints, { debugging: true }), hints);
  assert.equal(contextualHints(hints, { inspectedLine: 3 })[0], hints[2]);
});
