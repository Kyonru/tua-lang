"use strict";
const path = require("path");
const vscode = require("vscode");
const { ActivityStore } = require("./activity-store");
const { isPathInside } = require("../editor/project-layout");

function registerRunActivity() {
  const store = new ActivityStore();
  const changed = new vscode.EventEmitter();
  const provider = {
    onDidChangeTreeData: changed.event,
    getChildren: () => store.sessions,
    getTreeItem(session) {
      const item = new vscode.TreeItem(session.title || session.kind);
      item.id = session.id;
      item.description = session.label;
      item.tooltip = `${session.cwd}\n${session.label}\n${new Date(session.started).toLocaleString()}`;
      item.iconPath = new vscode.ThemeIcon(sessionIcon(session));
      item.contextValue = session.active && session.stop ? "tuaSession.active" : "tuaSession.finished";
      item.command = { command: "tua.activity.show", title: "Inspect session", arguments: [session] };
      return item;
    },
  };
  const view = vscode.window.createTreeView("tua.sessions", { treeDataProvider: provider });
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 20);
  status.name = "Tua run status";
  status.command = "tua.activity.show";
  const update = () => {
    void vscode.commands.executeCommand("setContext", "tua.hasActiveSession", store.sessions.some(session => session.active));
    const latest = store.sessions[0];
    changed.fire();
    view.message = latest ? undefined : "Run a Tua project to see build and reload status.";
    if (!latest) return status.hide();
    status.text = `$(${sessionIcon(latest)}) Tua: ${latest.label}`;
    status.tooltip = `${latest.title || latest.kind} · ${latest.cwd}\nClick for affected modules and diagnostics`;
    status.backgroundColor = ["failed", "previous", "reloadFailed", "runtimeFailed"].includes(latest.phase) ? new vscode.ThemeColor("statusBarItem.warningBackground") : undefined;
    status.show();
  };
  update();
  const disposable = vscode.Disposable.from(store, changed, view, status, store.subscribe(update),
    vscode.commands.registerCommand("tua.activity.show", async selected => {
      const session = selected || store.sessions[0];
      if (!session) return vscode.commands.executeCommand("tua.sessions.focus");
      const items = [{ label: "Open Problems", description: session.label, command: "workbench.actions.view.problems" }];
      for (const [uri, diagnostics] of vscode.languages.getDiagnostics()) {
        if (uri.scheme !== "file" || !isPathInside(session.cwd, uri.fsPath)) continue;
        for (const diagnostic of diagnostics) items.push({ label: `${path.basename(uri.fsPath)}:${diagnostic.range.start.line + 1}`, description: diagnostic.message, uri, range: diagnostic.range });
      }
      for (const name of session.modules) items.push({ label: name, description: "Reloaded module", command: "tua.showModuleGraph" });
      const picked = await vscode.window.showQuickPick(items, { title: `${session.title || session.kind}: ${session.label}`, placeHolder: session.cwd });
      if (picked?.uri) return vscode.window.showTextDocument(picked.uri, { selection: picked.range, preview: true });
      if (picked?.command) return vscode.commands.executeCommand(picked.command);
    }),
    vscode.commands.registerCommand("tua.activity.stop", selected => {
      const sessions = selected ? [selected] : store.sessions.filter(session => session.active);
      for (const session of sessions) session.stop?.();
    }),
  );
  return { store, dispose: () => disposable.dispose() };
}

function sessionIcon(session) {
  if (session.phase === "building" || session.phase === "reloading") return "sync~spin";
  if (["failed", "previous", "reloadFailed", "runtimeFailed"].includes(session.phase)) return "warning";
  if (session.active) return "debug-start";
  return session.phase === "stopped" ? "debug-stop" : "pass";
}

module.exports = { registerRunActivity };
