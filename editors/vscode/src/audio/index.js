"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (character) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[character]);
}

function pills(values, empty) {
  if (!values?.length) return `<span class="empty">${escapeHtml(empty)}</span>`;
  return values.map((value) => `<span class="pill">${escapeHtml(value)}</span>`).join("");
}

function html(graph) {
  const systems = graph?.systems || [];
  const cards = systems.map((system) => `
    <article>
      <header><strong>Audio system</strong><button data-uri="${escapeHtml(system.uri)}" data-line="${system.range?.start?.line || 0}">Open declaration</button></header>
      <section><h2>Buses</h2>${pills(system.buses, "Implicit master bus")}</section>
      <section><h2>Cues</h2>${pills(system.cues, "No cues")}</section>
      <section><h2>Music scenes</h2>${pills(system.musicStates, "No scenes")}</section>
      <section><h2>Layers</h2>${pills(system.layers, "No layers")}</section>
      <section><h2>Effects</h2>${pills(system.effects, "No effects")}</section>
    </article>`).join("");
  return `<!doctype html><html><head><meta charset="utf-8"><style>
    body{font-family:var(--vscode-font-family);padding:20px;color:var(--vscode-foreground);background:var(--vscode-editor-background)}
    nav{display:flex;justify-content:space-between;align-items:center;margin-bottom:18px}article{border:1px solid var(--vscode-panel-border);border-radius:8px;padding:16px;margin-bottom:14px}
    header{display:flex;justify-content:space-between;align-items:center}section{margin-top:14px}h1{font-size:20px;margin:0}h2{font-size:11px;text-transform:uppercase;letter-spacing:.08em;color:var(--vscode-descriptionForeground)}
    .pill{display:inline-block;padding:4px 8px;margin:0 6px 6px 0;border-radius:999px;background:var(--vscode-badge-background);color:var(--vscode-badge-foreground)}.empty{color:var(--vscode-descriptionForeground)}button{color:var(--vscode-button-foreground);background:var(--vscode-button-background);border:0;padding:6px 10px;border-radius:3px}</style></head>
    <body><nav><h1>Audio Explorer</h1><button id="reload">Reload</button></nav>${cards || '<p class="empty">No static audioSystem declarations found.</p>'}
    <script>const vscode=acquireVsCodeApi();document.getElementById('reload').onclick=()=>vscode.postMessage({command:'reload'});document.querySelectorAll('[data-uri]').forEach(button=>button.onclick=()=>vscode.postMessage({command:'open',uri:button.dataset.uri,line:Number(button.dataset.line)}));</script></body></html>`;
}

function registerAudioDevex({ getClient }) {
  let panel;
  const render = async () => {
    const client = getClient();
    if (!client || !panel) return;
    panel.webview.html = "<p>Loading audio model…</p>";
    try {
      const response = await client.sendRequest("tua/audioGraph", null);
      panel.webview.html = html(response?.graph);
    } catch (error) {
      panel.webview.html = `<p>Audio Explorer failed: ${escapeHtml(error?.message || error)}</p>`;
    }
  };
  const command = vscode.commands.registerCommand(COMMANDS.showAudioExplorer, async () => {
    if (!getClient()) return vscode.window.showWarningMessage("The Tua language server is not running.");
    if (!panel) {
      panel = vscode.window.createWebviewPanel("tuaAudioExplorer", "Tua Audio Explorer", vscode.ViewColumn.Beside, { enableScripts: true, retainContextWhenHidden: true });
      panel.onDidDispose(() => { panel = undefined; });
      panel.webview.onDidReceiveMessage(async (message) => {
        if (message.command === "reload") return render();
        if (message.command === "open") {
          const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(message.uri));
          const editor = await vscode.window.showTextDocument(document);
          const position = new vscode.Position(message.line || 0, 0);
          editor.selection = new vscode.Selection(position, position);
        }
      });
    } else panel.reveal(vscode.ViewColumn.Beside, false);
    await render();
  });
  return vscode.Disposable.from(command, { dispose() { panel?.dispose(); } });
}

module.exports = { registerAudioDevex };
