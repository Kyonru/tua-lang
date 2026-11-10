"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { isStaleDocumentResponse } = require("../shared/documents");
const { openLocationMessage } = require("../shared/locations");
const { ensureExperimentalPreviewEnabled } = require("../shared/preview-gate");
const { escapeHtml, jsonForHtml, webviewDocument } = require("../shared/webview");

const REFRESH_DELAY_MS = 180;
const LABELS = {
  action: "Action", condition: "Condition", wait: "Wait", invert: "Invert",
  repeatN: "Repeat", sequence: "Sequence", selector: "Selector",
  reactiveSequence: "Reactive sequence", reactiveSelector: "Reactive selector",
  parallelAll: "Parallel all", parallelAny: "Parallel any",
  dynamic: "Computed node",
};

function registerBehaviorTreePreview({ configurationTarget, getClient, isTuaDocument }) {
  let panel;
  let document;
  let sequence = 0;
  let timer;

  const render = async ({ loading = false } = {}) => {
    const client = getClient();
    if (!panel || !document || !client) return;
    const current = ++sequence;
    if (loading) panel.webview.html = messageHtml(panel.webview, "Reading behavior trees…");
    try {
      const params = { textDocument: { uri: document.uri.toString() } };
      let response = await client.sendRequest("tua/inspect", params);
      if (isStaleDocumentResponse(response, document)) {
        response = await client.sendRequest("tua/inspect", params);
      }
      if (current !== sequence || client !== getClient()) return;
      if (isStaleDocumentResponse(response, document)) {
        throw new Error("Inspection is stale. Reload after synchronization.");
      }
      const trees = (Array.isArray(response?.items) ? response.items : [])
        .filter((item) => item?.kind === "behaviorTree" && typeof item.name === "string");
      panel.webview.html = trees.length
        ? behaviorTreePreviewHtml(panel.webview, document.uri.toString(), trees)
        : messageHtml(panel.webview, "No static BehaviorTree.new(...) definitions were found.");
    } catch (error) {
      if (current !== sequence || client !== getClient()) return;
      panel.webview.html = messageHtml(panel.webview, error instanceof Error ? error.message : String(error));
    }
  };

  const show = async () => {
    if (!(await ensureExperimentalPreviewEnabled(configurationTarget, "Tua behavior-tree previews"))) {
      return;
    }
    const active = vscode.window.activeTextEditor?.document;
    if (!active || !isTuaDocument(active)) {
      vscode.window.showWarningMessage("Open a Tua file containing a BehaviorTree.new(...) definition.");
      return;
    }
    if (!getClient()) {
      vscode.window.showWarningMessage("The Tua language server is not running.");
      return;
    }
    document = active;
    if (!panel) {
      panel = vscode.window.createWebviewPanel(
        "tuaBehaviorTreePreview", "Tua Behavior Tree Preview", vscode.ViewColumn.Beside,
        { enableScripts: true, retainContextWhenHidden: true },
      );
      panel.webview.onDidReceiveMessage(async (message) => {
        if (message?.command === "reload") return render();
        return openLocationMessage(message);
      });
      panel.onDidDispose(() => {
        panel = undefined;
        document = undefined;
        sequence += 1;
        clearTimeout(timer);
      });
    } else {
      panel.reveal(vscode.ViewColumn.Beside, false);
    }
    await render({ loading: true });
  };

  const command = vscode.commands.registerCommand(COMMANDS.previewBehaviorTree, show);
  const changes = vscode.workspace.onDidChangeTextDocument((event) => {
    if (!panel || event.document.uri.toString() !== document?.uri.toString()) return;
    clearTimeout(timer);
    timer = setTimeout(() => { void render(); }, REFRESH_DELAY_MS);
  });
  return vscode.Disposable.from(command, changes, {
    dispose() { clearTimeout(timer); sequence += 1; panel?.dispose(); },
  });
}

function messageHtml(webview, message) {
  return webviewDocument(webview, {
    title: "Behavior Tree Preview",
    body: `<main class="message"><p>${escapeHtml(message)}</p><button id="reload">Reload</button></main>`,
    styles: `body{color:var(--vscode-foreground);background:var(--vscode-editor-background);font:var(--vscode-font-size) var(--vscode-font-family)}.message{padding:24px}button{color:var(--vscode-button-foreground);background:var(--vscode-button-background);border:0;padding:8px 12px;cursor:pointer}`,
    script: `document.getElementById("reload").addEventListener("click", () => vscode.postMessage({command:"reload"}));`,
  });
}

function behaviorTreePreviewHtml(webview, uri, trees) {
  const ranges = [];
  const nodeHtml = (node) => {
    const index = ranges.push(node.range) - 1;
    const kind = typeof node.kind === "string" ? node.kind : "dynamic";
    const label = LABELS[kind] || "Computed node";
    const children = Array.isArray(node.children) ? node.children : [];
    return `<li><button class="node ${kind === "dynamic" ? "dynamic" : ""}" data-index="${index}" type="button"><span>${escapeHtml(label)}</span><small>line ${Number(node.range?.start?.line ?? 0) + 1}</small></button>${children.length ? `<ul>${children.map(nodeHtml).join("")}</ul>` : ""}</li>`;
  };
  const body = `<header><div><h1>Behavior trees</h1><p>${escapeHtml(uri)}</p></div><button id="reload" type="button">Reload</button></header><main>${trees.map((tree) => `<section><h2>${escapeHtml(tree.name)}</h2>${tree.truncated ? `<p class="empty">Preview limited to the first 1,000 nodes.</p>` : ""}${tree.root ? `<ul class="tree">${nodeHtml(tree.root)}</ul>` : `<p class="empty">Root not set in this file.</p>`}</section>`).join("")}</main>`;
  const script = `const source = ${jsonForHtml(uri)}; const ranges = ${jsonForHtml(ranges)};
    document.getElementById("reload").addEventListener("click", () => vscode.postMessage({command:"reload"}));
    document.querySelectorAll(".node").forEach((button) => button.addEventListener("click", () => {
      vscode.postMessage({command:"openLocation", uri:source, range:ranges[Number(button.dataset.index)]});
    }));`;
  const styles = `
    *{box-sizing:border-box}body{margin:0;color:var(--vscode-foreground);background:var(--vscode-editor-background);font:var(--vscode-font-size) var(--vscode-font-family)}
    header{display:flex;align-items:center;justify-content:space-between;gap:16px;padding:18px 24px;border-bottom:1px solid var(--vscode-panel-border)}
    h1{font-size:1.25rem;margin:0}h2{font-size:1.05rem;margin:0 0 16px}p{margin:4px 0;color:var(--vscode-descriptionForeground);overflow-wrap:anywhere}
    main{padding:24px;max-width:1000px}section+section{margin-top:32px;padding-top:24px;border-top:1px solid var(--vscode-panel-border)}
    button{font:inherit;cursor:pointer}#reload{padding:6px 10px;color:var(--vscode-button-foreground);background:var(--vscode-button-background);border:0}
    .tree,.tree ul{list-style:none;margin:0;padding:0}.tree ul{margin-left:18px;padding-left:20px;border-left:1px solid var(--vscode-panel-border)}
    .tree li{position:relative;margin:8px 0}.tree ul>li::before{content:"";position:absolute;left:-20px;top:19px;width:20px;border-top:1px solid var(--vscode-panel-border)}
    .node{display:flex;gap:16px;align-items:center;justify-content:space-between;min-width:185px;max-width:100%;padding:8px 12px;text-align:left;color:var(--vscode-foreground);background:var(--vscode-editorWidget-background);border:1px solid var(--vscode-panel-border);border-radius:6px}
    .node:hover{background:var(--vscode-list-hoverBackground)}.node:focus-visible,#reload:focus-visible{outline:2px solid var(--vscode-focusBorder);outline-offset:2px}
    .node small{color:var(--vscode-descriptionForeground);white-space:nowrap}.node.dynamic{border-style:dashed}.empty{font-style:italic}
  `;
  return webviewDocument(webview, { title: "Behavior Tree Preview", body, styles, script });
}

module.exports = { behaviorTreePreviewHtml, registerBehaviorTreePreview };
