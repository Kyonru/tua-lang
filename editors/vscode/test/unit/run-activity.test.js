"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const { ActivityStore, devPhase } = require("../../src/workspace/activity-store");

test("hot reload distinguishes a request, success, and the last good build", () => {
  const store = new ActivityStore();
  const session = store.begin({ kind: "dev", cwd: "/game" });
  store.line(session, "dev: started love build");
  assert.equal(session.phase, "running");
  store.line(session, "dev: keeping the running game on its last good build");
  assert.equal(session.label, "Build failed — running previous code");
  store.line(session, "dev: sent reload 2 to the game");
  assert.equal(session.phase, "reloading");
  store.line(session, "[tua dev] reloaded player");
  assert.equal(session.phase, "reloaded");
  assert.deepEqual(session.modules, ["player"]);
  assert.equal(devPhase("game says reloaded"), undefined);
  assert.equal(devPhase("[tua dev] hot reload is watching for rebuilds"), undefined);
  assert.equal(devPhase("[tua dev] enemy.lua:8: unexpected symbol near end"), "reloadFailed");
  assert.equal(devPhase("[tua dev] main.lua changed; restarting"), "reloading");
  store.line(session, "[tua dev] reloaded player, combat");
  assert.deepEqual(session.modules, ["player", "combat"]);
});

test("sessions isolate late events, preserve active runs, and bound history", () => {
  const store = new ActivityStore();
  const active = store.begin({ kind: "run", cwd: "/first" });
  for (let i = 0; i < 30; i++) store.finish(store.begin({ kind: "run", cwd: "/other" }), 0);
  assert.equal(store.sessions.length, 21);
  assert(store.sessions.includes(active));
  store.update(active, "running");
  store.finish(active, 1);
  assert.equal(active.label, "Exited with errors");
  store.update(active, "running");
  assert.equal(active.active, false);
  assert.equal(active.label, "Exited with errors");
});
