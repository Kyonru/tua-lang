"use strict";
// Optional desktop test: TUA_BIN=... TUA_DEBUGGER_EXTENSION_PATH=... node scripts/multiplayer-runtime-e2e.js
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const childProcess = require("node:child_process");
const Module = require("node:module");
const { DebugClient } = require("@vscode/debugadapter-testsupport");
const { ActivityStore } = require("../src/workspace/activity-store");
const tua = process.env.TUA_BIN || path.resolve(__dirname, "../../../target/debug/tua");
const love = process.env.TUA_LOVE || (process.platform === "darwin" ? "/Applications/love.app/Contents/MacOS/love" : "love");
const roots = [], starts = [], ends = [], trackers = [];
const clients = new Map();
class EventEmitter {
  constructor() { this.listeners = new Set(); this.event = fn => { this.listeners.add(fn); return { dispose: () => this.listeners.delete(fn) }; }; }
  fire(value) { for (const fn of this.listeners) fn(value); }
  dispose() { this.listeners.clear(); }
}
const listen = (list, fn) => { list.push(fn); return { dispose: () => list.splice(list.indexOf(fn), 1) }; };
const vscode = {
  EventEmitter, Uri: { file: fsPath => ({ fsPath }) }, workspace: { getWorkspaceFolder: () => undefined },
  window: { showErrorMessage: message => console.error(message) },
  debug: {
    onDidStartDebugSession: fn => listen(starts, fn),
    onDidTerminateDebugSession: fn => listen(ends, fn),
    registerDebugAdapterTrackerFactory: (_type, factory) => listen(trackers, factory),
    async startDebugging(_folder, configuration) {
      const root = process.env.TUA_DEBUGGER_EXTENSION_PATH;
      assert(root, "Set TUA_DEBUGGER_EXTENSION_PATH to a local Lua debugger extension directory");
      const adapter = path.join(root, "extension/debugAdapter.js");
      const client = new DebugClient(process.execPath, adapter, "lua-local", { cwd: path.dirname(adapter), env: process.env }, true);
      client.defaultTimeout = 15000;
      const session = { id: configuration.tuaDebugInstance, configuration };
      const item = { client, session, terminated: false };
      clients.set(session.id, item);
      const listeners = trackers.map(factory => factory.createDebugAdapterTracker(session)).filter(Boolean);
      const adapterOutput = [];
      client.on("output", event => { adapterOutput.push(event.body?.output || ""); listeners.forEach(tracker => tracker.onDidSendMessage({ type: "event", event: "output", body: event.body })); });
      client.on("terminated", () => { item.terminated = true; ends.forEach(fn => fn(session)); });
      await client.start();
      starts.forEach(fn => fn(session));
      const source = path.join(configuration.cwd, "src/main.tua");
      let result;
      try { result = await client.hitBreakpoint({ ...configuration, extensionPath: root, workspacePath: configuration.cwd, verbose: true }, { path: source, line: 4, verified: true }, { path: source, line: 4 }); }
      catch (error) { throw new Error(error.message + "\n" + adapterOutput.join("")); }
      assert.equal(result[2].body.stackFrames[0].source.path, source);
      return true;
    },
    async stopDebugging(session) {
      const item = clients.get(session.id);
      if (!item || item.terminated) return;
      const done = item.client.waitForEvent("terminated");
      await item.client.disconnectRequest({ terminateDebuggee: true });
      await done;
    },
  },
};
const original = Module._load;
Module._load = function(id, ...args) { return id === "vscode" ? vscode : original.call(this, id, ...args); };
const { HotReloadInstancesTerminal } = require("../src/love/hot-reload-instances");
const { registerDebugInstanceGroups } = require("../src/love/debug-instances");
const { prepareLoveDebugOutput, createLoveDebugConfiguration, debuggerScriptFiles } = require("../src/love/debug-project");
Module._load = original;
function fixture(main) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-multiplayer-runtime-"));
  roots.push(root);
  fs.mkdirSync(path.join(root, "src"));
  fs.writeFileSync(path.join(root, "tua.toml"), 'source = "src"\nout = "build"\nmain = "main.tua"\ntarget = "love11-luajit"\nextension = "tua"\nemit_maps = true\n');
  fs.writeFileSync(path.join(root, "src/main.tua"), main);
  fs.writeFileSync(path.join(root, "src/conf.lua"), 'function love.conf(t) t.window.visible = false; t.window.width = 64; t.window.height = 64; t.modules.audio = false end\n');
  return root;
}
async function until(predicate, label) {
  const deadline = Date.now() + 15000;
  while (!predicate()) { assert(Date.now() < deadline, `Timed out: ${label}`); await new Promise(resolve => setTimeout(resolve, 20)); }
}
const instances = [{ name: "Host", args: ["host"] }, { name: "Client", args: ["client"] }];
async function hotReload() {
  const root = fixture('io.stdout:setvbuf("no")\nlocal state = require("state")\nlocal player = ""\nlocal previous = 0\nfunction love.load(args) player = args[1] end\nfunction love.update()\n  local value = state.value()\n  if value ~= previous then print(player .. ":" .. value); previous = value end\nend\n');
  const module = path.join(root, "src/state.tua");
  fs.writeFileSync(module, 'return { value = function() return 1 end }\n');
  const activity = new ActivityStore(), output = [];
  activity.events.on("output", event => output.push(event.line));
  const terminal = new HotReloadInstancesTerminal({ activity, kind: "dev", title: "Reload players", cwd: root, outputDirectory: path.join(root, "build"), tuaCommand: tua, loveCommand: love, instances });
  const running = terminal.run();
  try {
    await until(() => output.includes("host:1") && output.includes("client:1"), "both players start");
    fs.writeFileSync(module, 'return { value = function() return 2 end }\n');
    await until(() => output.includes("host:2") && output.includes("client:2"), "both players reload");
    fs.writeFileSync(module, 'local invalid =\n');
    await until(() => output.includes("dev: keeping the running game on its last good build"), "failed build preserves live players");
    assert(terminal.children.every(child => !child.finished));
    fs.writeFileSync(module, 'return { value = function() return 3 end }\n');
    await until(() => output.includes("host:3") && output.includes("client:3"), "players recover after build error");
    terminal.children[0].close();
    await until(() => !terminal.children[0].activeProcess, "first player stops");
    assert(!terminal.children[1].finished);
    fs.writeFileSync(module, 'return { value = function() return 4 end }\n');
    await until(() => output.includes("client:4"), "remaining player still reloads");
    fs.appendFileSync(path.join(root, "src/main.tua"), '\nprint("entry-restarted")\n');
    await until(() => output.includes("entry-restarted") && output.filter(line => line === "client:4").length >= 2, "entrypoint restart retains player arguments");
    console.log("Native hot reload passed: shared reloads, build-error recovery, independent stop, entrypoint restart");
  } catch (error) { console.error(output.join("\n")); throw error; }
  finally { terminal.close(); await running; activity.dispose(); }
}
async function debug() {
  const root = fixture('local marker: number = 41\n\nfunction love.load(args)\n  marker = marker + 1\n  print(args[1] .. ":" .. marker)\nend\nfunction love.update() love.event.quit() end\n');
  const activity = new ActivityStore();
  let builds = 0;
  const feature = registerDebugInstanceGroups({ activity, prepare: prepareLoveDebugOutput, configuration: createLoveDebugConfiguration, scriptFiles: debuggerScriptFiles,
    build: async () => { builds++; const result = childProcess.spawnSync(tua, ["build"], { cwd: root, encoding: "utf8" }); assert.equal(result.status, 0, result.stdout + result.stderr); return 0; } });
  try {
    const result = await feature.start({ layout: { configDirectory: root, workspaceRoot: root, outputDirectory: path.join(root, "build") }, instances, title: "Debug players", tuaCommand: tua, loveCommand: love });
    assert(result, "debug group must start");
    assert.equal(builds, 1);
    const [host, client] = [...clients.values()];
    assert(!host.terminated && !client.terminated);
    const hostThreads = await host.client.threadsRequest();
    const hostEnded = host.client.waitForEvent("terminated");
    await host.client.continueRequest({ threadId: hostThreads.body.threads[0].id });
    await hostEnded;
    assert(!client.terminated, "continuing Host leaves Client paused");
    assert(fs.existsSync(path.join(result.outputDirectory, "conf.lua")));
    const clientThreads = await client.client.threadsRequest();
    const clientEnded = client.client.waitForEvent("terminated");
    await client.client.continueRequest({ threadId: clientThreads.body.threads[0].id });
    await clientEnded;
    assert(!fs.existsSync(result.outputDirectory));
    console.log("Native debugging passed: source breakpoints, independent continue, last-player cleanup");
  } finally {
    for (const item of clients.values()) {
      if (!item.terminated) await Promise.race([item.client.terminateRequest({}).catch(() => {}), new Promise(resolve => setTimeout(resolve, 2000))]);
      // This adapter no longer answers disconnect after its game has exited.
      item.client.stopAdapter();
    }
    await feature.dispose(); activity.dispose();
  }
}
(async () => {
  try { if (!process.env.TUA_DEBUG_ONLY) await hotReload(); await debug(); }
  finally { for (const root of roots) fs.rmSync(root, { recursive: true, force: true }); }
})().then(() => process.exit(0), error => { console.error(error); process.exit(1); });
