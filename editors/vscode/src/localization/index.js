"use strict";

const crypto = require("crypto");
const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { openLocationMessage } = require("../shared/locations");
const { createRenderGate } = require("../shared/render-gate");
const { localizationHtml } = require("./template");

function registerLocalizationExplorer({ getClient }) {
  let panel;
  let timer;
  let search = "";
  let offset = 0;
  let sourceUri;
  const gate = createRenderGate();
  async function refresh() {
    if (!panel?.visible) return;
    const client = getClient();
    if (!client) { panel.webview.postMessage({ type: "error", message: "Start the Tua language server to inspect catalogs." }); return; }
    const request = gate.begin();
    try {
      const params = { search, offset };
      if (sourceUri) params.uri = sourceUri;
      const page = await client.sendRequest("tua/localizationCatalog", params);
      if (!gate.isCurrent(request) || !panel?.visible) return;
      panel.webview.postMessage({ type: "page", page });
    } catch (error) {
      if (gate.isCurrent(request) && panel?.visible) panel.webview.postMessage({ type: "error", message: String(error.message || error) });
    }
  }
  function scheduleRefresh() {
    if (!panel?.visible) return;
    clearTimeout(timer);
    timer = setTimeout(refresh, 400);
  }
  const show = vscode.commands.registerCommand(COMMANDS.showLocalizationExplorer, async () => {
    const active = vscode.window.activeTextEditor?.document;
    sourceUri = active && /\.(?:tua|lua|tdialogue)$/i.test(active.uri.path)
      ? active.uri.toString()
      : undefined;
    if (!panel) {
      panel = vscode.window.createWebviewPanel("tuaLocalization", "Tua Localization", vscode.ViewColumn.Beside, {
        enableScripts: true,
        retainContextWhenHidden: true,
      });
      panel.webview.html = localizationHtml(panel.webview, crypto.randomBytes(16).toString("base64"));
      panel.webview.onDidReceiveMessage(async message => {
        if (message?.type === "ready" || message?.type === "refresh") return refresh();
        if (message?.type === "search") { search = String(message.value || "").slice(0, 100); offset = 0; return refresh(); }
        if (message?.type === "page") { offset = Math.max(0, Number(message.offset) || 0); return refresh(); }
        if (message?.command === "openLocation") return openLocationMessage(message);
      });
      panel.onDidChangeViewState?.(() => { if (panel?.visible) refresh(); });
      panel.onDidDispose?.(() => { gate.invalidate(); clearTimeout(timer); panel = undefined; });
    } else {
      panel.reveal(vscode.ViewColumn.Beside, false);
    }
    await refresh();
  });
  const changed = vscode.workspace.onDidChangeTextDocument(event => {
    if (/\.(lua|tua|tdialogue|toml)$/i.test(event.document.uri.path)) scheduleRefresh();
  });
  const saved = vscode.workspace.onDidSaveTextDocument(document => {
    if (/\.(lua|tua|tdialogue|toml)$/i.test(document.uri.path)) scheduleRefresh();
  });
  return vscode.Disposable.from(show, changed, saved, { dispose() { gate.invalidate(); clearTimeout(timer); panel?.dispose(); } });
}

module.exports = { registerLocalizationExplorer };
