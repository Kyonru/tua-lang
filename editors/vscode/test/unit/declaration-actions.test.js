"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { declarationAt, matchingInstances } = require("../../src/editor/declaration-target");

test("declaration targets handle multiline types, comments and nested arguments", () => {
  const source = 'local player: Player = store(\n { score = math.max(1, 2), text = "input({})" }\n)';
  const target = declarationAt(source, source.indexOf("score"));
  assert.equal(target.name, "player"); assert.equal(target.kind, "stores");
  assert.equal(source.slice(target.helperOffset, target.helperOffset + 5), "store");
  assert.equal(source.slice(target.nameOffset, target.nameOffset + 6), "player");
  assert.equal(declarationAt(source, source.length), undefined);
  for (const text of ['-- local player = store({})', 'local text = "local player = store({})"', 'local a, b = store({})', 'local a = custom.store({})', 'local a = store({']) {
    assert.equal(declarationAt(text, text.indexOf("store")), undefined);
  }
  for (const helper of ["input", "stateMachine", "objectPool"]) {
    const text = `local value = ${helper}({})`;
    assert.equal(declarationAt(text, 7).helper, helper);
  }
});

test("runtime selection requires the same source binding and connected player", () => {
  const target = { path: "/game/src/state.tua", line: 2, name: "player", kind: "stores" };
  const object = { id: 1, kind: "stores", origin: { ...target, generated: false } };
  const entry = { session: { active: true }, status: "connected", snapshot: { objects: [object, { ...object, id: 2 }] } };
  const entries = new Map([["host", entry], ["client", { ...entry }]]);
  assert.equal(matchingInstances(entries, target).length, 4);
  for (const changed of [{ line: 3 }, { name: "other" }, { path: "/other/state.tua" }, { kind: "pools" }]) assert.equal(matchingInstances(entries, { ...target, ...changed }).length, 0);
  entry.session.active = false;
  assert.equal(matchingInstances(entries, target).length, 0);
  entry.session.active = true; object.origin.generated = true;
  assert.equal(matchingInstances(entries, target).length, 0);
});

const Module = require("node:module"), load = Module._load;
let provider;
Module._load = function(name, ...args) {
  if (name === "vscode") return {
    Uri: { parse: value => value }, CodeAction: class { constructor(title) { this.title = title; } },
    languages: { registerCodeActionsProvider(_selector, value) { provider = value; return { dispose() {} }; } },
    commands: { registerCommand() { return { dispose() {} }; } },
    Disposable: { from() { return { dispose() {} }; } },
  };
  return load.call(this, name, ...args);
};
const { registerDeclarationActions, actionsFor } = require("../../src/editor/declaration-actions");
Module._load = load;

test("actions use explicit targets and do not advertise unimplemented live input inspection", () => {
  const target = { helper: "objectPool", name: "bullets", kind: "pools", uri: "file:///game.tua", line: 3 };
  const actions = actionsFor(target);
  assert.deepEqual(actions[0].arguments, [target]);
  assert.equal(actions[0].command, "tua.showObjectPoolExplorer");
  assert(actions.some(action => action.command === "tua.inspector.forDeclaration"));
  assert.deepEqual(actionsFor({ helper: "input", name: "controls", kind: "input" }).map(action => action.command), ["vscode.open"]);
});

test("code actions require a compiler-confirmed builtin and discard stale responses", async () => {
  const source = "local player = store({ score = 0 })";
  const document = { version: 1, getText: () => source, offsetAt: p => p.character, positionAt: character => ({ line: 0, character }), uri: { scheme: "file", fsPath: "/game.tua", toString: () => "file:///game.tua" } };
  let response = [{ uri: "tua:/builtins/store.tua" }], stale = false;
  registerDeclarationActions({ isTuaDocument: () => true, getClient: () => ({ async sendRequest(method, params) {
    assert.equal(method, "textDocument/definition"); assert.equal(params.position.character, source.indexOf("store"));
    if (stale) document.version++;
    return response;
  } }) });
  const range = { start: { line: 0, character: 7 } };
  assert.equal((await provider.provideCodeActions(document, range, {}, {})).length, 2);
  response = [{ uri: "file:///custom-store.tua" }];
  assert.deepEqual(await provider.provideCodeActions(document, range, {}, {}), []);
  response = [{ uri: "tua:/builtins/store.tua" }]; stale = true;
  assert.deepEqual(await provider.provideCodeActions(document, range, {}, {}), []);
});
