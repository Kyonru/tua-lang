"use strict";

const assert = require("node:assert/strict");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const { buildSync } = require("esbuild");
const { outputFiles } = buildSync({ entryPoints: [path.resolve(__dirname, "../../src/dialogue/preview.js")], bundle: true, platform: "node", format: "cjs", external: ["vscode"], write: false });
const range = { start: { line: 3, character: 1 }, end: { line: 3, character: 6 } };
const preview = { sections: [{ label: "start<script>", range, lines: [{ text: "Hello %{name}", range, choices: [{ text: "Go", visibleCondition: true }] }] }], routes: [{ section: 0, line: 0, choice: 0, kind: "choice", target: "start<script>", range }] };

function fixture(sendRequest) {
  let command;
  let panel;
  let onChange;
  const opened = [];
  const document = { languageId: "tdialogue", version: 1, uri: { toString: () => "file:///scene.tdialogue" } };
  const vscode = {
    Uri: { parse: value => ({ fsPath: new URL(value).pathname }) },
    Position: class { constructor(line, character) { this.line = line; this.character = character; } },
    Range: class {},
    ViewColumn: { Beside: 2 },
    Disposable: { from: () => ({ dispose() {} }) },
    commands: {
      registerCommand(_name, handler) { command = handler; return { dispose() {} }; },
      executeCommand(...args) { opened.push(args); },
    },
    workspace: {
      getConfiguration: () => ({ get: () => true }),
      onDidChangeTextDocument(handler) { onChange = handler; return { dispose() {} }; },
    },
    window: {
      activeTextEditor: { document },
      createWebviewPanel() {
        panel = { visible: true, webview: { cspSource: "test", onDidReceiveMessage(handler) { panel.receive = handler; } }, onDidDispose(handler) { panel.dispose = handler; }, onDidChangeViewState(handler) { panel.reveal = handler; } };
        return panel;
      },
    },
  };
  const context = { module: { exports: {} }, require: name => name === "vscode" ? vscode : require(name), setTimeout, clearTimeout };
  vm.runInNewContext(outputFiles[0].text, context);
  const api = context.module.exports;
  const client = { sendRequest: (...args) => sendRequest(document, ...args) };
  api.registerDialoguePreview({ configurationTarget: () => undefined, getClient: () => client });
  return { ...api, document, opened, show: () => command(), panel: () => panel, change: () => onChange({ document }) };
}

test("dialogue outline escapes content, preserves authored text and limits large views", () => {
  const f = fixture();
  const html = f.dialoguePreviewHtml({ cspSource: "test" }, f.document.uri.toString(), preview);
  assert.match(html, /start&lt;script&gt;/);
  assert.doesNotMatch(html, /start<script>/);
  assert.match(html, /Hello %\{name\}/);
  assert.match(html, /Visibility condition \(not evaluated\)/);
  assert.match(html, /Content-Security-Policy/);
  assert.match(html, /focus-visible/);
  const large = f.dialoguePreviewHtml({ cspSource: "test" }, f.document.uri.toString(), { sections: Array(201).fill(preview.sections[0]), routes: [] });
  assert.equal((large.match(/<section>/g) || []).length, 200);
  assert.match(large, /Showing the first 200 sections/);
});

test("hidden preview refreshes on reveal and rejects stale navigation", async () => {
  let calls = 0;
  const f = fixture(async (document, method) => {
    calls++;
    assert.equal(method, "tua/dialoguePreview");
    return { documentVersion: document.version, preview };
  });
  await f.show();
  const panel = f.panel();
  await panel.receive({ command: "openLocation", uri: f.document.uri.toString(), range });
  assert.equal(f.opened.length, 1);
  panel.visible = false;
  f.document.version++;
  f.change();
  await panel.receive({ command: "openLocation", uri: f.document.uri.toString(), range });
  assert.equal(f.opened.length, 1);
  await new Promise(resolve => setTimeout(resolve, 220));
  assert.equal(calls, 1);
  panel.visible = true;
  await panel.reveal();
  assert.equal(calls, 2);
  panel.dispose();
});

test("invalid source clears prior content and stale responses do not render", async () => {
  let result = { documentVersion: 1, preview };
  const f = fixture(async () => result);
  await f.show();
  result = { documentVersion: 1, preview: null };
  await f.panel().receive({ command: "reload" });
  assert.match(f.panel().webview.html, /Check Problems/);
  assert.doesNotMatch(f.panel().webview.html, /Hello %/);
  result = { documentVersion: 0, preview };
  await f.panel().receive({ command: "reload" });
  assert.match(f.panel().webview.html, /Source changed during analysis/);
  f.panel().dispose();
});
