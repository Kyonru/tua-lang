"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { RuntimeOrigins, bindingAtLine } = require("../../src/inspector/origins");

test("runtime binding names cover typed stores, pools, signals, states, worlds and multiline arguments", () => {
  for (const [kind, helper] of [["stores", "store"], ["pools", "objectPool"], ["signals", "signal"], ["states", "stateMachine"], ["ecs", "ecs.world"]]) {
    assert.deepEqual(bindingAtLine(`local player: any = ${helper}({\n  score = 0\n})\n`, 3, kind), { name: "player", line: 1, character: 6 });
    assert.equal(bindingAtLine(`shared.player = ${helper}({})`, 1, kind).name, "shared.player");
  }
  assert.equal(bindingAtLine("local events = SignalBus.new()", 1, "signals").name, "events");
  assert.equal(bindingAtLine("local bullets = makePool({})", 1, "pools", "makePool").name, "bullets");
});

test("runtime names do not guess through comments, strings, nested constructors or multi-assignments", () => {
  for (const source of ["-- local player = store({})", 'print("local player = store({})")', "local a, b = store({}), store({})", "return store({})", "local parent = store({ child = store({}) })", "local result = factory(store({}))"]) {
    assert.equal(bindingAtLine(source, 1, "stores"), undefined, source);
  }
  assert.equal(bindingAtLine('--[[\nlocal fake = store({})\n]]\nlocal real = store({ text = "})" })', 4, "stores").name, "real");
});

test("origins map creation lines to Tua names, reject external paths, and reset on reconnect", t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-origins-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const outputDirectory = path.join(root, "build"); fs.mkdirSync(outputDirectory);
  const sourcePath = path.join(root, "state.tua");
  fs.writeFileSync(sourcePath, "-- shared state\nlocal player = store({\n score = 0\n})\n");
  fs.writeFileSync(path.join(outputDirectory, "state.lua"), "local __tua_store = require('tuastore')\nlocal player = __tua_store({})\n");
  const origins = new RuntimeOrigins(() => ({ sourcePath, lineMappings: [{ generatedLine: 2, sourceLine: 2 }] }));
  const entry = { outputDirectory, session: { cwd: root }, connectionGeneration: 1 };
  const object = { id: 1, kind: "stores", source: "@state.lua", line: 2 };
  assert.deepEqual(origins.describe(entry, object), { path: sourcePath, line: 2, character: 6, name: "player", generated: false, file: "state.tua" });
  assert.equal(origins.describe(entry, { ...object, source: "@../../private.lua" }), undefined);
  assert.equal(origins.describe(entry, { ...object, source: "=dynamic chunk" }), undefined);
  fs.writeFileSync(sourcePath, "-- changed source\nlocal session = store({})\n");
  entry.connectionGeneration++;
  assert.equal(origins.describe(entry, object).name, "session");
  const fallback = new RuntimeOrigins().describe(entry, object);
  assert.equal(fallback.path, path.join(outputDirectory, "state.lua"));
  assert.equal(fallback.generated, true);
  assert.equal(fallback.name, "player");
});
