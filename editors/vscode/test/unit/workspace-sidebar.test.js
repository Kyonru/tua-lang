"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const Module = require("node:module");
const vscode = {
  EventEmitter: class { constructor() { this.event = () => {}; } fire() {} dispose() {} },
  Uri: { parse: value => ({ fsPath: new URL(value).pathname }) },
};
const original = Module._load;
Module._load = function(id, ...args) { return id === "vscode" ? vscode : original.call(this, id, ...args); };
const { WorkspaceTree, graphNodes, registerWorkspaceSidebar } = require("../../src/workspace/sidebar");
Module._load = original;

test("workspace graphs retain declaration locations and stable identities", () => {
  const range = { start: { line: 2, character: 0 }, end: { line: 2, character: 6 } };
  const result = graphNodes("modules", { graph: { modules: [{ uri: "file:///game/player.tua", exports: [{ name: "Player", range }] }] } });
  assert.equal(result[0].label, "player.tua");
  assert.equal(result[0].children[0].uri, "file:///game/player.tua");
  assert.deepEqual(result[0].children[0].range, range);
  assert.equal(graphNodes("signals", { events: [] })[0].label, "No declarations found");
});

test("workspace requests are lazy and invalidate on refresh or server change", async () => {
  let calls = 0;
  let client = { sendRequest: async () => { calls++; return { events: [] }; } };
  const tree = new WorkspaceTree(() => client);
  const node = (await tree.getChildren()).find(item => item.kind === "signals");
  assert.equal(calls, 0);
  await tree.getChildren(node);
  await tree.getChildren(node);
  assert.equal(calls, 1);
  tree.refresh();
  await tree.getChildren(node);
  assert.equal(calls, 2);
  client = { ...client };
  await tree.getChildren(node);
  assert.equal(calls, 3);
});


test("workspace sidebar invalidates edited and externally changed sources and disposes timers", async () => {
  const events = {};
  const listen = name => callback => { events[name] = callback; return { dispose() { delete events[name]; } }; };
  vscode.Disposable = class { constructor(fn) { this.dispose = fn; } static from(...items) { return new this(() => items.forEach(item => item.dispose())); } };
  let provider;
  vscode.window = { createTreeView: (_, options) => { provider = options.treeDataProvider; return { dispose() {} }; } };
  vscode.commands = { registerCommand: () => ({ dispose() {} }) };
  vscode.workspace = {
    createFileSystemWatcher: () => ({ onDidCreate: listen("create"), onDidChange: listen("change"), onDidDelete: listen("delete"), dispose() {} }),
    onDidChangeTextDocument: listen("edit"), onDidOpenTextDocument: listen("open"), onDidCloseTextDocument: listen("close"),
    onDidSaveTextDocument: listen("save"), onDidChangeWorkspaceFolders: listen("folders"),
  };
  let calls = 0;
  const registration = registerWorkspaceSidebar({ getClient: () => client });
  const client = { sendRequest: async () => { calls++; return { events: [] }; } };
  const node = { id: "signals", kind: "signals", request: "tua/signalGraph" };
  await provider.getChildren(node);
  for (const event of ["edit", "create", "change", "delete"]) {
    events[event]({ document: { uri: { path: "/game/main.tua" } } });
    await new Promise(resolve => setTimeout(resolve, 180));
    await provider.getChildren(node);
  }
  for (const event of ["open", "close"]) {
    events[event]({ uri: { path: "/game/main.lua" } });
    await new Promise(resolve => setTimeout(resolve, 180));
    await provider.getChildren(node);
  }
  assert.equal(calls, 7);
  events.edit({ document: { uri: { path: "/game/main.tua" } } });
  registration.dispose();
  assert.deepEqual(Object.keys(events), []);
});


test("project files include Lua interop while retaining generated-directory exclusions", async () => {
  vscode.FileType = { File: 1, Directory: 2, SymbolicLink: 64 };
  vscode.Uri.joinPath = (uri, name) => ({ toString: () => `file://${uri.fsPath}/${name}` });
  vscode.workspace.fs = { readDirectory: async () => [
    ["main.tua", 1], ["interop.lua", 1], ["tua.toml", 1],
    ["build", 2], ["target", 2], ["node_modules", 2], ["linked.lua", 65],
  ] };
  const tree = new WorkspaceTree(() => undefined);
  const children = await tree.getChildren({ kind: "directory", uri: "file:///game" });
  assert.deepEqual(children.map(child => child.label), ["interop.lua", "main.tua", "tua.toml"]);
});
