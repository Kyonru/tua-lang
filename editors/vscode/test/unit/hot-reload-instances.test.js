"use strict";

const assert = require("assert/strict");
const Module = require("module");
const os = require("os");
const path = require("path");
const test = require("node:test");

class EventEmitter {
  constructor() {
    this.listeners = new Set();
    this.event = (listener) => {
      this.listeners.add(listener);
      return { dispose: () => this.listeners.delete(listener) };
    };
  }

  fire(value) {
    for (const listener of this.listeners) {
      listener(value);
    }
  }

  dispose() {
    this.listeners.clear();
  }
}

const vscodeMock = {
  EventEmitter,
  Disposable: { from: (...items) => ({ dispose: () => items.forEach((item) => item.dispose?.()) }) },
  commands: { registerCommand: () => ({ dispose() {} }) },
  workspace: { getConfiguration: () => ({ get: (_key, fallback) => fallback }) },
  window: {},
};

const originalLoad = Module._load;
Module._load = function load(request, parent, isMain) {
  if (request === "vscode") {
    return vscodeMock;
  }
  return originalLoad.call(this, request, parent, isMain);
};
const { HotReloadInstancesTerminal } = require("../../src/love/hot-reload-instances");
Module._load = originalLoad;


const { ActivityStore } = require("../../src/workspace/activity-store");

test("players start only once after completed output; reload and failed-build events reach each live player", async () => {
  const activity = new ActivityStore();
  const terminal = new HotReloadInstancesTerminal({ cwd: os.tmpdir(), outputDirectory: os.tmpdir(), activity, kind: "dev", title: "Players" });
  let launches = 0;
  terminal.runInstances = async () => { launches++; };
  terminal.killActiveProcess = () => {};
  terminal.writeProcessLine("initial build: built 1 file(s)");
  assert.equal(launches, 0);
  terminal.writeProcessLine("dev: ready /build");
  terminal.writeProcessLine("dev: ready /build");
  assert.equal(launches, 1);
  let maps = 0;
  const child = { session: activity.begin({ kind: "dev" }), loadSourceMaps: () => { maps++; return []; } };
  terminal.children = [child, { finished: true }];
  terminal.writeProcessLine("dev: sent reload 2 to the game");
  assert.equal(maps, 1);
  assert.equal(child.session.phase, "reloading");
  terminal.writeProcessLine("dev: keeping the running game on its last good build");
  assert.equal(child.session.phase, "previous");
  terminal.finish(0);
  activity.dispose();
});

test("watcher exit stops all players and reports failure", async () => {
  const terminal = new HotReloadInstancesTerminal({ cwd: os.tmpdir(), outputDirectory: os.tmpdir(), tuaCommand: "tua" });
  let closes = 0;
  terminal.children = [{ close: () => closes++ }, { close: () => closes++ }];
  terminal.runProcess = async (command, args) => { assert.deepEqual(args, ["dev", "--no-launch"]); return 3; };
  let exit;
  terminal.onDidClose(code => { exit = code; });
  await terminal.run();
  assert.equal(closes, 2);
  assert.equal(exit, 3);
});
