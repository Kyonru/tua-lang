"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const { EventEmitter } = require("node:events");
const childProcess = require("node:child_process");
const { checkServerCommand } = require("../../src/health/server-probe");

function fixture(t) {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const child = new EventEmitter();
  child.stdout = new EventEmitter();
  child.stderr = new EventEmitter();
  child.kill = t.mock.fn();
  t.mock.method(childProcess, "spawn", () => child);
  return child;
}

test("server probe accepts a valid cold launch after the old five-second deadline", async t => {
  const child = fixture(t);
  const probe = checkServerCommand("/bundled/tua", "/project");
  t.mock.timers.tick(13000);
  assert.equal(child.kill.mock.callCount(), 0);
  child.stdout.emit("data", Buffer.from("tua 0.11.0\n"));
  child.emit("exit", 0);
  assert.deepEqual(await probe, { ok: true, version: "tua 0.11.0" });
  t.mock.timers.tick(30000);
  assert.equal(child.kill.mock.callCount(), 0, "completion clears the deadline");
});

test("server probe kills an unresponsive process and ignores its late exit", async t => {
  const child = fixture(t);
  const probe = checkServerCommand("/bundled/tua", "/project");
  t.mock.timers.tick(30000);
  const result = await probe;
  assert.equal(child.kill.mock.callCount(), 1);
  assert.equal(result.ok, false);
  assert.match(result.error.message, /timed out/);
  child.emit("exit", 0);
  assert.equal(await probe, result);
});

test("server probe reports launch errors without waiting for its deadline", async t => {
  const child = fixture(t);
  const probe = checkServerCommand("/missing/tua", "/project");
  const error = new Error("ENOENT");
  child.emit("error", error);
  assert.deepEqual(await probe, { ok: false, error });
  t.mock.timers.tick(30000);
  assert.equal(child.kill.mock.callCount(), 0);
});
