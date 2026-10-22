"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");

const { positionAfter } = require("../../scripts/lsp-profile-common");

test("positionAfter converts the end of a multiline needle", () => {
  const source = [
    "local value = call(",
    '  "first",',
    "  second",
    ")",
  ].join("\n");

  assert.deepEqual(positionAfter(source, 'call(\n  "first",'), {
    line: 1,
    character: 10,
  });
});

test("positionAfter rejects a missing needle", () => {
  assert.throws(
    () => positionAfter("local value = 1", "missing"),
    /needle not found in source: missing/,
  );
});
