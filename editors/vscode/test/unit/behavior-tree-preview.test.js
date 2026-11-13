"use strict";

const assert = require("node:assert/strict");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const { buildSync } = require("esbuild");

const { outputFiles } = buildSync({
  entryPoints: [path.resolve(__dirname, "../../src/behavior-trees/preview.js")],
  bundle: true,
  platform: "node",
  format: "cjs",
  external: ["vscode"],
  write: false,
});
let previewCommand;
let panel;
let onDocumentChange;
const document = { uri: { toString: () => "file:///game/ai.tua" }, version: 1 };
const vscode = {
  Uri: { parse: value => ({ fsPath: new URL(value).pathname }) },
  ViewColumn: { Beside: 2 },
  Disposable: { from: () => ({ dispose() {} }) },
  commands: { registerCommand(_name, handler) { previewCommand = handler; return { dispose() {} }; } },
  workspace: {
    asRelativePath: uri => uri.fsPath.replace("/game/", ""),
    getConfiguration: () => ({ get: () => true }),
    onDidChangeTextDocument: handler => { onDocumentChange = handler; return { dispose() {} }; },
  },
  window: {
    activeTextEditor: { document },
    createWebviewPanel() {
      panel = { visible: true, webview: { cspSource: "test", onDidReceiveMessage() {} }, onDidDispose() {}, onDidChangeViewState(handler) { panel.onViewState = handler; }, reveal() {} };
      return panel;
    },
  },
};
const context = { module: { exports: {} }, require: (name) => name === "vscode" ? vscode : require(name), setTimeout, clearTimeout };
vm.runInNewContext(outputFiles[0].text, context);
const { behaviorTreePreviewHtml, registerBehaviorTreePreview } = context.module.exports;

test("behavior tree preview renders nested and computed nodes with safe source links", () => {
  const range = (line) => ({ start: { line, character: 2 }, end: { line, character: 18 } });
  const html = behaviorTreePreviewHtml(
    { cspSource: "test" },
    "file:///game/ai.tua",
    [{ name: "enemy<script>", root: { kind: "selector", range: range(1), children: [
      { kind: "action", range: range(2), children: [] },
      { kind: "dynamic", range: range(3), children: [] },
    ] } }],
  );
  assert.match(html, /enemy&lt;script&gt;/);
  assert.doesNotMatch(html, /enemy<script>/);
  assert.match(html, /Computed node/);
  assert.match(html, /openLocation/);
  assert.match(html, /"line":3/);
  assert.match(html, /Content-Security-Policy/);
  assert.match(html, /tree-scroll/);
  assert.match(html, /tree li::before/);
  assert.match(html, /title="file:\/\/\/game\/ai\.tua">ai\.tua/);
  assert.match(html, /node--branch/);
});

test("hover command opens only the targeted tree", async () => {
  const range = { start: { line: 2, character: 0 }, end: { line: 2, character: 10 } };
  const requests = [];
  const client = { sendRequest: async (_method, params) => {
    requests.push(params);
    return { documentVersion: 1, items: [
      { kind: "behaviorTree", name: "patrol", root: { kind: "wait", range, children: [] } },
    ] };
  } };
  registerBehaviorTreePreview({
    configurationTarget: () => undefined,
    isTuaDocument: () => true,
    getClient: () => client,
  });
  await previewCommand({ uri: document.uri.toString(), position: { line: 2, character: 6 } });
  assert.equal(requests.length, 1);
  assert.equal(requests[0].position.character, 6);
  assert.match(panel.webview.html, /patrol/);
  assert.doesNotMatch(panel.webview.html, /Root not set/);
});

test("hidden behavior-tree preview defers inspection until reveal", async () => {
  const requests = [];
  const client = { sendRequest: async (_, params) => {
    requests.push(params);
    return { documentVersion: document.version, items: [
      { kind: "behaviorTree", name: `patrol${document.version}`, root: { kind: "wait", children: [] } },
    ] };
  } };
  registerBehaviorTreePreview({ configurationTarget: () => undefined, isTuaDocument: () => true, getClient: () => client });
  await previewCommand();
  assert.equal(requests.length, 1);
  panel.visible = false;
  document.version++;
  onDocumentChange({ document });
  await new Promise(resolve => setTimeout(resolve, 220));
  assert.equal(requests.length, 1, "editing a hidden preview must not inspect");
  panel.visible = true;
  await panel.onViewState();
  assert.equal(requests.length, 2);
  assert.match(panel.webview.html, /patrol2/);
  document.version = 1;
});
