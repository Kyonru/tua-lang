"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");
const vm = require("node:vm");

const directory = path.resolve(__dirname, "../../src/editor");
const moduleFixture = { exports: {} };
vm.runInNewContext(fs.readFileSync(path.join(directory, "unit-test-scaffold.js"), "utf8"), {
  module: moduleFixture,
  require: (id) => {
    if (id === "vscode" || id === "./project-layout") return {};
    return require(id.startsWith(".") ? path.resolve(directory, id) : id);
  },
});
const { scaffoldSource } = moduleFixture.exports;

test("untouched scaffold stays pending without loading runtime-dependent modules", (t) => {
  const busted = process.env.TUA_BUSTED || "busted";
  const available = spawnSync(busted, ["--version"], { encoding: "utf8", timeout: 10000 });
  if (available.error?.code === "ENOENT") return t.skip("Busted is not installed");
  assert.equal(available.status, 0, available.stderr || String(available.error));
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-scaffold-test-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.mkdirSync(path.join(root, "game"));
  fs.writeFileSync(path.join(root, "game/range.lua"), 'error("runtime-dependent module was loaded")\n');
  const spec = path.join(root, "range_spec.lua");
  const source = scaffoldSource("game.range", "draw");
  fs.writeFileSync(spec, source);
  const run = () => spawnSync(busted, [spec], { cwd: root, encoding: "utf8", timeout: 10000 });
  let result = run();
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /0 successes.*0 failures.*0 errors.*1 pending/);
  assert(!result.stdout.includes("runtime-dependent module was loaded"));

  // Loading a module is an explicit author action; real failures must remain visible.
  fs.writeFileSync(spec, source.replace("-- local subject = require", "local subject = require"));
  result = run();
  assert.notEqual(result.status, 0);
  assert.match(result.stdout + result.stderr, /runtime-dependent module was loaded/);
});
