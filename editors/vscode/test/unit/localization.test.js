"use strict";

const assert = require("node:assert/strict");
const Module = require("node:module");
const test = require("node:test");
const { localizationHtml } = require("../../src/localization/template");

test("localization view keeps catalog data out of initial HTML and uses text nodes", () => {
  const html = localizationHtml({}, "fixed-nonce");
  assert.match(html, /Content-Security-Policy/);
  assert.match(html, /script-src 'nonce-fixed-nonce'/);
  assert.match(html, /node\.textContent = text/);
  assert.match(html, /aria-label="Translation coverage"/);
  assert.match(html, /type: 'search'/);
  assert.match(html, /openLocation/);
});

test("localization explorer discards stale replies and refreshes after source edits", async () => {
  let show, receive, onEdit, onSave, onReveal;
  const posted = [];
  const requests = [];
  const panel = {
    visible: true,
    webview: {
      postMessage(message) { posted.push(message); },
      onDidReceiveMessage(handler) { receive = handler; },
    },
    onDidChangeViewState(handler) { onReveal = handler; },
    onDidDispose() {}, reveal() {}, dispose() {},
  };
  const vscode = {
    ViewColumn: { Beside: 2 },
    Disposable: { from: (...items) => ({ dispose() { items.forEach(item => item.dispose?.()); } }) },
    commands: { registerCommand(_name, handler) { show = handler; return { dispose() {} }; } },
    window: {
      activeTextEditor: { document: { uri: { path: "/game/src/main.tua", toString: () => "file:///game/src/main.tua" } } },
      createWebviewPanel() { return panel; },
    },
    workspace: {
      onDidChangeTextDocument(handler) { onEdit = handler; return { dispose() {} }; },
      onDidSaveTextDocument(handler) { onSave = handler; return { dispose() {} }; },
    },
  };
  const originalLoad = Module._load;
  Module._load = function (name, ...args) {
    return name === "vscode" ? vscode : originalLoad.call(this, name, ...args);
  };
  let registerLocalizationExplorer;
  try {
    ({ registerLocalizationExplorer } = require("../../src/localization"));
  } finally {
    Module._load = originalLoad;
  }
  const feature = registerLocalizationExplorer({ getClient: () => ({
    sendRequest(method, params) {
      assert.equal(method, "tua/localizationCatalog");
      return new Promise((resolve, reject) => requests.push({ params, resolve, reject }));
    },
  }) });
  const page = key => ({ enabled: true, locales: ["en"], total: 1, offset: 0, rows: [{ key, cells: [] }] });
  const tick = () => new Promise(resolve => setImmediate(resolve));
  try {
    const opened = show();
    assert.equal(requests.length, 1);
    const searched = receive({ type: "search", value: "menu.play" });
    assert.deepEqual(requests[1].params, { search: "menu.play", offset: 0, uri: "file:///game/src/main.tua" });
    requests[1].resolve(page("menu.play"));
    await searched;
    requests[0].resolve(page("stale"));
    await opened;
    assert.deepEqual(posted.filter(message => message.type === "page").map(message => message.page.rows[0].key), ["menu.play"]);

    onEdit({ document: { uri: { path: "/game/src/locales/en.lua" } } });
    await new Promise(resolve => setTimeout(resolve, 450));
    assert.equal(requests.length, 3, "catalog edit refreshes the open explorer");
    requests[2].resolve(page("menu.updated"));
    await tick();
    assert.equal(posted.at(-1).page.rows[0].key, "menu.updated");

    panel.visible = false;
    onSave({ uri: { path: "/game/src/locales/en.lua" } });
    await new Promise(resolve => setTimeout(resolve, 450));
    assert.equal(requests.length, 3, "hidden explorers defer refresh work");
    panel.visible = true;
    onReveal();
    assert.equal(requests.length, 4, "revealing the explorer reloads current catalog data");
    requests[3].reject(new Error("catalog unavailable"));
    await tick();
    assert.equal(posted.at(-1).type, "error");
    assert.match(posted.at(-1).message, /catalog unavailable/);
    receive({ type: "refresh" });
    requests[4].resolve(page("menu.recovered"));
    await tick();
    assert.equal(posted.at(-1).page.rows[0].key, "menu.recovered");
  } finally {
    feature.dispose();
  }
});
