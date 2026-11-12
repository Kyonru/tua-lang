"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { createProceduralRunner } = require("../src/procedural/runner");

const tua = process.env.TUA_BIN || path.resolve(__dirname, "../../../target/debug/tua");
const love = process.env.LOVE_BIN || (process.platform === "darwin" ? "/Applications/love.app/Contents/MacOS/love" : "love");

async function main() {
  assert(fs.existsSync(tua), `Build Tua first or set TUA_BIN: ${tua}`);
  const runner = createProceduralRunner({ getServerCommand: () => tua, getLoveCommand: () => love });
  const before = new Set(fs.readdirSync(os.tmpdir()).filter(name => name.startsWith("tua-rot-preview-")));
  try {
    const options = { width: 15, height: 11, seed: 817, algorithm: "sidewinder", radius: 5 };
    const first = await runner.generate(options);
    const repeat = await runner.generate(options);
    assert.deepEqual(repeat, first, "same seed must reproduce the same map, route, and sight");
    assert.equal(first.map.length, 11);
    assert.equal(first.map[0], "0".repeat(15));
    assert.deepEqual(first.path[0], { x: 2, y: 2 });
    assert.deepEqual(first.path.at(-1), { x: 14, y: 10 });
    for (let i = 1; i < first.path.length; i++) {
      const from = first.path[i - 1], to = first.path[i];
      assert.equal(Math.abs(from.x - to.x) + Math.abs(from.y - to.y), 1, "route moves four-way");
    }
    assert.equal(first.visible[1][1], "1", "start is visible");
    const otherSeed = await runner.generate({ ...options, seed: 818 });
    assert.notDeepEqual(otherSeed.map, first.map, "a new seed changes the maze");
    const arena = await runner.generate({ ...options, algorithm: "arena" });
    assert.equal(arena.path.length, 0, "arena is a fully walled canvas until cells are opened");
    assert(arena.map.every(row => row === "0".repeat(options.width)));
    assert.notDeepEqual(arena.map, first.map);
    const binary = await runner.generate({ ...options, algorithm: "binaryTree" });
    assert(binary.path.length > 0, "binary tree has a route");
    console.log("Procedural preview: deterministic map, three algorithms, route, sight, and generated runtime passed.");
  } finally {
    runner.stop();
    const left = fs.readdirSync(os.tmpdir()).filter(name => name.startsWith("tua-rot-preview-") && !before.has(name));
    assert.deepEqual(left, [], "isolated sample directories must be removed");
  }
}

main().catch(error => { console.error(error); process.exitCode = 1; });
