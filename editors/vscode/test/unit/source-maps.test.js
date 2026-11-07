"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const Module = require("node:module");
const originalLoad = Module._load;
Module._load = function (id, ...args) { return id === "vscode" ? {} : originalLoad.call(this, id, ...args); };
const { remapTracebackLine } = require("../../src/editor/source-maps");
Module._load = originalLoad;
test("unquoted traceback paths can contain spaces", () => {
  const maps = [{ generated: "build/nested dir/crash.lua", sourcePath: "src/nested dir/crash.tua", lineMappings: [{ generatedLine: 17, sourceLine: 7 }] }];
  assert.equal(remapTracebackLine("Error: build/nested dir/crash.lua:17: boom", maps), "src/nested dir/crash.tua:7 (generated build/nested dir/crash.lua:17)");
});
