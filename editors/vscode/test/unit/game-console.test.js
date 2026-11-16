"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const { ConsoleBuffer, sourceLocation } = require("../../src/workspace/console-buffer");
const session = { id: "one", cwd: "/game" };

test("console collapses repeats within a session and filters module and severity", () => {
  const buffer = new ConsoleBuffer(3);
  buffer.append(session, "\x1b[31m[warn] [combat] low health\x1b[0m");
  buffer.append(session, "[warn] [combat] low health");
  buffer.append({ ...session, id: "two" }, "[warn] [combat] low health");
  assert.equal(buffer.entries.length, 2);
  assert.equal(buffer.entries[0].count, 2);
  assert.equal(buffer.filtered({ session: "one", severity: "warn", module: "combat" }).length, 1);
  assert.equal(buffer.filtered({ severity: "error" }).length, 0);
});

test("console bounds memory and keeps changed source mappings distinct", () => {
  const buffer = new ConsoleBuffer(2);
  buffer.append(session, "error", { location: { path: "/game/a.tua", line: 1 } });
  buffer.append(session, "error", { location: { path: "/game/a.tua", line: 2 } });
  assert.equal(buffer.entries.length, 2);
  buffer.append(session, "x".repeat(20000));
  assert.equal(buffer.entries.length, 2);
  assert.equal(buffer.entries[1].text.length, 16000);
  assert.equal(buffer.entries[0].location.line, 2);
  buffer.clear();
  assert.equal(buffer.entries.length, 0);
});

test("a mapped traceback frame on stderr is an error even without an error keyword", () => {
  const buffer = new ConsoleBuffer();
  const entry = buffer.append(session, "build/main.lua:17: attempt to index nil", {
    stream: "stderr", location: { path: "/game/src/main.tua", line: 8, character: 0 },
  });
  assert.equal(entry.severity, "error");
  assert.deepEqual(entry.location, { path: "/game/src/main.tua", line: 8, character: 0 });
});

test("mapped errors preserve spaces and convert source coordinates to editor positions", () => {
  assert.deepEqual(sourceLocation("/my game/src/player.tua:42 (generated build/player.lua:80)", "/game"), { path: "/my game/src/player.tua", line: 41, character: 0 });
  assert.deepEqual(sourceLocation("error: src/player.tua:8:3", "/game"), { path: "/game/src/player.tua", line: 7, character: 2 });
  assert.equal(sourceLocation("ordinary output", "/game"), undefined);
});

test("absolute traceback paths retain the workspace spelling through a symlink", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-console-source-"));
  try {
    const real = path.join(root, "real game");
    const alias = path.join(root, "game");
    fs.mkdirSync(path.join(real, "src"), { recursive: true });
    fs.symlinkSync(real, alias, "dir");
    assert.deepEqual(sourceLocation(`${real}/src/main.tua:7:2: error`, alias),
      { path: `${alias}/src/main.tua`, line: 6, character: 1 });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
