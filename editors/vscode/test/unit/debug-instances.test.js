"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const Module = require("node:module");
const { ActivityStore } = require("../../src/workspace/activity-store");
let state;
const vscode = {
  Uri: { file: fsPath => ({ fsPath }) },
  workspace: { getWorkspaceFolder: () => undefined },
  window: { showErrorMessage: message => state.errors.push(message) },
  debug: {
    onDidStartDebugSession: fn => { state.start = fn; return { dispose() {} }; },
    onDidTerminateDebugSession: fn => { state.end = fn; return { dispose() {} }; },
    registerDebugAdapterTrackerFactory: (_type, factory) => { state.trackers = factory; return { dispose() {} }; },
    async startDebugging(_folder, configuration) {
      if (state.failAt === state.sessions.length) return false;
      const session = { id: String(state.sessions.length), configuration };
      state.sessions.push(session);
      state.start(session);
      return true;
    },
    async stopDebugging(session) { state.stops.push(session.id); state.end(session); },
  },
};
const original = Module._load;
Module._load = function(id, ...args) { return id === "vscode" ? vscode : original.call(this, id, ...args); };
const { registerDebugInstanceGroups } = require("../../src/love/debug-instances");
const { prepareLoveDebugOutput, createLoveDebugConfiguration, debuggerScriptFiles } = require("../../src/love/debug-project");
const { loadSourceMapForGeneratedPath } = require("../../src/editor/source-maps");
Module._load = original;

function fixture(t, build) {
  state = { sessions: [], errors: [], stops: [], builds: 0 };
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-debug-test-"));
  const outputDirectory = path.join(root, "build");
  fs.mkdirSync(outputDirectory);
  fs.mkdirSync(path.join(root, "src"));
  fs.writeFileSync(path.join(outputDirectory, "main.lua"), "print('game')\n");
  fs.writeFileSync(path.join(outputDirectory, "main.lua.map"), JSON.stringify({ version: 3, file: "main.lua", sourceRoot: "..", sources: ["src/main.tua"], mappings: "AAAA", names: [] }));
  fs.writeFileSync(path.join(outputDirectory, "main.lua.map.json"), JSON.stringify({ version: 1, generated: "build/main.lua", source: "src/main.tua", lineMappings: [{ generatedLine: 1, sourceLine: 1 }] }));
  const activity = new ActivityStore();
  const feature = registerDebugInstanceGroups({ activity, build: build || (async () => { state.builds++; return 0; }),
    prepare: prepareLoveDebugOutput, configuration: createLoveDebugConfiguration, scriptFiles: debuggerScriptFiles });
  const options = { layout: { outputDirectory, configDirectory: root, workspaceRoot: root }, title: "Players", configurationName: "Players",
    instances: [{ name: "Host", args: ["--host"] }, { name: "Client", args: ["--join", "localhost"] }], tuaCommand: "tua", loveCommand: "love" };
  t.after(() => { feature.dispose(); activity.dispose(); fs.rmSync(root, { recursive: true, force: true }); });
  return { feature, activity, options, root };
}

test("debug players build once, retain source maps, and release their snapshot only after the last exit", async t => {
  const { feature, activity, options, root } = fixture(t);
  const result = await feature.start(options);
  assert.equal(state.builds, 1);
  const [host, client] = state.sessions;
  assert.notEqual(host.configuration.tuaDebugInstance, client.configuration.tuaDebugInstance);
  assert.deepEqual(host.configuration.args.slice(1), ["--host"]);
  assert.deepEqual(client.configuration.args.slice(1), ["--join", "localhost"]);
  assert(!fs.existsSync(path.join(options.layout.outputDirectory, "conf.lua")), "source build stays unmodified");
  const map = JSON.parse(fs.readFileSync(path.join(result.outputDirectory, "main.lua.map")));
  assert.equal(path.resolve(map.sourceRoot, map.sources[0]), path.join(root, "src/main.tua"));
  assert.equal(loadSourceMapForGeneratedPath(path.join(result.outputDirectory, "main.lua")).sourcePath, path.join(root, "src/main.tua"));
  const output = [];
  activity.events.on("output", event => output.push(event));
  state.trackers.createDebugAdapterTracker(host).onDidSendMessage({ type: "event", event: "output", body: { output: "host-only" } });
  assert.equal(output.length, 1);
  assert.equal(output[0].session.title, "Players: Host");
  state.end(host);
  assert(fs.existsSync(path.join(result.outputDirectory, "conf.lua")), "peer still needs the bootstrap");
  assert(activity.sessions.find(s => s.title === "Players: Client").active);
  state.end(client);
  assert(!fs.existsSync(result.outputDirectory));
  assert(activity.sessions.every(s => !s.active));
});

test("player stop is independent; group stop closes all remaining debugger sessions", async t => {
  const { feature, activity, options } = fixture(t);
  const result = await feature.start(options);
  activity.sessions.find(s => s.title === "Players: Host").stop();
  assert.deepEqual(state.stops, ["0"]);
  assert(fs.existsSync(result.outputDirectory));
  activity.sessions.find(s => s.title === "Players").stop();
  assert.deepEqual(state.stops, ["0", "1"]);
  assert(!fs.existsSync(result.outputDirectory));
});

test("partial debugger launch failure stops started peers and cleans up", async t => {
  const { feature, activity, options } = fixture(t);
  state.failAt = 1;
  assert.equal(await feature.start(options), undefined);
  assert.deepEqual(state.stops, ["0"]);
  assert(!fs.existsSync(state.sessions[0].configuration.args[0]));
  assert(activity.sessions.every(s => !s.active));
  assert.match(state.errors[0], /Client/);
});

test("stop during build prevents debugger launches", async t => {
  let complete;
  let signal;
  const { feature, activity, options } = fixture(t, opts => { signal = opts.signal; return new Promise(resolve => { complete = resolve; }); });
  const pending = feature.start(options);
  activity.sessions[0].stop();
  assert(signal.aborted);
  complete(0);
  assert.equal(await pending, undefined);
  assert.equal(state.sessions.length, 0);
  assert.equal(activity.sessions[0].phase, "stopped");
});

test("a failed build never prepares or launches a debugger", async t => {
  const { feature, activity, options } = fixture(t, async () => 5);
  assert.equal(await feature.start(options), undefined);
  assert.equal(state.sessions.length, 0);
  assert.equal(activity.sessions[0].exitCode, 1);
});
