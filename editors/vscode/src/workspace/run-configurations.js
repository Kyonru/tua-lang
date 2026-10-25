"use strict";
const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { DEFAULT_CONFIGURATIONS, normalizeConfigurations } = require("./run-options");
const SELECTED = "tua.run.selectedConfiguration";
const MODES = { run: COMMANDS.runLoveProject, debug: COMMANDS.debugLoveProject, hotReload: COMMANDS.runLoveProjectWithHotReload, package: COMMANDS.packageGame };

function registerRunConfigurations(context) {
  const changed = new vscode.EventEmitter();
  let configurations = [];
  let error;
  const selected = () => configurations.find(item => item.name === context.workspaceState.get(SELECTED)) || configurations[0];
  const view = vscode.window.createTreeView("tua.run", { treeDataProvider: {
    onDidChangeTreeData: changed.event,
    getChildren: () => configurations,
    getTreeItem(configuration) {
      const item = new vscode.TreeItem(configuration.name);
      item.id = configuration.name;
      item.description = configuration.mode === "hotReload" ? "hot reload" : configuration.mode;
      item.iconPath = new vscode.ThemeIcon(configuration === selected() ? "check" : "circle-outline");
      item.contextValue = "tuaRunConfiguration";
      item.command = { command: "tua.run.selectConfiguration", title: "Select configuration", arguments: [configuration] };
      return item;
    },
  } });
  const refresh = () => {
    try { configurations = normalizeConfigurations(vscode.workspace.getConfiguration("tua").get("run.configurations", DEFAULT_CONFIGURATIONS)); error = undefined; }
    catch (failure) { configurations = []; error = failure.message; }
    view.description = selected()?.name;
    view.message = error;
    changed.fire();
  };
  const choose = async requested => {
    const picked = requested ? configurations.find(item => item.name === requested.name) : await vscode.window.showQuickPick(
      configurations.map(item => ({ ...item, label: item.name, description: item.mode, picked: item === selected() })),
      { title: "Tua run configuration", placeHolder: "Choose the action for the Run button" },
    );
    if (picked) { await context.workspaceState.update(SELECTED, picked.name); refresh(); }
    return picked;
  };
  const run = async requested => {
    const configuration = requested ? await choose(requested) : selected();
    if (!configuration) return vscode.window.showWarningMessage(error || "Choose a Tua run configuration first.");
    await vscode.commands.executeCommand("tua.console.open");
    return vscode.commands.executeCommand(MODES[configuration.mode], { console: true, ...(configuration.mode === "package" ? { target: configuration.packageTarget } : {}) });
  };
  refresh();
  return vscode.Disposable.from(changed, view,
    vscode.commands.registerCommand("tua.run.select", () => choose()),
    vscode.commands.registerCommand("tua.run.selectConfiguration", requested => choose(requested)),
    vscode.commands.registerCommand("tua.run.selected", () => run()),
    vscode.commands.registerCommand("tua.run.configuration", run),
    vscode.commands.registerCommand("tua.run.configure", () => vscode.commands.executeCommand("workbench.action.openSettings", "tua.run.configurations")),
    vscode.workspace.onDidChangeConfiguration(event => { if (event.affectsConfiguration("tua.run.configurations")) refresh(); }),
  );
}

module.exports = { registerRunConfigurations };
