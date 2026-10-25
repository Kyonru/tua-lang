"use strict";
const vscode = require("vscode");
const { ConsoleBuffer } = require("./console-buffer");

function registerGameConsole(store) {
  const buffer = new ConsoleBuffer();
  const changed = new vscode.EventEmitter();
  const filters = {};
  let timer;
  const view = vscode.window.createTreeView("tua.console", { treeDataProvider: {
    onDidChangeTreeData: changed.event,
    getChildren: () => buffer.filtered(filters),
    getTreeItem(entry) {
      const item = new vscode.TreeItem(entry.text);
      item.id = entry.id;
      item.description = `${entry.count > 1 ? `×${entry.count} · ` : ""}${entry.module}`;
      item.tooltip = `${new Date(entry.time).toLocaleTimeString()} · ${entry.session.title || entry.session.kind} · ${entry.severity}\n${entry.text}`;
      item.iconPath = new vscode.ThemeIcon(entry.severity === "error" ? "error" : entry.severity === "warn" ? "warning" : "output",
        entry.severity === "error" ? new vscode.ThemeColor("errorForeground") : entry.severity === "warn" ? new vscode.ThemeColor("list.warningForeground") : undefined);
      item.contextValue = "tuaConsoleEntry";
      if (entry.location) item.command = { command: "vscode.open", title: "Open Tua source", arguments: [vscode.Uri.file(entry.location.path), {
        preview: true, selection: new vscode.Range(entry.location.line, entry.location.character, entry.location.line, entry.location.character),
      }] };
      return item;
    },
  } });
  const refresh = () => {
    const entries = buffer.filtered(filters);
    view.description = [filters.severity || "all levels", filters.module, filters.session && "one session"].filter(Boolean).join(" · ");
    view.message = entries.length ? undefined : buffer.entries.length ? "No messages match these filters." : "Run a Tua project to capture its output here.";
    changed.fire();
  };
  const onOutput = ({ session, line, options }) => {
    buffer.append(session, line, options);
    if (!timer) timer = setTimeout(() => { timer = undefined; refresh(); }, 100);
  };
  store.events.on("output", onOutput);
  refresh();
  return vscode.Disposable.from(changed, view,
    { dispose() { clearTimeout(timer); store.events.off("output", onOutput); } },
    vscode.commands.registerCommand("tua.console.open", () => vscode.commands.executeCommand("tua.console.focus")),
    vscode.commands.registerCommand("tua.console.clear", () => { buffer.clear(); refresh(); }),
    vscode.commands.registerCommand("tua.console.copy", entry => entry && vscode.env.clipboard.writeText(entry.text)),
    vscode.commands.registerCommand("tua.console.level", async () => {
      const choice = await vscode.window.showQuickPick(["All levels", "error", "warn", "info", "debug", "trace"], { title: "Console severity" });
      if (choice) { filters.severity = choice === "All levels" ? undefined : choice; refresh(); }
    }),
    vscode.commands.registerCommand("tua.console.module", async () => {
      const choices = [{ label: "All modules", value: undefined }, ...[...new Set(buffer.entries.map(entry => entry.module))].sort().map(value => ({ label: value, value }))];
      const choice = await vscode.window.showQuickPick(choices, { title: "Console module" });
      if (choice) { filters.module = choice.value; refresh(); }
    }),
    vscode.commands.registerCommand("tua.console.session", async () => {
      const choice = await vscode.window.showQuickPick([{ label: "All sessions" }, ...store.sessions.map(session => ({ label: session.title || session.kind, description: `${session.label} · ${new Date(session.started).toLocaleTimeString()}`, id: session.id }))], { title: "Console session" });
      if (choice) { filters.session = choice.id; refresh(); }
    }),
  );
}

module.exports = { registerGameConsole };
