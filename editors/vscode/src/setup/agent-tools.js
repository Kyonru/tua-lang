"use strict";

const childProcess = require("node:child_process");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { COMMANDS } = require("../constants");

const PROVIDER_ID = "tua.mcp";

function executableName() {
  return process.platform === "win32" ? "tua.exe" : "tua";
}

function bundledCli(context) {
  const candidate = path.join(context.extensionPath, "bin", executableName());
  return fs.existsSync(candidate) ? candidate : undefined;
}

function stableCli(context) {
  if (context.globalStorageUri?.scheme !== "file") return undefined;
  return path.join(context.globalStorageUri.fsPath, "bin", executableName());
}

function installCli(context) {
  const source = bundledCli(context);
  const destination = stableCli(context);
  if (!source || !destination) throw new Error("This Tua install has no local bundled CLI");
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  const sourceHash = crypto.createHash("sha256").update(fs.readFileSync(source)).digest("hex");
  if (fs.existsSync(destination)) {
    const installedHash = crypto.createHash("sha256").update(fs.readFileSync(destination)).digest("hex");
    if (sourceHash === installedHash) return destination;
  }
  const temporary = path.join(path.dirname(destination), `.${executableName()}-${process.pid}.tmp`);
  try {
    fs.copyFileSync(source, temporary);
    if (process.platform !== "win32") fs.chmodSync(temporary, 0o755);
    try {
      fs.renameSync(temporary, destination);
    } catch (error) {
      if (process.platform !== "win32" || !["EEXIST", "EPERM"].includes(error.code)) throw error;
      fs.rmSync(destination, { force: true });
      fs.renameSync(temporary, destination);
    }
  } finally {
    fs.rmSync(temporary, { force: true });
  }
  return destination;
}

function codexName(folder) {
  const slug = folder.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 24) || "project";
  const suffix = crypto.createHash("sha256").update(folder.uri.fsPath).digest("hex").slice(0, 8);
  return `tua-${slug}-${suffix}`;
}

function codexArgs(binary, folder) {
  return [binary, "mcp", "--stdio", "--workspace", folder.uri.fsPath];
}

function execCodex(args) {
  return new Promise((resolve, reject) => {
    childProcess.execFile("codex", args, { timeout: 15000, windowsHide: true, maxBuffer: 65536 },
      (error, stdout, stderr) => {
        if (error) {
          error.stderr = stderr;
          reject(error);
        } else resolve({ stdout, stderr });
      });
  });
}

function probeMcp(binary, folder) {
  return new Promise((resolve, reject) => {
    const child = childProcess.spawn(binary, ["mcp", "--stdio", "--workspace", folder.uri.fsPath], {
      stdio: ["pipe", "pipe", "pipe"], windowsHide: true,
    });
    let stdout = "";
    let stderr = "";
    let settled = false;
    const finish = error => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (error) reject(error);
      else resolve();
    };
    const timer = setTimeout(() => {
      child.kill();
      finish(new Error("Tua MCP did not answer within 20 seconds"));
    }, 20000);
    child.once("error", finish);
    child.stdin.once("error", finish);
    child.stdout.on("data", chunk => {
      stdout += chunk.toString("utf8");
      if (stdout.length > 131072) {
        child.kill();
        finish(new Error("Tua MCP response was unexpectedly large"));
      }
    });
    child.stderr.on("data", chunk => { stderr = (stderr + chunk.toString("utf8")).slice(-4096); });
    child.once("close", code => {
      if (settled) return;
      try {
        const responses = stdout.trim().split("\n").filter(Boolean).map(JSON.parse);
        const summary = responses.find(row => row.id === 2)?.result;
        if (code !== 0 || !summary || summary.isError || !summary.structuredContent) {
          throw new Error(`Tua MCP tool call failed: ${stderr || code}`);
        }
        finish();
      } catch (error) { finish(error); }
    });
    for (const request of [
      { jsonrpc: "2.0", id: 1, method: "initialize", params: {
        protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "tua-extension", version: "1" },
      } },
      { jsonrpc: "2.0", method: "notifications/initialized" },
      { jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "tua_project_summary", arguments: {} } },
      { jsonrpc: "2.0", id: 3, method: "shutdown" },
    ]) child.stdin.write(JSON.stringify(request) + "\n");
    child.stdin.end();
  });
}

function shellQuote(value) {
  return `'${value.replace(/'/g, `'"'"'`)}'`;
}

async function configureCodex(binary, folder) {
  const name = codexName(folder);
  const args = codexArgs(binary, folder);
  try {
    const existing = JSON.parse((await execCodex(["mcp", "get", name, "--json"])).stdout);
    const transport = existing.transport || existing;
    if (transport.command === args[0] && JSON.stringify(transport.args) === JSON.stringify(args.slice(1))) {
      return { name, alreadyConfigured: true };
    }
    throw new Error(`Codex already has a different server named ${name}; remove it manually before setting up Tua`);
  } catch (error) {
    if (error.code === "ENOENT") throw new Error("Codex CLI was not found on PATH");
    if (!error.stderr?.includes("No MCP server named")) throw error;
  }
  await execCodex(["mcp", "add", name, "--", ...args]);
  const configured = JSON.parse((await execCodex(["mcp", "get", name, "--json"])).stdout);
  const transport = configured.transport || configured;
  if (transport.command !== binary) throw new Error("Codex did not retain the Tua MCP command");
  return { name, alreadyConfigured: false };
}

function workspaceFolders() {
  if (!vscode.workspace.isTrusted) return [];
  return (vscode.workspace.workspaceFolders || []).filter(folder => folder.uri.scheme === "file");
}

function registerAgentTools(context) {
  const disposables = [];
  disposables.push(vscode.commands.registerCommand(COMMANDS.setupAgentTools, async () => {
    try {
      const folders = workspaceFolders();
      if (!folders.length) throw new Error("Open and trust a local Tua project first");
      const folder = folders.length === 1 ? folders[0] : await vscode.window.showQuickPick(
        folders.map(item => ({ label: item.name, description: item.uri.fsPath, folder: item })),
        { placeHolder: "Choose the project to expose to Codex" },
      ).then(item => item?.folder);
      if (!folder) return;
      const binary = installCli(context);
      await probeMcp(binary, folder);
      try {
        const result = await configureCodex(binary, folder);
        void vscode.window.showInformationMessage(
          `Tua tools are ${result.alreadyConfigured ? "already available" : "configured"} for Codex in ${folder.name}. CLI: ${binary}`,
          "Copy CLI Path",
        ).then(choice => choice === "Copy CLI Path" && vscode.env.clipboard.writeText(binary));
      } catch (error) {
        const command = ["codex", "mcp", "add", codexName(folder), "--", ...codexArgs(binary, folder)]
          .map(shellQuote).join(" ");
        await vscode.env.clipboard.writeText(command);
        throw new Error(`${error.message}. The setup command was copied to your clipboard; CLI: ${binary}`);
      }
    } catch (error) {
      void vscode.window.showErrorMessage(`Tua agent setup: ${error.message || error}`);
    }
  }));

  if (typeof vscode.lm?.registerMcpServerDefinitionProvider === "function"
      && typeof vscode.McpStdioServerDefinition === "function") {
    const changed = new vscode.EventEmitter();
    const provider = {
      onDidChangeMcpServerDefinitions: changed.event,
      provideMcpServerDefinitions: () => {
        const binary = bundledCli(context);
        if (!binary) return [];
        return workspaceFolders().map(folder => {
          const definition = new vscode.McpStdioServerDefinition(
            `Tua (${folder.name})`, binary, ["mcp", "--stdio", "--workspace", folder.uri.fsPath],
            {}, context.extension.packageJSON.version,
          );
          definition.cwd = folder.uri;
          return definition;
        });
      },
    };
    disposables.push(changed);
    disposables.push(vscode.lm.registerMcpServerDefinitionProvider(PROVIDER_ID, provider));
    disposables.push(vscode.workspace.onDidChangeWorkspaceFolders(() => changed.fire()));
    if (typeof vscode.workspace.onDidGrantWorkspaceTrust === "function") {
      disposables.push(vscode.workspace.onDidGrantWorkspaceTrust(() => changed.fire()));
    }
  }
  return vscode.Disposable.from(...disposables);
}

module.exports = { registerAgentTools, bundledCli, stableCli, installCli, codexName, codexArgs, configureCodex, probeMcp };
