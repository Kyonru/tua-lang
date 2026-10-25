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
const { LoveProjectTerminal } = require("../../src/love/run-project");
Module._load = originalLoad;


const fs = require("node:fs");

function fixture(t, instances, script) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "tua-players-"));
  const program = path.join(directory, "game.js");
  fs.writeFileSync(program, script);
  const sessions = [];
  const activity = {
    begin(options) { const session = { ...options, lines: [] }; sessions.push(session); return session; },
    update(session, phase) { session.phase = phase; },
    output(session, line) { session.lines.push(line); },
    line() {},
    finish(session, code, stopped) { Object.assign(session, { code, stopped, finished: true }); },
  };
  const terminal = new LoveProjectTerminal({ cwd: directory, outputDirectory: program,
    loveCommand: process.execPath, tuaCommand: "unused", instances, title: "Players", activity });
  let builds = 0;
  terminal.runProcess = async () => { builds++; return 0; };
  t.after(() => { terminal.close(); fs.rmSync(directory, { recursive: true, force: true }); });
  return { terminal, sessions, builds: () => builds };
}

async function until(predicate) {
  const deadline = Date.now() + 5000;
  while (!predicate()) {
    assert(Date.now() < deadline, "timed out waiting for child process");
    await new Promise(resolve => setTimeout(resolve, 10));
  }
}

test("one build starts independent players with literal arguments and isolated logs", async t => {
  const { terminal, sessions, builds } = fixture(t, [
    { name: "Host", args: ["--host", "two words", "$(literal)"] },
    { name: "Client", args: ["--join", "localhost"] },
  ], "console.log(JSON.stringify(process.argv.slice(2)))");
  await terminal.run();
  assert.equal(builds(), 1);
  assert.equal(sessions[0].code, 0);
  assert.equal(sessions[1].title, "Players: Host");
  assert.equal(sessions[2].title, "Players: Client");
  assert(sessions[1].lines.includes('["--host","two words","$(literal)"]'));
  assert(sessions[2].lines.includes('["--join","localhost"]'));
  assert(!sessions[2].lines.includes('["--host","two words","$(literal)"]'));
});

test("closing one player leaves the other running; group stop closes the rest", async t => {
  const { terminal, sessions } = fixture(t, [
    { name: "Host", args: [] }, { name: "Client", args: [] },
  ], "console.log('ready'); setInterval(() => {}, 1000)");
  const running = terminal.run();
  await until(() => sessions.length === 3 && sessions.slice(1).every(s => s.lines.includes("ready")));
  sessions[1].stop();
  await until(() => sessions[1].finished);
  assert(!sessions[2].finished);
  assert(!terminal.finished);
  sessions[0].stop();
  await running;
  assert(sessions.every(s => s.finished));
  assert(terminal.children.every(child => child.activeProcess === undefined));
});

test("a failed build launches no players", async t => {
  const { terminal, sessions } = fixture(t, [{ name: "Host", args: [] }], "process.exit(0)");
  terminal.runProcess = async () => 7;
  await terminal.run();
  assert.equal(sessions.length, 1);
  assert.equal(sessions[0].code, 7);
});

test("a player failure is reported without stopping its peers", async t => {
  const { terminal, sessions } = fixture(t, [
    { name: "Host", args: ["fail"] }, { name: "Client", args: [] },
  ], "if (process.argv[2] === 'fail') process.exit(3); console.log('ready'); setInterval(() => {}, 1000)");
  const running = terminal.run();
  await until(() => sessions.length === 3 && sessions[1].finished && sessions[2].lines.includes("ready"));
  assert.equal(sessions[1].code, 3);
  assert(!sessions[2].finished);
  sessions[2].stop();
  await running;
  assert.equal(sessions[0].code, 3);
});
