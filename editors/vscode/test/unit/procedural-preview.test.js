"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const { DEFAULTS, parsePreview, previewSource, validateOptions } = require("../../src/procedural/runner");

test("procedural sample accepts only bounded literal settings", () => {
  assert.deepEqual(validateOptions({}), DEFAULTS);
  assert.throws(() => validateOptions({ algorithm: 'sidewinder"); os.execute("oops") --' }), /algorithm/u);
  assert.throws(() => validateOptions({ width: 12 }), /width/u);
  assert.throws(() => validateOptions({ seed: Number.POSITIVE_INFINITY }), /Seed/u);
  assert.throws(() => validateOptions({ radius: 65 }), /radius/u);
  const source = previewSource({ width: 11, height: 11, seed: 42, radius: 4, algorithm: "binaryTree" });
  assert.match(source, /Rot\.Map\.generate\(11, 11, "binaryTree", rng\)/u);
  assert.match(source, /Rot\.Path\.astar\(maze, 2, 2, 10, 10\)/u);
  assert.match(source, /love\.event\.quit\(0\)/u);
  assert.doesNotMatch(source, /require\s*\(/u);
});

test("procedural response validates rows and route against the generated map", () => {
  const options = { width: 11, height: 11, seed: 1, radius: 4, algorithm: "arena" };
  const row = "01111111110";
  const wall = "00000000000";
  const body = Array.from({ length: 11 }, (_, y) => `M:${y === 0 || y === 10 ? wall : row}\nV:${y === 0 || y === 10 ? wall : row}`).join("\n");
  const output = `LÖVE startup text\nTUA_ROT_PREVIEW_V1\n${body}\nP:2,2;3,2;10,10\nTUA_ROT_PREVIEW_END\n`;
  const parsed = parsePreview(output, options);
  assert.equal(parsed.map.length, 11);
  assert.equal(parsed.visible[1][1], "1");
  assert.deepEqual(parsed.path.at(-1), { x: 10, y: 10 });
  assert.throws(() => parsePreview(output.replace("P:2,2", "P:1,1"), options), /walkable/u);
  assert.throws(() => parsePreview(output.replace("TUA_ROT_PREVIEW_END", ""), options), /complete/u);
  assert.throws(() => parsePreview(output.replace("M:011", "M:021"), options), /row/u);
});
