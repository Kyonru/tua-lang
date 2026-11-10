"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { openLocationMessage } = require("../shared/locations");
const { createRenderGate } = require("../shared/render-gate");
const { escapeHtml, webviewDocument } = require("../shared/webview");

const styles = `
  body{margin:0;padding:20px;color:var(--vscode-foreground);background:var(--vscode-editor-background);font:var(--vscode-font-size) var(--vscode-font-family)}
  nav,article header{display:flex;align-items:center;justify-content:space-between;gap:12px;flex-wrap:wrap}
  nav{margin-bottom:18px}main{max-width:1000px;margin:auto}article{border:1px solid var(--vscode-panel-border);border-radius:8px;padding:16px;margin-bottom:14px}
  section{margin-top:14px}h1{font-size:20px;margin:0}h2{font-size:12px;color:var(--vscode-descriptionForeground)}
  article strong{overflow-wrap:anywhere}.pill{display:inline-block;padding:4px 8px;margin:0 6px 6px 0;border-radius:999px;background:var(--vscode-badge-background);color:var(--vscode-badge-foreground)}
  .empty{color:var(--vscode-descriptionForeground)}button{color:var(--vscode-button-foreground);background:var(--vscode-button-background);border:1px solid transparent;padding:6px 10px;border-radius:3px;cursor:pointer}
  button:hover{background:var(--vscode-button-hoverBackground)}button:focus-visible{outline:2px solid var(--vscode-focusBorder);outline-offset:2px}
  @media(max-width:600px){body{padding:12px}article{padding:12px}}
`;

function documentHtml(webview, body, script = "") {
  return webviewDocument(webview, { title: "Audio Explorer", body: `<main>${body}</main>`, styles, script });
}

function loadingHtml(webview) {
  return documentHtml(webview, '<p role="status">Loading audio model…</p>');
}

function errorHtml(webview, error) {
  return documentHtml(webview,
    `<h1>Audio Explorer unavailable</h1><p role="alert">${escapeHtml(error instanceof Error ? error.message : String(error))}</p><button id="reload" type="button">Retry</button>`,
    'document.getElementById("reload").addEventListener("click", () => vscode.postMessage({command:"reload"}));');
}

function pills(values, empty) {
  if (!values?.length) return `<span class="empty">${escapeHtml(empty)}</span>`;
  return values.map((value) => `<span class="pill">${escapeHtml(value)}</span>`).join("");
}

function analysisCards(analyses) {
  if (!analyses.length) return '<p class="empty">Run tua check or tua build to generate analysis metadata.</p>';
  return analyses.map((analysis) => {
    const bpm = analysis.bpm == null ? "unresolved" : Number(analysis.bpm).toFixed(2);
    const confidence = Number(analysis.confidence || 0).toFixed(2);
    const ambiguity = Number(analysis.tempoAmbiguity || 0).toFixed(2);
    const candidates = (analysis.tempoCandidates || []).map((candidate) =>
      `${Number(candidate.bpm).toFixed(1)} (${escapeHtml(candidate.relation)}, ${Number(candidate.relativeScore).toFixed(2)})`
    );
    return `<article class="analysis">
      <header><strong>${escapeHtml(analysis.path)}</strong><button data-uri="${escapeHtml(analysis.assetUri)}" data-line="0">Open asset</button></header>
      <p>BPM ${bpm} · confidence ${confidence} · ambiguity ${ambiguity} · boundary influence ${Number(analysis.boundaryInfluence || 0).toFixed(2)}</p>
      <section><h2>Tempo candidates</h2>${pills(candidates, "No reliable candidates")}</section>
    </article>`;
  }).join("");
}

function html(webview, graph, analyses = []) {
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
  return documentHtml(webview,
    `<nav><h1>Audio Explorer</h1><button id="reload" type="button">Reload</button></nav>${cards || '<p class="empty">No static audioSystem declarations found.</p>'}<h2>Generated analysis</h2>${analysisCards(analyses)}`,
    `document.getElementById("reload").addEventListener("click", () => vscode.postMessage({command:"reload"}));
     document.querySelectorAll("[data-uri]").forEach(button => button.addEventListener("click", () => {
       const line = Number(button.dataset.line);
       vscode.postMessage({command:"openLocation", uri:button.dataset.uri, range:{start:{line,character:0},end:{line,character:0}}});
     }));`);
}

function registerAudioDevex({ getClient }) {
  let panel;
  const renderGate = createRenderGate();
  const loadAnalysis = async () => {
    if (!vscode.workspace?.findFiles) return [];
    const files = await vscode.workspace.findFiles("**/audio-index.json", "**/{.git,.tua,node_modules,build}/**");
    const analyses = [];
    for (const uri of files) {
      try {
        const data = JSON.parse(Buffer.from(await vscode.workspace.fs.readFile(uri)).toString("utf8"));
        const folder = vscode.workspace.getWorkspaceFolder(uri);
        for (const item of Array.isArray(data) ? data : []) {
          analyses.push({
            ...item,
            assetUri: folder ? vscode.Uri.joinPath(folder.uri, item.path).toString() : uri.toString(),
          });
        }
      } catch (_) {
        // A partially written index will be picked up by the next refresh.
      }
    }
    return analyses;
  };
  const render = async () => {
    const client = getClient();
    if (!client || !panel) return;
    const currentPanel = panel;
    const request = renderGate.begin();
    currentPanel.webview.html = loadingHtml(currentPanel.webview);
    try {
      const response = await client.sendRequest("tua/audioGraph", null);
      const analyses = await loadAnalysis();
      if (!renderGate.isCurrent(request) || panel !== currentPanel) return;
      currentPanel.webview.html = html(currentPanel.webview, response?.graph, analyses);
    } catch (error) {
      if (!renderGate.isCurrent(request) || panel !== currentPanel) return;
      currentPanel.webview.html = errorHtml(currentPanel.webview, error);
    }
  };
  const command = vscode.commands.registerCommand(COMMANDS.showAudioExplorer, async () => {
    if (!getClient()) return vscode.window.showWarningMessage("The Tua language server is not running.");
    if (!panel) {
      panel = vscode.window.createWebviewPanel("tuaAudioExplorer", "Tua Audio Explorer", vscode.ViewColumn.Beside, { enableScripts: true, retainContextWhenHidden: true });
      panel.onDidDispose(() => { renderGate.invalidate(); panel = undefined; });
      panel.webview.onDidReceiveMessage(async (message) => {
        if (message?.command === "reload") return render();
        return openLocationMessage(message);
      });
    } else panel.reveal(vscode.ViewColumn.Beside, false);
    await render();
  });
  return vscode.Disposable.from(command, { dispose() { renderGate.invalidate(); panel?.dispose(); } });
}

module.exports = { registerAudioDevex, audioExplorerHtml: html, audioExplorerErrorHtml: errorHtml };
