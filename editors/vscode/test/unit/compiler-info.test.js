"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { parseCompilerInfo, readCompilerInfo, compilerInfoLines } = require("../../src/setup/compiler-info");
const record = { schemaVersion: 1, version: "0.11.0", target: "aarch64-apple-darwin", profile: "release", revision: "a".repeat(40), dirty: false, executableSha256: "b".repeat(64) };
test("compiler provenance retains exact identity and tolerates unavailable metadata", () => {
  assert.deepEqual(parseCompilerInfo(JSON.stringify(record)), record);
  const partial = parseCompilerInfo(JSON.stringify({...record, revision: null, dirty: null, executableSha256: "invalid"}));
  assert.equal(partial.revision, null);
  assert.equal(partial.executableSha256, null);
  assert.ok(compilerInfoLines(partial).includes("Compiler build dirty: unknown"));
  assert.equal(parseCompilerInfo("tua 0.10.0\n"), null);
  assert.equal(parseCompilerInfo("null"), null);
  assert.equal(parseCompilerInfo(JSON.stringify({...record, schemaVersion: 2})), null);
});
test("optional probe bounds output/time and gracefully accepts legacy compilers or failures", async () => {
  const fake = (error, text) => (command, args, options, callback) => {
    assert.equal(command, "/compiler path/tua");
    assert.deepEqual(args, ["--version", "--json"]);
    assert.equal(options.timeout, 3000);
    assert.equal(options.maxBuffer, 16384);
    callback(error, text);
  };
  assert.deepEqual(await readCompilerInfo("/compiler path/tua", "/workspace", fake(null, JSON.stringify(record))), record);
  assert.equal(await readCompilerInfo("/compiler path/tua", undefined, fake(null, "tua 0.10.0")), null);
  assert.equal(await readCompilerInfo("/compiler path/tua", undefined, fake(new Error("timeout"), "")), null);
});
