"use strict";

const path = require("path");
const vscode = require("vscode");
const { COMMANDS } = require("../constants");

const SECTIONS = [
  ["files", "Project files", "files"],
  ["modules", "Modules", "package", "tua/moduleGraph", COMMANDS.showModuleGraph],
  ["signals", "Signals", "radio-tower", "tua/signalGraph", COMMANDS.showSignalGraph],
  ["ecs", "ECS", "symbol-struct", "tua/ecsGraph", COMMANDS.showEcsExplorer],
];

function registerWorkspaceSidebar({ getClient }) {
  const provider = new WorkspaceTree(getClient);
  const view = vscode.window.createTreeView("tua.workspace", { treeDataProvider: provider, showCollapseAll: true });
  const refresh = () => provider.refresh();
  return vscode.Disposable.from(provider, view,
    vscode.commands.registerCommand("tua.workspace.open", () => vscode.commands.executeCommand("workbench.view.extension.tua")),
    vscode.commands.registerCommand("tua.workspace.refresh", refresh),
    vscode.commands.registerCommand("tua.workspace.graph", node => node?.graphCommand && vscode.commands.executeCommand(node.graphCommand)),
    ...[["sprites", COMMANDS.previewSprites], ["states", COMMANDS.previewStateMachine], ["generated", COMMANDS.openGeneratedLua]].map(([name, command]) =>
      vscode.commands.registerCommand(`tua.workspace.${name}`, async node => {
        if (node?.uri) await vscode.window.showTextDocument(vscode.Uri.parse(node.uri), { preview: true });
        return vscode.commands.executeCommand(command);
      })),
    vscode.workspace.onDidSaveTextDocument(refresh),
    vscode.workspace.onDidChangeWorkspaceFolders(refresh),
  );
}

class WorkspaceTree {
  constructor(getClient) {
    this.getClient = getClient;
    this.changed = new vscode.EventEmitter();
    this.onDidChangeTreeData = this.changed.event;
    this.cache = new Map();
  }
  refresh() { this.cache.clear(); this.changed.fire(); }
  dispose() { this.cache.clear(); this.changed.dispose(); }
  getTreeItem(node) {
    const item = new vscode.TreeItem(node.label, node.children?.length || node.kind ? vscode.TreeItemCollapsibleState.Collapsed : vscode.TreeItemCollapsibleState.None);
    item.id = node.id;
    item.description = node.description;
    item.tooltip = node.tooltip || node.label;
    item.contextValue = node.graphCommand ? "tuaWorkspace.graph" : node.uri?.endsWith(".tua") ? "tuaWorkspace.source" : "tuaWorkspace.item";
    if (node.uri) item.resourceUri = vscode.Uri.parse(node.uri);
    if (node.icon) item.iconPath = new vscode.ThemeIcon(node.icon);
    if (node.command) item.command = { command: node.command, title: node.label };
    else if (node.uri && node.kind !== "directory") {
      const options = { preview: true };
      if (node.range) options.selection = new vscode.Range(node.range.start.line, node.range.start.character, node.range.end.line, node.range.end.character);
      item.command = { command: "vscode.open", title: "Open source", arguments: [vscode.Uri.parse(node.uri), options] };
    }
    return item;
  }
  async getChildren(node) {
    if (!node) return [
      ...SECTIONS.map(([kind, label, icon, request, graphCommand]) => ({ id: kind, kind, label, icon, request, graphCommand })),
      { id: "pools", label: "Object pools", icon: "database", command: COMMANDS.showObjectPoolExplorer },
      { id: "audio", label: "Audio", icon: "unmute", command: COMMANDS.showAudioExplorer },
      { id: "setup", label: "Editor setup", icon: "settings-gear", command: COMMANDS.openSetup },
    ];
    if (node.children) return node.children;
    if (node.kind === "files") return (vscode.workspace.workspaceFolders || []).map(folder => ({ id: folder.uri.toString(), uri: folder.uri.toString(), label: folder.name, kind: "directory", icon: "root-folder" }));
    try {
      if (node.kind === "directory") {
        const entries = await vscode.workspace.fs.readDirectory(vscode.Uri.parse(node.uri));
        return entries.filter(([name, type]) => !(type & vscode.FileType.SymbolicLink) && !name.startsWith(".") &&
          !["node_modules", "build", "dist", "target"].includes(name) && (type === vscode.FileType.Directory || /\.(tua|toml)$/i.test(name)))
          .sort((a, b) => (b[1] === vscode.FileType.Directory) - (a[1] === vscode.FileType.Directory) || a[0].localeCompare(b[0], undefined, { numeric: true }))
          .map(([name, type]) => {
            const uri = vscode.Uri.joinPath(vscode.Uri.parse(node.uri), name).toString();
            return { id: uri, uri, label: name, kind: type === vscode.FileType.Directory ? "directory" : undefined };
          });
      }
      const client = this.getClient();
      if (!client) return [{ id: `${node.id}:offline`, label: "Start the language server", icon: "debug-restart", command: COMMANDS.restartServer }];
      if (client !== this.client) { this.cache.clear(); this.client = client; }
      if (!this.cache.has(node.kind)) this.cache.set(node.kind, client.sendRequest(node.request, null));
      const response = await this.cache.get(node.kind);
      return graphNodes(node.kind, response);
    } catch (error) {
      this.cache.delete(node.kind);
      return [{ id: `${node.id}:error`, label: "Could not load — refresh to retry", tooltip: String(error.message || error), icon: "warning", command: "tua.workspace.refresh" }];
    }
  }
}

function graphNodes(kind, response) {
  const source = (item, label, prefix) => ({ id: `${prefix}:${item.id || item.uri}:${label}`, label, uri: item.uri, range: item.nameRange || item.range });
  let nodes = [];
  if (kind === "modules") nodes = (response?.graph?.modules || []).map(item => ({
    ...source(item, path.basename(vscode.Uri.parse(item.uri).fsPath), kind),
    description: `${item.dependencyCount || 0} dependencies`,
    children: (item.exports || []).map(entry => source({ ...entry, uri: item.uri }, entry.name, `export:${item.uri}`)),
  }));
  if (kind === "signals") nodes = (response?.events || []).map(item => ({
    id: `signal:${item.id || `${item.busUri}:${item.bus}:${item.name}`}`, label: item.name, description: item.bus,
    children: (item.calls || []).map((call, index) => ({ ...source(call, `${call.role || call.method} · ${path.basename(vscode.Uri.parse(call.uri).fsPath)}`, `${item.id}:${index}`), icon: "symbol-event" })),
  }));
  if (kind === "ecs") nodes = ["components", "systems", "queries"].map(group => ({
    id: `ecs:${group}`, label: group[0].toUpperCase() + group.slice(1),
    children: (response?.graph?.[group] || []).map(item => source(item, item.name || item.localName || item.runtimeName || "Anonymous", `ecs:${group}`)),
  }));
  return nodes.length ? nodes : [{ id: `${kind}:empty`, label: "No declarations found", icon: "info" }];
}

module.exports = { registerWorkspaceSidebar, WorkspaceTree, graphNodes };
