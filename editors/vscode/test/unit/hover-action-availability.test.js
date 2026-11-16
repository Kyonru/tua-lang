"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const Module = require("node:module");

class MarkdownString {
  constructor(value) { this.value = value; }
}
class Hover {
  constructor(contents, range) { this.contents = contents; this.range = range; }
}
const originalLoad = Module._load;
Module._load = function load(name, ...args) {
  if (name === "vscode") return { Hover, MarkdownString };
  return originalLoad.call(this, name, ...args);
};
const { hoverActionsAtPosition, isMemberIdentifier, provideMemberHoverActions } =
  require("../../src/editor/hover-actions");
Module._load = originalLoad;

function document(line) {
  const word = /[A-Za-z_][A-Za-z0-9_]*$/.exec(line)?.[0];
  const range = word && { start: { line: 0, character: line.length - word.length }, end: { line: 0, character: line.length } };
  return {
    uri: { toString: () => "file:///main.tua" },
    version: 1,
    getText: () => line,
    lineAt: () => ({ text: line }),
    getWordRangeAtPosition: () => range,
  };
}

test("member hover links require the existing LSP hover and do not send another request", async () => {
  const source = document("local typo = metadata.healht");
  const position = { line: 0, character: source.lineAt().text.length - 1 };
  let calls = 0;
  const next = () => { calls += 1; return null; };
  const result = await provideMemberHoverActions({ document: source, isTuaDocument: () => true, next, position });
  assert.equal(result, null);
  assert.equal(calls, 1);
  assert.equal(await hoverActionsAtPosition({ document: source, getClient: () => { throw Error("unexpected request"); }, isTuaDocument: () => true, position }), undefined);
});

test("known dot and colon members append trusted navigation to one LSP hover", async () => {
  for (const line of ["local health = metadata.health", "local width = image:getWidth"]) {
    const source = document(line);
    const position = { line: 0, character: line.length - 1 };
    let calls = 0;
    const original = new Hover([new MarkdownString("member documentation")]);
    const result = await provideMemberHoverActions({
      document: source, isTuaDocument: () => true,
      next: () => { calls += 1; return original; }, position,
    });
    assert.equal(calls, 1);
    assert.equal(result.contents[0].value, "member documentation");
    assert.match(result.contents[1].value, /command:tua\.hover\.references/);
    assert.match(result.contents[1].value, /command:tua\.hover\.implementations/);
    assert.deepEqual(result.contents[1].isTrusted.enabledCommands,
      ["tua.hover.references", "tua.hover.implementations"]);
  }
});

test("bare identifiers stay on the separate hover provider path", async () => {
  const source = document("loadAssets");
  const range = source.getWordRangeAtPosition();
  assert.equal(isMemberIdentifier(source, range), false);
  assert.equal(isMemberIdentifier(document("image : getWidth"),
    document("image : getWidth").getWordRangeAtPosition()), true);
  const original = new Hover([new MarkdownString("root")]);
  const result = await provideMemberHoverActions({
    document: source, isTuaDocument: () => true,
    next: () => original, position: { line: 0, character: 3 },
  });
  assert.equal(result, original);
});
