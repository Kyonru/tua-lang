"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { probePosition, replaceOnce } = require("./lsp-editor-profile");
test("editor probes reject absent and ambiguous anchors", () => {
  assert.throws(() => probePosition("self.x\nself.x", "self."), /ambiguous/);
  assert.throws(() => probePosition("self.x", "missing"), /absent/);
});
test("editor probes use actual multiline UTF-16 positions", () => {
  assert.deepEqual(probePosition("-- first\r\nlocal emoji = '😀'; api.field", "local emoji = '😀'; api."), { line: 1, character: 24 });
});
test("overlay edits preserve replacement text literally", () => {
  assert.equal(replaceOnce("local value = 1", "value = 1", "value = '$&'"), "local value = '$&'");
});
