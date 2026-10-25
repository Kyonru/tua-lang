"use strict";
const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { activeProjectLayout, configuredLoveCommand } = require("../love/run-project");
const { findDebuggerExtension, hasDebuggerSourceMap, DEBUGGER_EXTENSION_ID } = require("../love/debug-project");
const { probeVersion, emitMapsSetting } = require("./check-helpers");

function registerProjectChecks({ getClient, getServerCommand }) {
  const changed = new vscode.EventEmitter();
  let rows = [];
  let serial = 0;
  const view = vscode.window.createTreeView("tua.checks", { treeDataProvider: {
    onDidChangeTreeData: changed.event,
    getChildren: () => rows,
    getTreeItem(row) {
      const item = new vscode.TreeItem(row.label);
      item.id = row.id;
      item.description = row.detail;
      item.tooltip = `${row.detail}${row.action ? `\n${row.action.title}` : ""}`;
      item.iconPath = new vscode.ThemeIcon(row.ok ? "pass" : "warning");
      item.contextValue = row.action ? "tuaSetupFinding" : "tuaSetupResult";
      if (row.action) item.command = { command: "tua.checks.fix", title: row.action.title, arguments: [row] };
      return item;
    },
  } });
  view.message = "Run setup checks to inspect compiler, LOVE, debugger, and source maps.";
  const action = (title, command, ...args) => ({ title, command, args });
  const run = async () => {
    const request = ++serial;
    view.message = "Checking project…";
    const results = [];
    if (!vscode.workspace.isTrusted) {
      results.push({ id: "trust", label: "Workspace trust", detail: "Executable checks require a trusted workspace", action: action("Manage Workspace Trust", "workbench.trust.manage") });
    } else {
      const [compiler, love] = await Promise.all([probeVersion(getServerCommand()), probeVersion(configuredLoveCommand())]);
      results.push({ id: "compiler", label: "Compiler", ...compiler, action: compiler.ok ? undefined : action("Select compiler", COMMANDS.selectServer) });
      results.push({ id: "love", label: "LOVE", ...love, action: love.ok ? undefined : action("Choose LOVE executable", "tua.checks.chooseLove") });
    }
    results.push({ id: "server", label: "Language server", ok: Boolean(getClient()), detail: getClient() ? "Running" : "Not running", action: getClient() ? undefined : action("Restart language server", COMMANDS.restartServer) });
    const debuggerExtension = findDebuggerExtension();
    results.push({ id: "debugger", label: "Lua debugger", ok: Boolean(debuggerExtension), detail: debuggerExtension ? "Installed" : "Optional — needed for Debug", action: debuggerExtension ? undefined : action("Install Second Local Lua Debugger", "workbench.extensions.installExtension", DEBUGGER_EXTENSION_ID) });
    try {
      const layout = await activeProjectLayout(getClient);
      if (!layout?.configPath) results.push({ id: "project", label: "Project configuration", detail: "Open a file from a Tua project", action: action("Open Explorer", "workbench.view.explorer") });
      else {
        const document = await vscode.workspace.openTextDocument(vscode.Uri.file(layout.configPath));
        const maps = emitMapsSetting(document.getText());
        results.push({ id: "maps", label: "Source-map configuration", ok: maps.enabled, detail: maps.enabled ? "Enabled" : "emit_maps is false", action: maps.enabled ? undefined : action("Enable source maps", "tua.checks.enableMaps", layout.configPath) });
        const emitted = hasDebuggerSourceMap(layout.outputDirectory);
        results.push({ id: "map-output", label: "Debugger source maps", ok: emitted, detail: emitted ? "Generated maps found" : "Build with source maps before debugging", action: emitted ? undefined : action("Build project", "tua.checks.build") });
      }
    } catch (error) {
      results.push({ id: "project", label: "Project configuration", detail: error.message, action: action("Open Tua settings", COMMANDS.openSettings) });
    }
    if (request === serial) { rows = results; view.message = undefined; changed.fire(); }
    return results;
  };
  return vscode.Disposable.from(view, changed,
    { dispose() { serial++; } },
    vscode.commands.registerCommand("tua.checks.run", run),
    vscode.commands.registerCommand("tua.checks.build", () => vscode.commands.executeCommand("tua.buildProject")),
    vscode.commands.registerCommand("tua.checks.fix", async requested => {
      const row = rows.find(item => item.id === requested?.id);
      if (row?.action) return vscode.commands.executeCommand(row.action.command, ...row.action.args);
    }),
    vscode.commands.registerCommand("tua.checks.chooseLove", async () => {
      const picked = await vscode.window.showOpenDialog({ canSelectFiles: true, canSelectFolders: false, canSelectMany: false, title: "Choose the LOVE executable" });
      if (picked?.[0]) { await vscode.workspace.getConfiguration("tua").update("love.command", picked[0].fsPath, vscode.ConfigurationTarget.Global); return run(); }
    }),
    vscode.commands.registerCommand("tua.checks.enableMaps", async configPath => {
      if (!configPath || !vscode.workspace.isTrusted) return;
      const document = await vscode.workspace.openTextDocument(vscode.Uri.file(configPath));
      const setting = emitMapsSetting(document.getText());
      if (setting.enabled) return;
      const edit = new vscode.WorkspaceEdit();
      edit.replace(document.uri, new vscode.Range(document.positionAt(setting.start), document.positionAt(setting.end)), "true");
      if (await vscode.workspace.applyEdit(edit)) await vscode.window.showTextDocument(document);
    }),
  );
}

module.exports = { registerProjectChecks };
