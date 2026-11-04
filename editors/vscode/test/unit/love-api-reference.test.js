"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const Module = require("node:module");
const path = require("node:path");

const commands = new Map();
let panel;
const vscode = {
  ViewColumn: { One: 1 },
  commands: {
    registerCommand: (id, callback) => {
      commands.set(id, callback);
      return { dispose() {} };
    },
  },
  window: {
    createWebviewPanel: (...args) => {
      panel = { args, webview: {} };
      return panel;
    },
  },
};

const originalLoad = Module._load;
Module._load = function (id, ...args) {
  return id === "vscode" ? vscode : originalLoad.call(this, id, ...args);
};
const { COMMAND, registerLoveApiReference } = require("../../src/love/api-reference");
Module._load = originalLoad;

test("command opens the bundled LÖVE API reference in an offline webview", () => {
  const extensionPath = path.resolve(__dirname, "../..");
  const disposable = registerLoveApiReference({ extensionPath });

  commands.get(COMMAND)();

  assert.equal(panel.args[0], "tuaLoveApiReference");
  assert.equal(panel.args[1], "LÖVE API Reference (Offline)");
  assert.equal(panel.args[2], vscode.ViewColumn.One);
  assert.equal(panel.args[3].enableScripts, true);
  assert.match(panel.webview.html, /Content-Security-Policy/);
  assert.match(panel.webview.html, /LÖVE 11\.5 Reference/);
  assert.match(panel.webview.html, /love\.graphics\.newImage/);
  assert.deepEqual(
    require("../../package.json").contributes.commands.find(item => item.command === COMMAND),
    { command: COMMAND, title: "Open LÖVE API Reference (Offline)", category: "Tua" },
  );

  disposable.dispose();
});
