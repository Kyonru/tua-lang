"use strict";

const assert = require("assert/strict");
const Module = require("module");
const test = require("node:test");

const disposable = { dispose() {} };
class EventEmitter {
  constructor() { this.event = () => disposable; }
  fire() {}
  dispose() {}
}
const vscode = {
  EventEmitter,
  TreeItem: class { constructor(label) { this.label = label; } },
  ThemeIcon: class {},
  TreeItemCollapsibleState: { None: 0, Collapsed: 1 },
  Uri: { parse: (value) => ({ toString: () => value }) },
  languages: { createDiagnosticCollection: () => ({ clear() {}, dispose() {} }) },
  window: { createTreeView: () => ({ onDidChangeVisibility: () => disposable, dispose() {} }) },
};
const originalLoad = Module._load;
Module._load = function (id, parent, main) {
  return id === "vscode" ? vscode : originalLoad.call(this, id, parent, main);
};
const { AssetExplorerController } = require("../../src/love/assets");
const { AssetTreeDataProvider } = require("../../src/love/assets/tree");
Module._load = originalLoad;

test("asset tree identities survive changed counts and retain deterministic order", () => {
  const provider = new AssetTreeDataProvider();
  const asset = (name) => ({ uri: `file:///assets/${name}`, path: name, category: "image", categoryLabel: "Images", references: [] });
  provider.update([asset("sprite10.png"), asset("sprite2.png")]);
  const category = provider.getChildren()[0];
  const id = provider.getTreeItem(category).id;
  const usage = provider.getChildren(category)[0];
  assert.equal(usage.label, "No static references");
  assert.deepEqual(provider.getChildren(usage).map((node) => node.asset.path), ["sprite2.png", "sprite10.png"]);
  provider.update([asset("sprite1.png")]);
  assert.equal(provider.getTreeItem(provider.getChildren()[0]).id, id);
});

test("disposing the explorer ignores a late inventory response", async () => {
  let resolve;
  const client = { sendRequest: () => new Promise((done) => { resolve = done; }) };
  const controller = new AssetExplorerController({ getClient: () => client });
  let updates = 0;
  controller.provider.update = () => { updates += 1; };
  const request = controller.refresh();
  controller.dispose();
  resolve({ inventory: { assets: [], diagnostics: [] } });
  assert.equal(await request, undefined);
  assert.equal(updates, 0);
  controller.scheduleRefresh();
  assert.equal(controller.refreshTimer, undefined);
  assert.equal(await controller.refresh(), undefined);
});

test("an older inventory cannot replace a newer result", async () => {
  const responses = [];
  const client = { sendRequest: () => new Promise((resolve) => responses.push(resolve)) };
  const controller = new AssetExplorerController({ getClient: () => client });
  const first = controller.refresh();
  const second = controller.refresh();
  responses[1]({ inventory: { assets: [], diagnostics: [] } });
  await second;
  responses[0]({ inventory: { assets: [{ path: "stale.png" }], diagnostics: [] } });
  await first;
  assert.deepEqual(controller.provider.assets, []);
  assert.match(controller.view.message, /No supported assets/);
  controller.dispose();
});
