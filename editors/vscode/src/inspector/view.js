"use strict";
const path = require("node:path");
const vscode = require("vscode");
const { InspectorSessions } = require("./sessions");
const GROUPS = [["ecs", "Entities and components", "symbol-struct"], ["states", "State machines", "type-hierarchy"], ["signals", "Signal activity", "radio-tower"], ["pools", "Object pools", "database"], ["stores", "Stores", "symbol-variable"]];

class InspectorTree {
  constructor(sessions) {
    this.sessions = sessions; this.changed = new vscode.EventEmitter();
    this.onDidChangeTreeData = this.changed.event;
  }
  entry() {
    return this.sessions.entries.get(this.selected) || [...this.sessions.entries.values()].find(entry => entry.session.active) || [...this.sessions.entries.values()].at(-1);
  }
  getTreeItem(node) {
    const item = new vscode.TreeItem(node.label, node.children?.length ? vscode.TreeItemCollapsibleState.Collapsed : vscode.TreeItemCollapsibleState.None);
    item.id = node.id; item.description = node.description;
    item.tooltip = node.tooltip || node.description || node.label;
    if (node.icon) item.iconPath = new vscode.ThemeIcon(node.icon);
    if (node.command) item.command = { command: node.command, title: node.label };
    return item;
  }
  getChildren(node) {
    if (node) return node.children || [];
    const entry = this.entry();
    if (!entry) return [];
    const prefix = `inspector:${entry.session.id}`;
    if (!entry.snapshot) return [{ id: prefix + ":waiting", label: entry.status === "unavailable" ? "Inspection could not start" : "Waiting for runtime connection", description: entry.session.title, icon: "info" }];
    const result = GROUPS.map(([kind, label, icon]) => {
      const objects = entry.snapshot.objects.filter(object => object.kind === kind).sort((a, b) => a.id - b.id);
      return { id: prefix + ":" + kind, label, icon, description: String(objects.length),
        children: objects.length ? objects.map(object => ({ id: prefix + ":object:" + object.id, label: object.label,
          description: kind === "states" ? String(object.data.current || "") : kind === "signals" ? `${object.data.emitted || 0} emitted` : kind === "stores" ? `${object.data.updates || 0} updates` : undefined,
          children: object.kind === "stores"
            ? storeNodes(object.data, prefix + ":object:" + object.id, entry.previousSnapshot?.objects.find(previous => previous.kind === "stores" && previous.id === object.id)?.data)
            : valueNodes(object.data, prefix + ":object:" + object.id),
        })) : [{ id: prefix + ":" + kind + ":empty", label: "No live objects registered", icon: "info" }],
      };
    });
    if (entry.snapshot.truncated) result.push({ id: prefix + ":limited", label: "Snapshot limits reached", description: "Large values are abbreviated", icon: "info" });
    return result;
  }
  dispose() { this.changed.dispose(); }
}

function valueNodes(value, prefix, depth = 0) {
  if (!value || typeof value !== "object" || depth >= 8) return [];
  return Object.keys(value).slice(0, 128).sort((a, b) => a.localeCompare(b, undefined, { numeric: true })).map(key => {
    const child = value[key];
    const nested = child !== null && typeof child === "object";
    return { id: `${prefix}/${encodeURIComponent(key)}`, label: key,
      description: nested ? `${Object.keys(child).length} fields` : String(child),
      children: nested ? valueNodes(child, `${prefix}/${encodeURIComponent(key)}`, depth + 1) : undefined };
  });
}

// Compare only serialized state, never runtime references or volatile age counters.
function storeNodes(data, prefix, previous) {
  const nodes = valueNodes(data, prefix);
  const state = nodes.find(node => node.label === "state");
  if (state && previous?.state && data.state && typeof data.state === "object") {
    state.children = changedValueNodes(data.state, previous.state, `${prefix}/state`);
  }
  return nodes;
}

function changedValueNodes(current, previous, prefix, depth = 0) {
  if (depth >= 8) return [];
  const keys = [...new Set([...Object.keys(current || {}), ...Object.keys(previous || {})])].slice(0, 128);
  return keys.sort((a, b) => a.localeCompare(b, undefined, { numeric: true })).map(key => {
    const present = Object.hasOwn(current || {}, key), wasPresent = Object.hasOwn(previous || {}, key);
    const value = present ? current[key] : previous[key], old = previous?.[key];
    const nested = value !== null && typeof value === "object";
    const children = nested ? changedValueNodes(present ? value : {}, wasPresent && old && typeof old === "object" ? old : {}, `${prefix}/${encodeURIComponent(key)}`, depth + 1) : undefined;
    const change = !present ? "removed" : !wasPresent ? "added" : nested
      ? (old === null || typeof old !== "object" || children.some(child => child.change) ? "changed" : undefined)
      : !Object.is(value, old) ? "changed" : undefined;
    const summary = nested ? `${Object.keys(value).length} fields` : String(value);
    return { id: `${prefix}/${encodeURIComponent(key)}`, label: key, change, children,
      description: `${summary}${change ? ` · ${change}` : ""}`,
      icon: change ? (change === "added" ? "diff-added" : change === "removed" ? "diff-removed" : "diff-modified") : undefined,
      tooltip: change === "changed" && !nested ? `Previous: ${old !== null && typeof old === "object" ? "[table]" : String(old)}\nCurrent: ${summary}` : undefined };
  });
}

function inspectorToolbarState(entry) {
  return { canControl: Boolean(entry?.session.active && entry.connection && entry.status === "connected"), paused: Boolean(entry?.paused) };
}

function registerRuntimeInspector(context, activity) {
  const enabled = () => vscode.workspace.isTrusted && vscode.workspace.getConfiguration("tua").get("runtimeInspector.enabled", false);
  const sessions = new InspectorSessions({ activity, enabled,
    assetsDirectory: path.join(context.extensionPath, "assets", "inspector"),
    reportError: message => vscode.window.showWarningMessage(message),
  });
  const tree = new InspectorTree(sessions);
  const view = vscode.window.createTreeView("tua.inspector", { treeDataProvider: tree, showCollapseAll: true });
  let toolbarState;
  const refresh = () => {
    const entry = tree.entry();
    const nextState = inspectorToolbarState(entry);
    if (toolbarState?.canControl !== nextState.canControl) void vscode.commands.executeCommand("setContext", "tua.inspectorCanControl", nextState.canControl);
    if (toolbarState?.paused !== nextState.paused) void vscode.commands.executeCommand("setContext", "tua.inspectorPaused", nextState.paused);
    toolbarState = nextState;
    view.description = entry?.session.title;
    if (entry) {
      const age = entry.received ? Math.floor((Date.now() - entry.received) / 1000) : undefined;
      const status = !entry.session.active ? "Ended · last snapshot" : entry.paused ? "Updates paused" : entry.status === "connected" ? "Connected" : entry.status === "waiting" ? "Connecting" : entry.status;
      view.message = `${status}${age === undefined ? "" : ` · ${age}s since snapshot`}`;
    } else view.message = enabled() ? "Launch a Tua run to inspect live runtime objects." : undefined;
    tree.changed.fire();
  };
  const timer = setInterval(() => { if (view.visible) refresh(); }, 1000); timer.unref();
  const setEnabled = async value => {
    if (!vscode.workspace.isTrusted) return vscode.window.showWarningMessage("Trust this workspace before enabling runtime inspection.");
    await vscode.workspace.getConfiguration("tua").update("runtimeInspector.enabled", value,
      vscode.workspace.workspaceFolders?.length ? vscode.ConfigurationTarget.Workspace : vscode.ConfigurationTarget.Global);
    if (!value) sessions.disconnectAll();
    refresh();
  };
  const disposables = [tree, view, sessions, sessions.subscribe(refresh),
    vscode.workspace.onDidChangeConfiguration(event => { if (event.affectsConfiguration("tua.runtimeInspector.enabled")) { if (!enabled()) sessions.disconnectAll(); refresh(); } }),
    vscode.commands.registerCommand("tua.inspector.enable", () => setEnabled(true)),
    vscode.commands.registerCommand("tua.inspector.disable", () => setEnabled(false)),
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
