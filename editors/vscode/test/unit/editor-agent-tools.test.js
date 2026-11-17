"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

function fixture() {
  const tools = new Map();
  const calls = [];
  const vscode = {
    commands: { executeCommand: async (name, options) => {
      calls.push({ name, options });
      return { taskName: name };
    } },
    debug: { activeDebugSession: undefined },
    Disposable: { from: (...items) => ({ dispose() { items.forEach(item => item.dispose()); } }) },
    LanguageModelTextPart: class { constructor(value) { this.value = value; } },
    LanguageModelToolResult: class { constructor(content) { this.content = content; } },
    lm: { registerTool: (name, tool) => {
      tools.set(name, tool);
      return { dispose() { tools.delete(name); } };
    } },
    workspace: { isTrusted: true, workspaceFolders: [{ uri: { scheme: "file" } }] },
  };
  const filename = path.resolve(__dirname, "../../src/agents/editor-tools.js");
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(filename, "utf8"), {
    module, setTimeout, clearTimeout,
    require: id => id === "vscode" ? vscode
      : id === "../constants" ? { COMMANDS: { runLoveProject: "tua.runLoveProject", debugLoveProject: "tua.debugLoveProject" } }
        : require(id),
  }, { filename });
  return { api: module.exports, vscode, tools, calls };
}

function value(response) { return JSON.parse(response.content[0].value); }

test("editor agent actions reuse Tua commands and refuse untrusted projects", async () => {
  const f = fixture();
  const registration = f.api.registerEditorAgentTools({ sessions: [] }, { entries: [] });
  assert.equal(f.tools.size, 4);
  assert.match(f.tools.get("tua_build_project").prepareInvocation().confirmationMessages.message, /starts a Tua task/);
  assert.equal(value(await f.tools.get("tua_build_project").invoke()).started, true);
  assert.equal(value(await f.tools.get("tua_run_love").invoke()).started, true);
  assert.equal(value(await f.tools.get("tua_start_love_debug").invoke()).started, true);
  assert.equal(f.calls.map(call => call.name).join(","), "tua.buildProject,tua.runLoveProject,tua.debugLoveProject");
  assert.equal(f.calls[1].options.console, true);
  f.vscode.workspace.isTrusted = false;
  await assert.rejects(f.tools.get("tua_build_project").invoke(), /trust a local Tua project/);
  assert.equal(f.calls.length, 3);
  registration.dispose();
  assert.equal(f.tools.size, 0);
});

test("debug context reads only the active Tua debugger and bounds stack, locals, and output", async () => {
  const f = fixture();
  const requests = [];
  f.vscode.debug.activeDebugSession = {
    id: "debug-1", name: "Debug LOVE Project (Tua)", type: "lua-local",
    configuration: { name: "Debug LOVE Project (Tua)" },
    async customRequest(method, args) {
      requests.push({ method, args });
      if (method === "threads") return { threads: [{ id: 7, name: "Main" }] };
      if (method === "stackTrace") return { stackFrames: [{ id: 9, name: "update", line: 12, source: { path: "/game/src/main.tua" } }] };
      if (method === "scopes") return { scopes: [{ name: "Locals", variablesReference: 11 }] };
      return { variables: [{ name: "hp", value: "4", type: "number" }] };
    },
  };
  const activity = { sessions: [{ id: "1", kind: "debug", phase: "running", active: true }] };
  const buffer = { entries: Array.from({ length: 45 }, (_, i) => ({
    session: { id: "1" }, severity: "info", text: `line ${i}`, time: i,
  })) };
  const context = await f.api.debugContext(activity, buffer);
  assert.equal(context.output.length, 40);
  assert.equal(context.output[0].text, "line 5");
  assert.equal(context.debug.frames[0].name, "update");
  assert.equal(context.debug.scopes[0].variables[0].value, "4");
  assert.equal(requests.map(request => request.method).join(","), "threads,stackTrace,scopes,variables");
  f.vscode.debug.activeDebugSession = { type: "lua-local", configuration: { name: "Other Lua game" } };
  assert.equal((await f.api.debugContext(activity, buffer)).debug.active, false);
  assert.equal(requests.length, 4);
});
