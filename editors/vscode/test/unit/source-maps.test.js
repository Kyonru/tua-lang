"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const Module = require("node:module");
const originalLoad = Module._load;
Module._load = function (id, ...args) { return id === "vscode" ? {} : originalLoad.call(this, id, ...args); };
const { createSourceMapLookup, lookupSourceLocation, remapTracebackLine } = require("../../src/editor/source-maps");
Module._load = originalLoad;
test("unquoted traceback paths can contain spaces", () => {
  const maps = [{ generated: "build/nested dir/crash.lua", sourcePath: "src/nested dir/crash.tua", lineMappings: [{ generatedLine: 17, sourceLine: 7 }] }];
  assert.equal(remapTracebackLine("Error: build/nested dir/crash.lua:17: boom", maps), "src/nested dir/crash.tua:7 (generated build/nested dir/crash.lua:17)");
});

test("source lookup skips line scans for unrelated generated files", () => {
  const unrelated = { generated: "build/other.lua", sourcePath: "src/other.tua",
    lineMappings: { find() { assert.fail("unrelated line mappings must not be scanned"); } } };
  const matching = { generated: "build/main.lua", sourcePath: "src/main.tua",
    lineMappings: [{ generatedLine: 17, sourceLine: 7 }] };
  assert.deepEqual(lookupSourceLocation([unrelated, matching], "build/main.lua", 17), {
    generatedLine: 17, generatedPath: "build/main.lua", score: "build/main.lua".length + 10000,
    sourceLine: 7, sourcePath: "src/main.tua",
  });
});

test("indexed profile lookup preserves path ranking and ambiguous matches", () => {
  const maps = [
    { generated: "main.lua", sourcePath: "src/fallback.tua", lineMappings: [{ generatedLine: 5, sourceLine: 1 }] },
    { generated: "build/main.lua", sourcePath: "src/main.tua", lineMappings: [{ generatedLine: 5, sourceLine: 7 }] },
    { generated: "build/main.lua", sourcePath: "src/duplicate.tua", lineMappings: [{ generatedLine: 6, sourceLine: 8 }] },
    { generated: "build/other.lua", sourcePath: "src/other.tua", lineMappings: [{ generatedLine: 5, sourceLine: 9 }] },
    { generated: "build/shared.lua", sourcePath: "src/one.tua", lineMappings: [{ generatedLine: 5, sourceLine: 1 }] },
    { generated: "build/shared.lua", sourcePath: "src/two.tua", lineMappings: [{ generatedLine: 5, sourceLine: 2 }] },
  ];
  const lookup = createSourceMapLookup(maps);
  for (const [path, line] of [["build/main.lua", 5], ["build/main.lua", 6], ["other.lua", 5], ["build/shared.lua", 5], ["missing.lua", 1]]) {
    assert.deepEqual(lookup(path, line), lookupSourceLocation(maps, path, line));
  }
  assert.equal(lookup("build/main.lua", 5)?.sourcePath, "src/main.tua");
  assert.equal(lookup("build/shared.lua", 5), undefined);
});
