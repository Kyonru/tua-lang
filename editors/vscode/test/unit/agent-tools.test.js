"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

function fixture(root, execFile) {
  const commands = new Map();
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
      showInformationMessage: async () => undefined,
      showErrorMessage: async () => undefined,
    },
    env: { clipboard: { writeText: async () => {} } },
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
      : id === "node:child_process" ? { execFile }
        : id === "../constants" ? { COMMANDS: { setupAgentTools: "tua.setup.agentTools" } }
        : require(id),
  }, { filename });
  return { api: module.exports, context, folder, commands, provider: () => provider, vscode };
}

test("agent discovery uses the packaged CLI and setup installs a stable copy only on request", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-agent-tools-"));
  try {
    const f = fixture(root, () => {});
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

test("Codex registration is scoped to the project and reuses an exact existing entry", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-codex-tools-"));
  try {
    const calls = [];
    let existing;
    const f = fixture(root, (_command, args, _options, callback) => {
      calls.push(args);
      if (args[1] === "get" && !existing) {
        const error = new Error("missing");
        error.code = 1;
        callback(error, "", "Error: No MCP server named 'tua-game' found.");
      } else if (args[1] === "add") {
        existing = { transport: { command: args[4], args: args.slice(5) } };
        callback(null, "", "");
      } else callback(null, JSON.stringify(existing), "");
    });
    const binary = path.join(root, "stable", "tua");
    const first = await f.api.configureCodex(binary, f.folder);
    assert.equal(first.alreadyConfigured, false);
    assert.equal(calls[1][1], "add");
    assert.equal(calls[1].at(-1), f.folder.uri.fsPath);
    const second = await f.api.configureCodex(binary, f.folder);
    assert.equal(second.alreadyConfigured, true);
    assert.equal(calls.filter(args => args[1] === "add").length, 1);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});
