"use strict";

const assert = require("assert/strict");
const test = require("node:test");
const { restoreGraphState } = require("../../src/state-machines/selection");

const preview = {
  source: "file:///game/actors.tua",
  machines: [
    { name: "menu", states: [], transitions: [] },
    { name: "player", states: [{ name: "idle" }, { name: "running" }], transitions: [
      { id: "player:start:5", event: "start", from: ["idle"], to: "running" },
      { id: "player:reset:6", event: "reset", from: [], to: "idle", wildcardFrom: true },
    ] },
  ],
};

test("graph selection follows names when machines and transition indices change", () => {
  const restored = restoreGraphState(preview, {
    source: preview.source, machine: "player", left: 180, top: 60,
    selection: { kind: "transition", id: "old-id", event: "start", source: "idle", to: "running" },
  });
  assert.equal(restored.machineIndex, 1);
  assert.deepEqual(restored.selection, { kind: "transition", id: "player:start:5", source: "idle" });
  assert.equal(restored.left, 180);
  assert.equal(restored.top, 60);
});

test("graph restoration rejects other files and removed selections", () => {
  const saved = { source: preview.source, machine: "player", selection: { kind: "state", name: "deleted" }, left: -10, top: Infinity };
  assert.deepEqual(restoreGraphState(preview, saved), { machineIndex: 1, selection: { kind: "machine" }, left: 0, top: 0 });
  assert.equal(restoreGraphState(preview, { ...saved, source: "file:///other.tua" }).machineIndex, 0);
  assert.equal(restoreGraphState(preview, { ...saved, machine: "deleted" }).machineIndex, 0);
});
