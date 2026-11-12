"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const Module = require("node:module");

const commands = [];
let offered = [], picked;
const position = { line: 0, character: 8 };
const document = {
  version: 1,
  uri: { toString: () => "file:///game.tua" },
};
const editor = { document, selection: { active: position } };
const vscode = {
  window: {
    activeTextEditor: editor,
    async showQuickPick(items) { offered = items; return picked?.(items); },
    async showTextDocument() { return editor; },
    showWarningMessage() { throw new Error("unexpected warning"); },
    showErrorMessage(message) { throw new Error(message); },
  },
  commands: {
    async executeCommand(name, ...args) {
      commands.push([name, ...args]);
      if (name === "vscode.executeCodeActionProvider") return [{ title: "Add annotation" }];
    },
  },
  Range: class { constructor(startLine, startCharacter, endLine, endCharacter) {
    this.start = { line: startLine, character: startCharacter };
    this.end = { line: endLine, character: endCharacter };
  } },
  Position: class {},
  Selection: class { constructor(anchor, active) { this.anchor = anchor; this.active = active; } },
};
const original = Module._load;
Module._load = function(id, ...args) { return id === "vscode" ? vscode : original.call(this, id, ...args); };
const { explainTypeAtCursor } = require("../../src/editor/type-explanation");
Module._load = original;

test("type explanation offers compiler-backed quick fixes at its source range", async () => {
  commands.length = 0;
  picked = items => items.find(item => item.action === "quickfix");
  const response = { documentVersion: 1, explanation: { expression: "value", type: "any", isAny: true,
    range: { start: { line: 0, character: 6 }, end: { line: 0, character: 11 } },
    steps: [{ kind: "dynamic", title: "Dynamic value", detail: "Unknown module" }] } };
  const result = await explainTypeAtCursor({ isTuaDocument: () => true, getClient: () => ({ sendRequest: async () => response }) });
  assert.equal(result.isAny, true);
  assert(offered.some(item => item.action === "quickfix"));
  assert.equal(commands[0][0], "vscode.executeCodeActionProvider");
  assert.equal(commands[0][3], "quickfix");
  assert.equal(commands.at(-1)[0], "editor.action.quickFix");
  assert.equal(editor.selection.active, position);
});
