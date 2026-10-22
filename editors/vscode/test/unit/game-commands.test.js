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
const { PACKAGE_TARGETS, TuaCommandTerminal } = require("../../src/love/game-commands");
Module._load = originalLoad;

function runTerminal(script, outputDirectory) {
  const lines = [];
  const terminal = new TuaCommandTerminal({
    cwd: os.tmpdir(),
    outputDirectory,
    tuaArgs: ["-e", script],
    tuaCommand: process.execPath,
  });
  terminal.onDidWrite((text) => lines.push(text.replace(/\r\n$/u, "")));
  return new Promise((resolve) => {
    terminal.onDidClose((code) => resolve({ code, lines }));
    terminal.open();
  });
}

test("the task terminal streams the tua command and reports its exit code", async () => {
  const { code, lines } = await runTerminal(
    "console.log('rebuild: rebuilt 1 file(s)'); process.exitCode = 3",
    os.tmpdir(),
  );

  assert.equal(code, 3);
  assert(lines[0].startsWith("$ "), "the command line is echoed first");
  assert(lines.includes("rebuild: rebuilt 1 file(s)"));
});

test("source maps load once the dev game has started", async () => {
  const missing = path.join(os.tmpdir(), `tua-no-build-${process.pid}-${Date.now()}`);
  const { lines } = await runTerminal("console.log('dev: started love build')", missing);

  assert(lines.includes("dev: started love build"));
  assert(
    lines.some((line) => line.includes("source maps") || line.includes("Could not load source maps")),
    lines.join("\n"),
  );
});

test("fused package targets name the setting that holds their LOVE runtime", () => {
  assert.deepEqual(
    PACKAGE_TARGETS.map((target) => [target.target, target.setting]),
    [
      ["love", undefined],
      ["windows", "love.windowsRuntime"],
      ["macos", "love.macosApp"],
    ],
  );
});
