"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const Module = require("node:module");

const originalLoad = Module._load;
Module._load = function load(name, ...args) {
  if (name === "vscode") return {};
  return originalLoad.call(this, name, ...args);
};
const { buildInspectionSourceIndex, mayContainInspectionAtOffset } =
  require("../../src/editor/hover-actions");
Module._load = originalLoad;

function candidate(source, needle, word) {
  return mayContainInspectionAtOffset(
    buildInspectionSourceIndex(source), source.indexOf(needle), word,
  );
}

test("preview hover inspection stays on declarations and same-file references", () => {
  for (const [constructor, body] of [
    ["spriteSheet", "{ frame = { width = 16, height = 16 } }"],
    ["stateMachine", "{ initial = 'ready', events = {} }"],
    ["BehaviorTree.new", "BehaviorTree.selector({})"],
  ]) {
    const source = `local Preview = ${constructor}(\n  ${body}\n)\nlocal unrelated = 1\nlocal alias = Preview\n`;
    const index = buildInspectionSourceIndex(source);
    assert.equal(index.uncertain, false, constructor);
    assert.equal(candidate(source, "unrelated", "unrelated"), false, constructor);
    assert.equal(candidate(source, "Preview =", "Preview"), true, constructor);
    assert.equal(candidate(source, "alias = Preview", "Preview"), true, constructor);
    assert.equal(candidate(source, body, "frame"), true, constructor);
  }
});

test("unknown constructor shapes conservatively retain inspection", () => {
  const source = "local Preview = (stateMachine({ initial = 'ready' }))\nlocal unrelated = 1\n";
  const index = buildInspectionSourceIndex(source);
  assert.equal(index.uncertain, true);
  assert.equal(candidate(source, "unrelated", "unrelated"), true);
  assert.equal(candidate("local unrelated = 1\n", "unrelated", "unrelated"), false);
  const interpolated = "local Preview = stateMachine({ label = `closed)` })\nlocal unrelated = 1\n";
  assert.equal(buildInspectionSourceIndex(interpolated).uncertain, true);
  assert.equal(candidate(interpolated, "unrelated", "unrelated"), true);
});

test("comments and strings cannot create false preview declarations", () => {
  const source = `-- local Fake = stateMachine({})\nlocal text = "spriteSheet({})"\nlocal Real = BehaviorTree.new(BehaviorTree.selector({}))\nlocal unrelated = 1\n`;
  const index = buildInspectionSourceIndex(source);
  assert.equal(index.uncertain, false);
  assert.deepEqual([...index.names], ["Real"]);
  assert.equal(candidate(source, "unrelated", "unrelated"), false);
  assert.equal(candidate(source, "Real =", "Real"), true);
});
