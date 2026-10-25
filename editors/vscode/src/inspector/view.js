"use strict";
const path = require("node:path");
const vscode = require("vscode");
const { normalizeOptions, CATEGORIES } = require("./options");
const { InspectorSessions } = require("./sessions");
const { matchingInstances } = require("../editor/declaration-target");
const { RuntimeOrigins } = require("./origins");
const { loadSourceMapForGeneratedPath } = require("../editor/source-maps");
const { valueNodes, storeNodes, presentObject } = require("./presentation");
const GROUPS = [["ecs", "Entities and components", "symbol-struct"], ["states", "State machines", "type-hierarchy"], ["signals", "Signal activity", "radio-tower"], ["pools", "Object pools", "database"], ["stores", "Stores", "symbol-variable"]];

class InspectorTree {
  constructor(sessions) {
    this.sessions = sessions; this.changed = new vscode.EventEmitter();
    this.onDidChangeTreeData = this.changed.event;
  }
  entry() {
    return this.sessions.entries.get(this.selected) || [...this.sessions.entries.values()].find(entry => entry.session.active) || [...this.sessions.entries.values()].at(-1);
  }
  getParent(node) {
    return this.getChildren().find(group => group.children?.some(child => child.id === node.id));
  }
  getTreeItem(node) {
    const item = new vscode.TreeItem(node.label, node.children?.length ? (node.expanded ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.Collapsed) : vscode.TreeItemCollapsibleState.None);
    item.id = node.id; item.description = node.description;
    item.tooltip = node.tooltip || node.description || node.label;
    if (node.origin) item.contextValue = "tuaRuntimeObject.source";
    if (node.icon) item.iconPath = new vscode.ThemeIcon(node.icon);
    if (node.command) item.command = { command: node.command, title: node.label, ...(node.arguments ? { arguments: node.arguments } : {}) };
    return item;
  }
  getChildren(node) {
    if (node) return node.children || [];
    const entry = this.entry();
    if (!entry) return [];
    const prefix = `inspector:${entry.session.id}`;
    if (!entry.snapshot) return [{ id: prefix + ":waiting", label: entry.status === "unavailable" ? "Inspection could not start" : "Waiting for runtime connection", description: entry.session.title, icon: "info" }];
    const result = GROUPS.filter(([kind]) => entry.snapshot.objects.some(object => object.kind === kind)).map(([kind, label, icon]) => {
      const objects = entry.snapshot.objects.filter(object => object.kind === kind).sort((a, b) => a.id - b.id);
      return { id: prefix + ":" + kind, label, icon, expanded: true, description: String(objects.length),
        children: objects.map(object => {
          const id = prefix + ":object:" + object.id;
          const origin = object.origin;
          const { summary, children, details } = presentObject(object, id,
            entry.previousSnapshot?.objects.find(previous => previous.kind === kind && previous.id === object.id)?.data);
          const name = origin?.name || object.label;
          const duplicate = objects.some(other => other.id !== object.id && (other.origin?.name || other.label) === name);
          if (origin) details.unshift({ id: id + ":source", label: "Creation site", description: `${origin.file}:${origin.line}`, tooltip: `${origin.path}:${origin.line}`, icon: "go-to-file", command: "tua.inspector.goToSource", arguments: [{ origin }] });
          details.push({ id: id + ":runtimeId", label: "Runtime ID", description: String(object.id) });
          children.push({ id: id + ":details", label: "Details", icon: "info", children: details });
          return { id, label: duplicate ? `${name} #${object.id}` : name, origin,
            description: summary,
            tooltip: origin ? `${origin.name || object.label} (#${object.id})\nCreated at ${origin.path}:${origin.line}${origin.generated ? " (generated Lua; no source mapping available)" : ""}` : `${object.label}\nCreation source unavailable`,
            children };
        }),
      };
    });
    if (!result.length) result.push({ id: prefix + ":empty", label: "No runtime objects yet", description: "Waiting for supported objects", icon: "info" });
    if (entry.snapshot.truncated) result.push({ id: prefix + ":limited", label: "Snapshot limits reached", description: "Large values are abbreviated", icon: "info" });
    return result;
  }
  dispose() { this.changed.dispose(); }
}

function inspectorToolbarState(entry) {
  return { canControl: Boolean(entry?.session.active && entry.connection && entry.status === "connected"), paused: Boolean(entry?.paused) };
}

function registerRuntimeInspector(context, activity) {
  const enabled = () => vscode.workspace.isTrusted && vscode.workspace.getConfiguration("tua").get("runtimeInspector.enabled", false);
  const options = () => { const config = vscode.workspace.getConfiguration("tua"); return normalizeOptions({ categories: config.get("runtimeInspector.categories"), updateInterval: config.get("runtimeInspector.updateInterval", 500) }); };
  const origins = new RuntimeOrigins(loadSourceMapForGeneratedPath);
  const sessions = new InspectorSessions({ activity, enabled, options, describeObject: (entry, object) => origins.describe(entry, object),
    assetsDirectory: path.join(context.extensionPath, "assets", "inspector"),
    reportError: message => vscode.window.showWarningMessage(message),
  });
  const tree = new InspectorTree(sessions);
  const view = vscode.window.createTreeView("tua.inspector", { treeDataProvider: tree, showCollapseAll: true });
  let toolbarState;
  const refresh = (updateTree = true) => {
    const entry = tree.entry();
    const nextState = inspectorToolbarState(entry);
    if (toolbarState?.canControl !== nextState.canControl) void vscode.commands.executeCommand("setContext", "tua.inspectorCanControl", nextState.canControl);
    if (toolbarState?.paused !== nextState.paused) void vscode.commands.executeCommand("setContext", "tua.inspectorPaused", nextState.paused);
    toolbarState = nextState;
    view.description = entry?.session.title;
    if (entry) {
      const age = entry.received ? Math.floor((Date.now() - entry.received) / 1000) : undefined;
      const status = !enabled() ? "Inspection disabled" : !entry.session.active ? "Ended · last snapshot" : entry.paused ? "Updates paused" : entry.status === "connected" ? "Live" : entry.status === "waiting" ? "Connecting" : entry.status;
      view.message = `${status}${age === undefined || (status === "Live" && age < 2) ? "" : ` · updated ${age}s ago`}`;
    } else view.message = enabled() ? (options().categories.length ? "Launch a Tua run to inspect live runtime objects." : "No inspection categories selected. Use Configure Runtime Inspection.") : undefined;
    if (updateTree) tree.changed.fire();
  };
  const timer = setInterval(() => { if (view.visible) refresh(false); }, 1000); timer.unref();
  const setEnabled = async value => {
    if (!vscode.workspace.isTrusted) return vscode.window.showWarningMessage("Trust this workspace before enabling runtime inspection.");
    await vscode.workspace.getConfiguration("tua").update("runtimeInspector.enabled", value,
      vscode.workspace.workspaceFolders?.length ? vscode.ConfigurationTarget.Workspace : vscode.ConfigurationTarget.Global);
    if (!value) sessions.disconnectAll();
    refresh();
  };
  const disposables = [tree, view, sessions, sessions.subscribe(refresh),
    vscode.workspace.onDidChangeConfiguration(event => { if (event.affectsConfiguration("tua.runtimeInspector.updateInterval")) sessions.updateInterval(options().updateInterval); if (event.affectsConfiguration("tua.runtimeInspector.categories")) refresh(); if (event.affectsConfiguration("tua.runtimeInspector.enabled")) { if (!enabled()) sessions.disconnectAll(); refresh(); } }),
    vscode.commands.registerCommand("tua.inspector.configure", async () => {
      const current = options();
      const labels = Object.fromEntries(GROUPS.map(([key, label]) => [key, label]));
      const picked = await vscode.window.showQuickPick(CATEGORIES.map(kind => ({ label: labels[kind], kind, picked: current.categories.includes(kind) })), { canPickMany: true, title: "Runtime Inspector categories", placeHolder: "Only selected categories are instrumented in new runs" });
      if (!picked) return;
      const choices = [100, 200, 500, 1000, 2000, 5000, 10000];
      if (!choices.includes(current.updateInterval)) choices.push(current.updateInterval);
      const rate = await vscode.window.showQuickPick(choices.sort((a, b) => a - b).map(value => ({ label: `${value} ms · ${1000 / value} updates/s`, description: value === current.updateInterval ? "Current" : value === 500 ? "Default" : undefined, value })), { title: "Inspector update interval", placeHolder: "Applies to connected players immediately; faster updates cost more" });
      if (!rate) return;
      const scope = vscode.workspace.workspaceFolders?.length ? vscode.ConfigurationTarget.Workspace : vscode.ConfigurationTarget.Global;
      const config = vscode.workspace.getConfiguration("tua");
      await config.update("runtimeInspector.categories", picked.map(item => item.kind), scope);
      await config.update("runtimeInspector.updateInterval", rate.value, scope);
      await vscode.window.showInformationMessage("Inspection settings saved. Restart running games to apply category changes; update frequency applies immediately.");
    }),
    vscode.commands.registerCommand("tua.inspector.enable", () => setEnabled(true)),
    vscode.commands.registerCommand("tua.inspector.disable", () => setEnabled(false)),
    vscode.commands.registerCommand("tua.inspector.goToSource", async node => {
      if (!node?.origin?.path || !Number.isSafeInteger(node.origin.line) || node.origin.line < 1) return;
      try {
        const document = await vscode.workspace.openTextDocument(vscode.Uri.file(node.origin.path));
        const line = Math.min(node.origin.line - 1, document.lineCount - 1);
        const character = Math.min(node.origin.character || 0, document.lineAt(line).text.length);
        await vscode.window.showTextDocument(document, { preview: true, selection: new vscode.Range(line, character, line, character) });
      } catch { await vscode.window.showInformationMessage("The creation source is no longer available. Start a new run to refresh its location."); }
    }),
    vscode.commands.registerCommand("tua.inspector.forDeclaration", async target => {
      if (!target?.path || !target.kind) return;
      if (!enabled()) {
        const action = await vscode.window.showInformationMessage("Runtime inspection is disabled. Enable it, then start a new Tua run to inspect this declaration.", "Enable Runtime Inspection");
        if (action) await setEnabled(true);
        await vscode.commands.executeCommand("tua.inspector.focus");
        return;
      }
      const matches = matchingInstances(sessions.entries, target);
      if (!matches.length) {
        await vscode.window.showInformationMessage(`No connected instance of ${target.name} matches this source location. Start or restart an inspected run with source maps enabled.`);
        return;
      }
      const selected = matches.length === 1 ? matches[0] : (await vscode.window.showQuickPick(matches.map(match => ({
        label: `${match.entry.session.title} · ${target.name} #${match.object.id}`, description: match.object.origin.file, match,
      })), { placeHolder: "Choose a runtime instance" }))?.match;
      if (!selected) return;
      // Recheck after the picker: the player may have stopped in the meantime.
      if (!matchingInstances(sessions.entries, target).some(match => match.entry === selected.entry && match.object.id === selected.object.id)) return;
      tree.selected = selected.entry.session.id;
      refresh();
      await vscode.commands.executeCommand("tua.inspector.focus");
      const node = tree.getChildren().flatMap(group => group.children || []).find(item => item.id === `inspector:${tree.selected}:object:${selected.object.id}`);
      if (node) await view.reveal(node, { select: true, focus: true, expand: true });
    }),
    vscode.commands.registerCommand("tua.inspector.open", () => vscode.commands.executeCommand("tua.inspector.focus")),
    vscode.commands.registerCommand("tua.inspector.select", async () => {
      const items = [...sessions.entries.values()].map(entry => ({ label: entry.session.title, description: entry.session.active ? entry.status : "Ended", entry }));
      if (!items.length) return vscode.window.showInformationMessage("Enable inspection, then launch a Tua run.");
      const picked = await vscode.window.showQuickPick(items, { placeHolder: "Choose a player to inspect" });
      if (picked) { tree.selected = picked.entry.session.id; refresh(); }
    }),
    vscode.commands.registerCommand("tua.inspector.pause", () => sessions.pause(tree.entry()?.session.id, true)),
    vscode.commands.registerCommand("tua.inspector.resume", () => sessions.pause(tree.entry()?.session.id, false)),
    vscode.commands.registerCommand("tua.inspector.refresh", () => sessions.refresh(tree.entry()?.session.id)),
    { dispose() { clearInterval(timer); void vscode.commands.executeCommand("setContext", "tua.inspectorCanControl", false); void vscode.commands.executeCommand("setContext", "tua.inspectorPaused", false); } },
  ];
  refresh();
  return { sessions, dispose: () => vscode.Disposable.from(...disposables).dispose() };
}
module.exports = { InspectorTree, valueNodes, storeNodes, inspectorToolbarState, registerRuntimeInspector };
