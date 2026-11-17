"use strict";

const assert = require("node:assert/strict");
const { EventEmitter } = require("node:events");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { PassThrough } = require("node:stream");
const test = require("node:test");
const vm = require("node:vm");

function fixture(root, clientChoice = "claude") {
  const commands = new Map();
  const copied = [];
  const messages = [];
  let provider;
  const folder = { name: "game", uri: { scheme: "file", fsPath: path.join(root, "game") } };
  const vscode = {
    commands: { registerCommand: (name, action) => {
      commands.set(name, action);
      return { dispose() { commands.delete(name); } };
    } },
    Disposable: { from: (...items) => ({ dispose() { items.forEach(item => item.dispose()); } }) },
    EventEmitter: class { event = () => {}; fire() {} dispose() {} },
    McpStdioServerDefinition: class {
      constructor(label, command, args, env, version) {
        Object.assign(this, { label, command, args, env, version });
      }
    },
    lm: { registerMcpServerDefinitionProvider: (_id, value) => {
      provider = value;
      return { dispose() {} };
    } },
    workspace: {
      isTrusted: true,
      workspaceFolders: [folder],
      onDidChangeWorkspaceFolders: () => ({ dispose() {} }),
      onDidGrantWorkspaceTrust: () => ({ dispose() {} }),
    },
    window: {
      showQuickPick: async choices => choices.find(choice => choice.id === clientChoice),
      showInformationMessage: async message => { messages.push(message); return undefined; },
      showErrorMessage: async () => undefined,
    },
    env: { clipboard: { writeText: async value => { copied.push(value); } } },
  };
  const context = {
    extensionPath: path.join(root, "extension"),
    globalStorageUri: { scheme: "file", fsPath: path.join(root, "storage") },
    extension: { packageJSON: { version: "0.10.0" } },
  };
  const filename = path.resolve(__dirname, "../../src/setup/agent-tools.js");
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(filename, "utf8"), {
    module,
    process,
    setTimeout,
    clearTimeout,
    require: id => id === "vscode" ? vscode
      : id === "node:child_process" ? { spawn: fakeMcpProcess }
        : id === "../constants" ? { COMMANDS: { setupAgentTools: "tua.setup.agentTools" } }
        : require(id),
  }, { filename });
  return { api: module.exports, context, folder, commands, provider: () => provider, vscode, copied, messages };
}

function fakeMcpProcess() {
  const child = new EventEmitter();
  child.stdin = new PassThrough();
  child.stdout = new PassThrough();
  child.stderr = new PassThrough();
  child.kill = () => {};
  queueMicrotask(() => {
    child.stdout.write(JSON.stringify({ id: 2, result: { structuredContent: { workspace: "test" } } }) + "\n");
    child.emit("close", 0);
  });
  return child;
}

test("agent discovery uses the packaged CLI and setup installs a stable copy only on request", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-agent-tools-"));
  try {
    const f = fixture(root);
    const bundled = path.join(f.context.extensionPath, "bin", process.platform === "win32" ? "tua.exe" : "tua");
    fs.mkdirSync(path.dirname(bundled), { recursive: true });
    fs.writeFileSync(bundled, "first binary");
    const registration = f.api.registerAgentTools(f.context);
    const target = f.api.stableCli(f.context);
    assert.equal(fs.existsSync(target), false, "activation must not install a CLI");
    const definitions = f.provider().provideMcpServerDefinitions();
    assert.equal(definitions.length, 1);
    assert.equal(definitions[0].command, bundled);
    assert.equal(definitions[0].args.at(-1), f.folder.uri.fsPath);
    assert.equal(definitions[0].cwd, f.folder.uri);
    f.vscode.workspace.isTrusted = false;
    assert.equal(f.provider().provideMcpServerDefinitions().length, 0);
    f.vscode.workspace.isTrusted = true;
    assert.equal(f.api.installCli(f.context), target);
    assert.equal(fs.readFileSync(target, "utf8"), "first binary");
    fs.writeFileSync(bundled, "updated binary");
    f.api.installCli(f.context);
    assert.equal(fs.readFileSync(target, "utf8"), "updated binary");
    registration.dispose();
    assert.equal(f.commands.size, 0);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});

test("agent setup uses the editor's local global storage path for a non-file storage URI", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-agent-storage-"));
  try {
    const f = fixture(root);
    f.context.globalStorageUri = { scheme: "vscode-userdata", fsPath: path.join(root, "virtual-storage") };
    f.context.globalStoragePath = path.join(root, "storage");
    const bundled = path.join(f.context.extensionPath, "bin", process.platform === "win32" ? "tua.exe" : "tua");
    fs.mkdirSync(path.dirname(bundled), { recursive: true });
    fs.writeFileSync(bundled, "binary");
    const target = path.join(f.context.globalStoragePath, "bin", path.basename(bundled));
    assert.equal(f.api.installCli(f.context), target);
    assert.equal(fs.readFileSync(target, "utf8"), "binary");
    f.api.registerAgentTools(f.context);
    await f.commands.get("tua.setup.agentTools")();
    assert.equal(JSON.parse(f.copied[0]).projects[f.folder.uri.fsPath].mcpServers[f.api.serverName(f.folder)].command, target);
    delete f.context.globalStoragePath;
    assert.throws(() => f.api.installCli(f.context), /did not provide a local global storage path/);
    fs.rmSync(bundled);
    assert.throws(() => f.api.installCli(f.context), /has no bundled CLI at/);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});

test("agent setup offers client-specific MCP settings without writing agent configuration", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-client-tools-"));
  try {
    const f = fixture(root);
    const bundled = path.join(f.context.extensionPath, "bin", process.platform === "win32" ? "tua.exe" : "tua");
    fs.mkdirSync(path.dirname(bundled), { recursive: true });
    fs.writeFileSync(bundled, "binary");
    f.api.registerAgentTools(f.context);
    await f.commands.get("tua.setup.agentTools")();
    assert.equal(f.copied.length, 1);
    const name = f.api.serverName(f.folder);
    const claude = JSON.parse(f.copied[0]).projects[f.folder.uri.fsPath].mcpServers[name];
    assert.equal(claude.command, f.api.stableCli(f.context));
    assert.equal(claude.args.at(-1), f.folder.uri.fsPath);
    assert.match(f.messages[0], /\.claude\.json/);
    assert.match(f.api.clientSetup("codex", claude.command, f.folder, false).text, /\[mcp_servers\./);
    const gemini = JSON.parse(f.api.clientSetup("gemini", claude.command, f.folder, false).text);
    assert.equal(gemini.mcpServers[name].command, claude.command);
    const other = JSON.parse(f.api.clientSetup("other", claude.command, f.folder, false).text);
    assert.equal(other.name, name);
    assert.equal(other.type, "stdio");
    assert.equal(f.api.clientSetup("copilot", claude.command, f.folder, true).available, true);
    const copilotFallback = JSON.parse(f.api.clientSetup("copilot", claude.command, f.folder, false).text).mcpServers[name];
    assert.equal(copilotFallback.command, claude.command);
    assert.equal(copilotFallback.type, "stdio");
    for (const client of ["codex", "gemini", "other", "copilot"]) {
      const selected = fixture(root, client);
      selected.api.registerAgentTools(selected.context);
      await selected.commands.get("tua.setup.agentTools")();
      assert.equal(selected.copied.length, client === "copilot" ? 0 : 1, client);
      assert.equal(selected.messages.length, 1, client);
    }
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});
