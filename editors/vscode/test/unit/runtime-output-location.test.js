"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const Module = require("node:module");
const originalLoad = Module._load;
Module._load = function (id, ...args) { return id === "vscode" ? {} : originalLoad.call(this, id, ...args); };
const { runtimeOutputLocation } = require("../../src/workspace/runtime-output-location");
Module._load = originalLoad;

const maps = [
  { generated: "build/main.lua", sourcePath: "/game/src/main.tua",
    lineMappings: [{ generatedLine: 17, sourceLine: 9 }] },
  { generated: "build/other.lua", sourcePath: "/game/src/other.tua",
    lineMappings: [{ generatedLine: 4, sourceLine: 3 }] },
];

test("run and debug output link generated errors to the first Tua frame", () => {
  assert.deepEqual(runtimeOutputLocation("Error: build/main.lua:17: nil value", "/game", maps),
    { path: "/game/src/main.tua", line: 8, character: 0 });
  assert.deepEqual(runtimeOutputLocation("build/main.lua:17: bad (build/other.lua:4)", "/game", maps),
    { path: "/game/src/main.tua", line: 8, character: 0 });
  assert.deepEqual(runtimeOutputLocation("nil value", "/game", maps, "build/main.lua", 17),
    { path: "/game/src/main.tua", line: 8, character: 0 });
});

test("explicit Tua source and unmapped lines remain usable", () => {
  assert.deepEqual(runtimeOutputLocation("nil value", "/game", maps, "/game/src/main.tua", 9),
    { path: "/game/src/main.tua", line: 8, character: 0 });
  assert.deepEqual(runtimeOutputLocation("src/other.tua:3: failed", "/game", []),
    { path: "/game/src/other.tua", line: 2, character: 0 });
  assert.equal(runtimeOutputLocation("ordinary output", "/game", maps), undefined);
});

test("debug adapter source paths use the workspace spelling", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-debug-source-"));
  try {
    const real = path.join(root, "real");
    const alias = path.join(root, "alias");
    fs.mkdirSync(real);
    fs.symlinkSync(real, alias, "dir");
    assert.deepEqual(runtimeOutputLocation("error", alias, [], `${real}/main.tua`, 5),
      { path: `${alias}/main.tua`, line: 4, character: 0 });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
