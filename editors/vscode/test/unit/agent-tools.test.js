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
  const cliCalls = [];
  const codexEntries = [];
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
      showWarningMessage: async message => { messages.push(message); return undefined; },
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
  const agentBin = path.join(root, "agent-bin");
  const agentEnv = { PATH: agentBin };
  const fakeExecFile = (command, args, options, callback) => {
    cliCalls.push({ command, args, options });
    const name = path.basename(command).replace(/\.exe$/, "");
    if (name === "codex") {
      if (args[1] === "list") return callback(null, JSON.stringify(codexEntries), "");
      const separator = args.indexOf("--");
      codexEntries.push({ name: args[2], enabled: true, transport: {
        type: "stdio", command: args[separator + 1], args: args.slice(separator + 2),
      } });
      return callback(null, "Added", "");
    }
    if (name === "claude" || name === "gemini") {
      const separator = args.indexOf("--");
      const serverName = name === "claude" ? args[separator - 1] : args[4];
      const definition = name === "claude"
        ? { command: args[separator + 1], args: args.slice(separator + 2) }
        : { command: args[5], args: [args[6], ...args.slice(separator + 1)] };
      const configPath = name === "claude"
        ? path.join(agentEnv.CLAUDE_CONFIG_DIR || root, ".claude.json")
        : path.join(root, ".gemini", "settings.json");
      const settings = fs.existsSync(configPath) ? JSON.parse(fs.readFileSync(configPath, "utf8")) : {};
      if (name === "claude") {
        settings.projects ||= {};
        const projectPath = fs.existsSync(options.cwd) ? fs.realpathSync(options.cwd) : options.cwd;
        settings.projects[projectPath] ||= {};
        settings.projects[projectPath].mcpServers ||= {};
        settings.projects[projectPath].mcpServers[serverName] = definition;
      } else {
        settings.mcpServers ||= {};
        settings.mcpServers[serverName] = definition;
      }
      fs.mkdirSync(path.dirname(configPath), { recursive: true });
      fs.writeFileSync(configPath, JSON.stringify(settings));
      return callback(null, "Added", "");
    }
    callback(new Error("Unknown fake CLI"), "", "");
  };
  vm.runInNewContext(fs.readFileSync(filename, "utf8"), {
    module,
    process: { platform: process.platform, pid: process.pid, env: agentEnv },
    setTimeout,
    clearTimeout,
    require: id => id === "vscode" ? vscode
      : id === "node:child_process" ? { spawn: fakeMcpProcess, execFile: fakeExecFile }
        : id === "node:os" ? { homedir: () => root }
        : id === "../constants" ? { COMMANDS: { setupAgentTools: "tua.setup.agentTools" } }
        : require(id),
  }, { filename });
  return { api: module.exports, context, folder, commands, provider: () => provider, vscode, copied, messages,
    cliCalls, agentBin, agentEnv, codexEntries };
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

test("agent setup copies client-specific MCP settings when native registration is unavailable", async () => {
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

test("agent setup registers and verifies Codex, Claude Code, and Gemini CLI without replacing other entries", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-agent-register-"));
  try {
    fs.mkdirSync(path.join(root, "agent-bin"));
    for (const client of ["codex", "claude", "gemini"]) {
      const executable = process.platform === "win32" ? `${client}.exe` : client;
      fs.writeFileSync(path.join(root, "agent-bin", executable), "fake CLI", { mode: 0o755 });
      const f = fixture(root, client);
      fs.mkdirSync(f.folder.uri.fsPath, { recursive: true });
      if (client === "claude") f.agentEnv.CLAUDE_CONFIG_DIR = path.join(root, "claude-config");
      const bundled = path.join(f.context.extensionPath, "bin", process.platform === "win32" ? "tua.exe" : "tua");
      fs.mkdirSync(path.dirname(bundled), { recursive: true });
      fs.writeFileSync(bundled, "binary");
      if (client === "codex") f.codexEntries.push({ name: "unrelated", transport: { type: "stdio", command: "other", args: [] } });
      f.api.registerAgentTools(f.context);
      await f.commands.get("tua.setup.agentTools")();
      assert.equal(f.copied.length, 0, client);
      assert.match(f.messages[0], /registered for/, client);
      assert.equal(f.cliCalls.filter(call => call.args[1] === "add").length, 1, client);
      assert.equal(f.cliCalls.find(call => call.args[1] === "add").options.cwd, f.folder.uri.fsPath);
      await f.commands.get("tua.setup.agentTools")();
      assert.match(f.messages[1], /already registered/, client);
      assert.equal(f.cliCalls.filter(call => call.args[1] === "add").length, 1, client);
    }
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});

test("agent setup preserves a conflicting MCP name and copies manual settings", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-agent-conflict-"));
  try {
    const f = fixture(root, "codex");
    fs.mkdirSync(f.agentBin, { recursive: true });
    fs.writeFileSync(path.join(f.agentBin, process.platform === "win32" ? "codex.exe" : "codex"),
      "fake CLI", { mode: 0o755 });
    const bundled = path.join(f.context.extensionPath, "bin", process.platform === "win32" ? "tua.exe" : "tua");
    fs.mkdirSync(path.dirname(bundled), { recursive: true });
    fs.writeFileSync(bundled, "binary");
    f.codexEntries.push({ name: f.api.serverName(f.folder), enabled: true,
      transport: { type: "stdio", command: "another-tool", args: [] } });
    f.api.registerAgentTools(f.context);
    await f.commands.get("tua.setup.agentTools")();
    assert.equal(f.cliCalls.some(call => call.args[1] === "add"), false);
    assert.equal(f.copied.length, 1);
    assert.match(f.messages[0], /already has a different MCP server/);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});
