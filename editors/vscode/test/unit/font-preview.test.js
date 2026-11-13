"use strict";

const assert = require("node:assert/strict");
const Module = require("node:module");
const test = require("node:test");

let panel;
let panelCount = 0;
let revealCount = 0;
const vscode = {
  Uri: { file: (fsPath) => ({ fsPath, scheme: "file" }) },
  ViewColumn: { Active: 1 },
  window: {
    createWebviewPanel(viewType, title, column, options) {
      panelCount += 1;
      panel = {
        viewType, title, column, options,
        reveal() { revealCount += 1; },
        onDidDispose(callback) { this.didDispose = callback; },
        webview: {
          cspSource: "vscode-resource:",
          asWebviewUri: (uri) => ({
            with: ({ query }) => ({ toString: () => `vscode-resource:${uri.fsPath}?${query}` }),
            toString: () => `vscode-resource:${uri.fsPath}`,
          }),
        },
      };
      return panel;
    },
  },
};
const originalLoad = Module._load;
Module._load = function (id, parent, main) {
  return id === "vscode" ? vscode : originalLoad.call(this, id, parent, main);
};
const { isFontFile, showFontPreview } = require("../../src/love/assets/font-preview");
Module._load = originalLoad;

test("font preview uses a local font resource and gives size and glyph samples", () => {
  const uri = { fsPath: "/game/assets/ui.ttf", scheme: "file", toString: () => "file:///game/assets/ui.ttf" };
  assert(isFontFile(uri));
  assert(!isFontFile({ fsPath: "/game/assets/ui.png", scheme: "file" }));
  assert(!isFontFile({ fsPath: "/game/assets/ui.ttf", scheme: "https" }));

  showFontPreview(uri);
  assert.equal(panel.viewType, "tuaFontPreview");
  assert.equal(panel.title, "Font: ui.ttf");
  assert.equal(panel.options.localResourceRoots[0].fsPath, "/game/assets");
  assert.match(panel.webview.html, /font-src vscode-resource:/);
  assert.match(panel.webview.html, /src: url\("vscode-resource:\/game\/assets\/ui.ttf\?\d+"\)/);
  assert.match(panel.webview.html, /18 px/);
  assert.match(panel.webview.html, /72 px/);
  assert.match(panel.webview.html, /Try text from your game/);
  assert.match(panel.webview.html, /sampleInput\.addEventListener\('input', updateSample\)/);
  assert.match(panel.webview.html, /0123456789/);
  assert.match(panel.webview.html, /Font could not be rendered/);
  assert.match(panel.webview.html, /Check final text in LÖVE/);
});

test("font preview escapes filename and resource URL in HTML and CSS", () => {
  showFontPreview({ fsPath: '/game/assets/<img src=x onerror="x">.otf', scheme: "file", toString: () => "file:///game/assets/escaped.otf" });
  assert.doesNotMatch(panel.webview.html, /<img src=x/);
  assert.match(panel.webview.html, /&lt;img src=x onerror=&quot;x&quot;&gt;\.otf/);
  assert.doesNotMatch(panel.webview.html, /url\("vscode-resource:\/game\/assets\/<img/);
});

test("opening the same font reveals its existing preview and disposal allows reopening", () => {
  const uri = { fsPath: "/game/assets/reused.otf", scheme: "file", toString: () => "file:///game/assets/reused.otf" };
  showFontPreview(uri);
  const originalPanel = panel;
  const originalHtml = panel.webview.html;
  const opened = panelCount;
  const revealed = revealCount;
  showFontPreview(uri);
  assert.equal(panelCount, opened);
  assert.equal(revealCount, revealed + 1);
  assert.notEqual(panel.webview.html, originalHtml, "reopening a font should reload its content");
  const resourceUrl = (html) => html.match(/vscode-resource:\/game\/assets\/reused\.otf\?\d+/)?.[0];
  assert(resourceUrl(originalHtml));
  assert(resourceUrl(panel.webview.html));
  assert.notEqual(resourceUrl(panel.webview.html), resourceUrl(originalHtml));
  originalPanel.didDispose();
  showFontPreview(uri);
  assert.equal(panelCount, opened + 1);
});
