"use strict";

const assert = require("assert/strict");
const test = require("node:test");
const { generateLargeTableSource, maxOptional } = require("./profile-large-tables");

test("large-table fixtures have deterministic wide and deep boundary probes", () => {
  const fixture = generateLargeTableSource(1_000, 16);

  assert.equal(fixture.firstField, "field_000");
  assert.equal(fixture.lastField, "field_999");
  assert.match(fixture.checked, /field_000: number/);
  assert.match(fixture.checked, /field_999 = 999/);
  assert.match(fixture.checked, /level_00: \{ level_01:/);
  assert.match(fixture.checked, /level_15: number/);
  assert.match(fixture.editor, /local completion_probe = wide\.field_000/);
  assert.match(
    fixture.editor,
    /return \{ wide = wide, deep = deep, completion_probe = completion_probe \}\n$/,
  );
  assert.ok(
    fixture.editor.indexOf("local completion_probe") < fixture.editor.indexOf("return "),
    "completion probe must appear before the final return statement",
  );
});

test("memory samples retain the available peak", () => {
  assert.equal(maxOptional(null, 100), 100);
  assert.equal(maxOptional(200, null), 200);
  assert.equal(maxOptional(200, 300), 300);
});
