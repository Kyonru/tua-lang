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
    Uri: { parse: value => ({ scheme: value.startsWith("file:") ? "file" : "tua", fsPath: value.replace(/^file:\/\//, ""), toString: () => value }) },
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

test("ordinary member hover does not request preview inspection", async () => {
  let source = "local width = love.graphics.getWidth()";
  let version = 1;
  let requests = 0;
  const document = {
    uri: { toString: () => "file:///game/src/main.tua" },
    get version() { return version; },
    getText: () => source,
    getWordRangeAtPosition: () => ({ start: { line: 0, character: 19 } }),
    lineAt: () => ({ text: source }),
  };
  const options = {
    document,
    position: { line: 0, character: 23 },
    isTuaDocument: () => true,
    getClient: () => ({ sendRequest: async () => {
      requests += 1;
      return { documentVersion: version, items: [] };
    } }),
  };

  await hoverActionsAtPosition(options);
  await hoverActionsAtPosition(options);
  assert.equal(requests, 0);

  source = "local tree = BehaviorTree.new()";
  version += 1;
  await hoverActionsAtPosition(options);
  assert.equal(requests, 1);
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
    getClient: () => ({ sendRequest: async method => method === "textDocument/definition"
      ? { uri: "file:///game/assets/hero.png", range: { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } } }
      : { documentVersion: 1, items: [] } }),
  });
  assert.match(hover.contents.value, /Preview Asset/);
  assert.match(hover.contents.value, /Reveal Asset/);
  assert(hover.contents.isTrusted.enabledCommands.includes("tua.assets.reveal"));
});

test("asset-like text without an asset definition gets no asset actions", async () => {
  const document = { uri: { toString: () => "file:///game/main.tua" }, version: 1,
    getWordRangeAtPosition: () => undefined, lineAt: () => ({ text: 'print("missing.png")' }) };
  const hover = await hoverActionsAtPosition({ document, position: { line: 0, character: 12 },
    isTuaDocument: () => true, getClient: () => ({ sendRequest: async method => method === "textDocument/definition"
      ? null : { documentVersion: 1, items: [] } }) });
  assert.equal(hover, undefined);
});
