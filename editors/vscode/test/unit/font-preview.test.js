"use strict";

const assert = require("node:assert/strict");
const Module = require("node:module");
const test = require("node:test");

let panel;
const vscode = {
  Uri: { file: (fsPath) => ({ fsPath, scheme: "file" }) },
  ViewColumn: { Active: 1 },
  window: {
    createWebviewPanel(viewType, title, column, options) {
      panel = {
        viewType, title, column, options,
        webview: {
          cspSource: "vscode-resource:",
          asWebviewUri: (uri) => ({ toString: () => `vscode-resource:${uri.fsPath}` }),
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
  const uri = { fsPath: "/game/assets/ui.ttf", scheme: "file" };
  assert(isFontFile(uri));
  assert(!isFontFile({ fsPath: "/game/assets/ui.png", scheme: "file" }));
  assert(!isFontFile({ fsPath: "/game/assets/ui.ttf", scheme: "https" }));

  showFontPreview(uri);
  assert.equal(panel.viewType, "tuaFontPreview");
  assert.equal(panel.title, "Font: ui.ttf");
  assert.equal(panel.options.localResourceRoots[0].fsPath, "/game/assets");
  assert.match(panel.webview.html, /font-src vscode-resource:/);
  assert.match(panel.webview.html, /src: url\("vscode-resource:\/game\/assets\/ui.ttf"\)/);
  assert.match(panel.webview.html, /18 px/);
  assert.match(panel.webview.html, /72 px/);
  assert.match(panel.webview.html, /0123456789/);
  assert.match(panel.webview.html, /Font could not be rendered/);
  assert.match(panel.webview.html, /Check final text in LÖVE/);
});

test("font preview escapes filename and resource URL in HTML and CSS", () => {
  showFontPreview({ fsPath: '/game/assets/<img src=x onerror="x">.otf', scheme: "file" });
  assert.doesNotMatch(panel.webview.html, /<img src=x/);
  assert.match(panel.webview.html, /&lt;img src=x onerror=&quot;x&quot;&gt;\.otf/);
  assert.doesNotMatch(panel.webview.html, /url\("vscode-resource:\/game\/assets\/<img/);
});
