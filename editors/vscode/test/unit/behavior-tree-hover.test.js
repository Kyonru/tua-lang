"use strict";

const assert = require("node:assert/strict");
const Module = require("node:module");
const test = require("node:test");

const load = Module._load;
Module._load = function (name, ...args) {
  if (name === "vscode") return {
    MarkdownString: class { constructor(value) { this.value = value; } },
    Hover: class { constructor(contents) { this.contents = contents; } },
    Position: class { constructor(line, character) { this.line = line; this.character = character; } },
    Range: class { constructor(start, end) { this.start = start; this.end = end; } },
  };
  return load.call(this, name, ...args);
};
const { hoverActionsAtPosition } = require("../../src/editor/hover-actions");
Module._load = load;

test("hover links to the behavior tree selected by the inspector", async () => {
  const uri = "file:///game/src/ai.tua";
  const document = {
    uri: { toString: () => uri }, version: 1,
    getWordRangeAtPosition: () => ({ start: { line: 0, character: 6 } }),
    lineAt: () => ({ text: "local patrol = BehaviorTree.new()" }),
  };
  const position = { line: 0, character: 7 };
  const hover = await hoverActionsAtPosition({
    document, position, isTuaDocument: () => true,
    getClient: () => ({ sendRequest: async (method, params) => {
      assert.equal(method, "tua/inspect");
      assert.deepEqual(params.position, position);
      return { documentVersion: 1, items: [{ kind: "behaviorTree", name: "patrol" }] };
    } }),
  });
  assert.match(hover.contents.value, /Preview patrol Behavior Tree/);
  assert.match(hover.contents.value, /command:tua\.previewBehaviorTree/);
  assert.deepEqual(hover.contents.isTrusted.enabledCommands.includes("tua.previewBehaviorTree"), true);
});

test("asset hover opens or reveals the indexed asset at the reference", async () => {
  const uri = "file:///game/src/main.tua";
  const document = {
    uri: { toString: () => uri }, version: 1,
    getWordRangeAtPosition: () => undefined,
    lineAt: () => ({ text: 'local image = image "assets/hero.png"' }),
  };
  const hover = await hoverActionsAtPosition({
    document, position: { line: 0, character: 25 }, isTuaDocument: () => true,
    getClient: () => ({ sendRequest: async method => method === "tua/assetInventory"
      ? { inventory: { assets: [{ path: "assets/hero.png", uri: "file:///game/assets/hero.png" }] } }
      : { documentVersion: 1, items: [] } }),
  });
  assert.match(hover.contents.value, /Preview Asset/);
  assert.match(hover.contents.value, /Reveal Asset/);
  assert(hover.contents.isTrusted.enabledCommands.includes("tua.assets.reveal"));
});
