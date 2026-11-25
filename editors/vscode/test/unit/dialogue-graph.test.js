"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const { dialogueGraphLayout, dialogueGraph } = require("../../src/dialogue/graph");

const range = { start: { line: 1, character: 0 }, end: { line: 1, character: 4 } };
const sections = ["start", "branch", "done", "unused"].map(label => ({ label, range, lines: [] }));
const route = (section, target, kind = "choice") => ({ section, target, kind, range });

test("map preserves cycles, parallel routes, disconnected nodes and source order", () => {
  const routes = [route(0, "branch"), route(0, "branch", "fallback"), route(1, "start", "branchFalse"), route(1, "branch", "branchTrue")];
  const graph = dialogueGraphLayout({ sections, routes });
  assert.deepEqual(graph.nodes.map(node => node.label), ["start", "branch", "done", "unused"]);
  assert.equal(graph.edges.length, 4);
  assert.equal(new Set(graph.edges.map(edge => edge.x)).size, 4);
  assert.equal(graph.edges[3].path, `M 260 140 H ${graph.edges[3].x} V 170 H 260`);
  assert.equal(graph.edges[2].label, "branch → start: Condition false");
  assert.equal(graph.omitted, 0);
});

test("map bounds nodes/routes and does not invent unresolved destinations", () => {
  const many = Array.from({ length: 41 }, (_, i) => ({ label: `node${i}`, range, lines: [] }));
  const routes = [route(0, "node40"), route(40, "node0"), route(0, "missing"), ...Array.from({ length: 81 }, () => route(0, "node1"))];
  const graph = dialogueGraphLayout({ sections: many, routes });
  assert.equal(graph.nodes.length, 40);
  assert.equal(graph.edges.length, 80);
  assert.equal(graph.omitted, 4);
  const duplicateBeyondLimit = [...many.slice(0, 40), many[1]];
  assert.equal(dialogueGraphLayout({ sections: duplicateBeyondLimit, routes: [route(0, "node1")] }).edges.length, 0);
  assert.equal(dialogueGraphLayout({ sections: [sections[0], sections[0]], routes: [route(0, "start")] }).edges.length, 0);
});

test("non-overlapping routes reuse lanes and all route types keep source ownership", () => {
  const kinds = ["choice", "fallback", "wait", "branchTrue", "branchFalse", "actionSuccess", "actionFailure"];
  const graph = dialogueGraphLayout({ sections, routes: [route(0, "start"), route(2, "done")] });
  assert.equal(graph.edges[0].x, graph.edges[1].x);
  const linked = [];
  const view = dialogueGraph({ sections, routes: kinds.map(kind => route(0, "done", kind)) }, (label, source) => {
    linked.push({ label, source });
    return "source link";
  });
  assert.equal(linked.length, sections.length + kinds.length);
  assert.ok(linked.every(link => link.source === range));
  assert.match(view.html, /Action succeeds/);
  assert.match(view.html, /Action fails/);
  assert.match(view.html, /After wait/);
  assert.match(view.html, /aria-label="Map route source links"/);
  assert.equal(dialogueGraph(null, () => {}).html, "");
});
