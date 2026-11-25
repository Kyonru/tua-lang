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
  let onClose;
  let diskChange;
  let diskCreate;
  let diskDelete;
  let watcherDisposed = false;
  let reopen = async () => document;
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
      openTextDocument: () => reopen(),
      onDidCloseTextDocument(handler) { onClose = handler; return { dispose() {} }; },
      createFileSystemWatcher: () => ({
        onDidChange(handler) { diskChange = handler; },
        onDidCreate(handler) { diskCreate = handler; },
        onDidDelete(handler) { diskDelete = handler; },
        dispose() { watcherDisposed = true; },
      }),
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
  return { ...api, document, opened,
    disk: (kind, uri = document.uri) => ({ change: diskChange, create: diskCreate, delete: diskDelete })[kind](uri),
    setReopen: handler => { reopen = handler; },
    watcherDisposed: () => watcherDisposed,
    close: () => onClose(document),
    show: () => command(), panel: () => panel, change: () => onChange({ document }) };
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

test("mock input is explicit, remains a draft during refresh and can be cleared", async () => {
  const requests = [];
  const f = fixture(async (document, _method, params) => {
    requests.push(params);
    return { documentVersion: document.version, preview };
  });
  await f.show();
  const uri = f.document.uri.toString();
  const text = '{"context":{"coins":2}}';
  assert.equal(requests[0].mockContext, undefined);
  await f.panel().receive({ command: "mockDraft", uri, text });
  assert.equal(requests.length, 1);
  await f.panel().receive({ command: "reload" });
  assert.equal(requests[1].mockContext, undefined);
  assert.match(f.panel().webview.html, /&quot;coins&quot;:2/);
  await f.panel().receive({ command: "applyMock", uri, text });
  assert.equal(requests[2].mockContext, text);
  assert.match(f.panel().webview.html, /compiler does not support mock evaluation/);
  await f.panel().receive({ command: "clearMock", uri });
  assert.equal(requests[3].mockContext, undefined);
  await f.panel().receive({ command: "applyMock", uri: "file:///other.tdialogue", text });
  assert.equal(requests.length, 4);
  f.panel().dispose();
  await f.show();
  assert.equal(requests[4].mockContext, undefined);
  assert.doesNotMatch(f.panel().webview.html, /&quot;coins&quot;:2/);
  f.panel().dispose();
});

test("clearing mock input discards an older in-flight result", async () => {
  let finish;
  const f = fixture(async (document, _method, params) => {
    if (params.mockContext !== undefined) return new Promise(resolve => { finish = resolve; });
    return { documentVersion: document.version, preview };
  });
  await f.show();
  const uri = f.document.uri.toString();
  const pending = f.panel().receive({ command: "applyMock", uri, text: "{}" });
  await f.panel().receive({ command: "clearMock", uri });
  finish({ documentVersion: 1, preview: { ...preview, mock: {
    conditions: [{ section: 0, line: null, choice: null, kind: "branch", value: true, range }],
    text: { texts: [{ section: 0, line: 0, choice: null, kind: "line", value: "stale replacement", range }] },
  } } });
  await pending;
  assert.doesNotMatch(f.panel().webview.html, /Mock branch: true|stale replacement/);
  f.panel().dispose();
});

test("mock errors and results are source-linked and HTML-safe", () => {
  const f = fixture();
  const evaluated = { ...preview, mock: { truncated: true, conditions: [{ section: 0, line: 0, choice: 0, kind: "visibility", error: "bad <type>", range }] } };
  const html = f.dialoguePreviewHtml({ cspSource: "test" }, f.document.uri.toString(), evaluated, undefined, { draft: '</textarea><script>bad()</script>', error: 'invalid <JSON>' });
  assert.match(html, /Mock visibility: bad &lt;type&gt;/);
  assert.match(html, /Open condition/);
  assert.match(html, /invalid &lt;JSON&gt;/);
  assert.doesNotMatch(html, /<script>bad/);
  assert.match(html, /4,096 conditions/);
});

test("mock text is escaped, keeps authored disclosures, and renders choice/reason fields", () => {
  const f = fixture();
  const model = { sections: [{ label: "start", range, lines: [{ text: "Hello %{name}", range, choices: [{ text: "Buy %{item}", reason: "Need %{coins}" }] }] }], routes: [{ section: 0, line: 0, choice: 0, kind: "choice", target: "start", range }], mock: { conditions: [], text: { texts: [
    { section: 0, line: 0, choice: null, kind: "line", value: "Hello <script>bad()</script>", range },
    { section: 0, line: 0, choice: 0, kind: "choice", value: "Buy shell", range },
    { section: 0, line: 0, choice: 0, kind: "reason", value: "Need 20", range },
  ] } } };
  const html = f.dialoguePreviewHtml({ cspSource: "test" }, f.document.uri.toString(), model);
  assert.match(html, /Hello &lt;script&gt;bad\(\)&lt;\/script&gt;/);
  assert.doesNotMatch(html, /<script>bad/);
  assert.match(html, /<summary>Authored text<\/summary>/);
  assert.match(html, /Hello %\{name\}/);
  assert.match(html, /Buy shell/);
  assert.match(html, /Need 20/);
});

test("text errors and limits preserve authored text", () => {
  const f = fixture();
  const model = { ...preview, mock: { conditions: [], text: { texts: [{ section: 0, line: 0, choice: null, kind: "line", error: "missing argument", range }], truncated: true } } };
  const html = f.dialoguePreviewHtml({ cspSource: "test" }, f.document.uri.toString(), model);
  assert.match(html, /Hello %\{name\}/);
  assert.match(html, /Mock text: missing argument/);
  assert.match(html, /Open source/);
  assert.match(html, /Authored text \(preview limit\)/);
  assert.match(html, /1 MiB combined output/);
  model.mock.text = { texts: [], bindingError: { message: "missing binding", range } };
  assert.match(f.dialoguePreviewHtml({ cspSource: "test" }, f.document.uri.toString(), model), /Mock text unavailable: missing binding/);
});


test("disk deletion clears source links and recreation refreshes on reveal", async () => {
  let calls = 0;
  const f = fixture(async document => {
    calls++;
    return { documentVersion: document.version, preview };
  });
  await f.show();
  f.disk("delete");
  assert.match(f.panel().webview.html, /file was deleted/);
  await f.panel().receive({ command: "openLocation", uri: f.document.uri.toString(), range });
  assert.equal(f.opened.length, 0);
  await f.panel().receive({ command: "reload" });
  assert.equal(calls, 1);
  f.panel().visible = false;
  f.document.isClosed = true;
  const reopened = { ...f.document, version: 1, isClosed: false };
  f.setReopen(async () => reopened);
  f.disk("create");
  await new Promise(resolve => setTimeout(resolve, 220));
  assert.equal(calls, 1);
  f.panel().visible = true;
  await f.panel().reveal();
  assert.equal(calls, 2);
  assert.match(f.panel().webview.html, /Hello %/);
  f.panel().dispose();
  assert.equal(f.watcherDisposed(), true);
});

test("disk events ignore unrelated files and preserve dirty buffer authority", async () => {
  let calls = 0;
  const f = fixture(async document => {
    calls++;
    return { documentVersion: document.version, preview };
  });
  await f.show();
  f.disk("change", { toString: () => "file:///other.tdialogue" });
  f.document.isDirty = true;
  f.disk("delete");
  f.disk("create");
  f.disk("change");
  await new Promise(resolve => setTimeout(resolve, 220));
  assert.equal(calls, 1);
  assert.match(f.panel().webview.html, /Hello %/);
  f.document.isDirty = false;
  f.disk("change");
  assert.doesNotMatch(f.panel().webview.html, /Hello %/);
  await new Promise(resolve => setTimeout(resolve, 220));
  assert.equal(calls, 2);
  f.panel().dispose();
});

test("a disk invalidation rejects a response already in flight", async () => {
  let finish;
  const f = fixture(() => new Promise(resolve => { finish = resolve; }));
  const showing = f.show();
  await new Promise(resolve => setTimeout(resolve, 0));
  f.disk("delete");
  finish({ documentVersion: 1, preview });
  await showing;
  assert.match(f.panel().webview.html, /file was deleted/);
  assert.doesNotMatch(f.panel().webview.html, /Hello %/);
  f.panel().dispose();
});


test("discarding a dirty buffer after disk deletion clears the preview", async () => {
  let calls = 0;
  const f = fixture(async document => {
    calls++;
    return { documentVersion: document.version, preview };
  });
  await f.show();
  f.document.isDirty = true;
  f.disk("delete");
  assert.match(f.panel().webview.html, /Hello %/);
  f.document.isDirty = false;
  f.document.isClosed = true;
  f.close();
  await new Promise(resolve => setTimeout(resolve, 220));
  assert.equal(calls, 1);
  assert.match(f.panel().webview.html, /file was deleted/);
  f.panel().dispose();
});

test("a closed buffer is reopened and disposal rejects its pending load", async () => {
  const f = fixture(async document => ({ documentVersion: document.version, preview }));
  await f.show();
  f.document.isClosed = true;
  let reopen;
  f.setReopen(() => new Promise(resolve => { reopen = resolve; }));
  const panel = f.panel();
  const loading = panel.receive({ command: "reload" });
  panel.dispose();
  const html = panel.webview.html;
  reopen({ ...f.document, isClosed: false });
  await loading;
  assert.equal(panel.webview.html, html);
});
