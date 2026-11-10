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
const context = { module: { exports: {} }, require: (name) => name === "vscode" ? {} : require(name) };
vm.runInNewContext(outputFiles[0].text, context);
const { behaviorTreePreviewHtml } = context.module.exports;

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
});
