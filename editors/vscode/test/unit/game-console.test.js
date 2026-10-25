"use strict";
const assert = require("node:assert/strict");
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

test("mapped errors preserve spaces and convert source coordinates to editor positions", () => {
  assert.deepEqual(sourceLocation("/my game/src/player.tua:42 (generated build/player.lua:80)", "/game"), { path: "/my game/src/player.tua", line: 41, character: 0 });
  assert.deepEqual(sourceLocation("error: src/player.tua:8:3", "/game"), { path: "/game/src/player.tua", line: 7, character: 2 });
  assert.equal(sourceLocation("ordinary output", "/game"), undefined);
});
