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
const BRANCH_KINDS = new Set([
  "sequence", "selector", "reactiveSequence", "reactiveSelector", "parallelAll", "parallelAny",
]);
const DECORATOR_KINDS = new Set(["invert", "repeatN"]);

function registerBehaviorTreePreview({ configurationTarget, getClient, isTuaDocument }) {
  let panel;
  let document;
  let selectedName;
  let sequence = 0;
  let timer;

  const render = async ({ loading = false, position } = {}) => {
    const client = getClient();
    if (!panel || !document || !client) return;
    const current = ++sequence;
    if (loading) panel.webview.html = messageHtml(panel.webview, "Reading behavior trees…");
    try {
      const params = { textDocument: { uri: document.uri.toString() } };
      if (position) params.position = position;
      let response = await client.sendRequest("tua/inspect", params);
      if (isStaleDocumentResponse(response, document)) {
        response = await client.sendRequest("tua/inspect", params);
      }
      if (current !== sequence || client !== getClient()) return;
      if (isStaleDocumentResponse(response, document)) {
        throw new Error("Inspection is stale. Reload after synchronization.");
      }
      const inspected = (Array.isArray(response?.items) ? response.items : [])
        .filter((item) => item?.kind === "behaviorTree" && typeof item.name === "string");
      if (position && inspected.length === 1) selectedName = inspected[0].name;
      const trees = selectedName
        ? inspected.filter((item) => item.name === selectedName)
        : inspected;
      panel.webview.html = trees.length
        ? behaviorTreePreviewHtml(panel.webview, document.uri.toString(), trees)
        : messageHtml(panel.webview, "No static BehaviorTree.new(...) definitions were found.");
    } catch (error) {
      if (current !== sequence || client !== getClient()) return;
      panel.webview.html = messageHtml(panel.webview, error instanceof Error ? error.message : String(error));
    }
  };

  const show = async (target) => {
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
    selectedName = undefined;
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
        selectedName = undefined;
        sequence += 1;
        clearTimeout(timer);
      });
    } else {
      panel.reveal(vscode.ViewColumn.Beside, false);
    }
    const position = target?.uri === document.uri.toString() ? target.position : undefined;
    await render({ loading: true, position });
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
    const role = BRANCH_KINDS.has(kind) ? "branch" : DECORATOR_KINDS.has(kind) ? "decorator" : kind === "dynamic" ? "computed" : "leaf";
    const line = Number(node.range?.start?.line ?? 0) + 1;
    return `<li><button class="node node--${role}" data-index="${index}" type="button" title="Open source at line ${line}" aria-label="Open ${escapeHtml(label)} at line ${line}"><span class="node-copy"><strong>${escapeHtml(label)}</strong><small>${role} · line ${line}</small></span></button>${children.length ? `<ul>${children.map(nodeHtml).join("")}</ul>` : ""}</li>`;
  };
  const body = `<header><div><h1>Behavior trees</h1><p>${escapeHtml(uri)}</p></div><button id="reload" type="button">Reload</button></header><main>${trees.map((tree) => `<section><div class="tree-heading"><h2>${escapeHtml(tree.name)}</h2><span>Static structure</span></div>${tree.truncated ? `<p class="note">Preview limited to the first 1,000 nodes.</p>` : ""}${tree.root ? `<div class="tree-scroll" role="region" aria-label="${escapeHtml(tree.name)} behavior tree" tabindex="0"><ul class="tree">${nodeHtml(tree.root)}</ul></div>` : `<p class="note">Root not set in this file.</p>`}</section>`).join("")}</main>`;
  const script = `const source = ${jsonForHtml(uri)}; const ranges = ${jsonForHtml(ranges)};
    document.getElementById("reload").addEventListener("click", () => vscode.postMessage({command:"reload"}));
    document.querySelectorAll(".node").forEach((button) => button.addEventListener("click", () => {
      vscode.postMessage({command:"openLocation", uri:source, range:ranges[Number(button.dataset.index)]});
    }));
    document.querySelectorAll(".tree-scroll").forEach((region) => {
      const root = region.querySelector(".tree > li > .node");
      if (!root) return;
      const regionBox = region.getBoundingClientRect();
      const rootBox = root.getBoundingClientRect();
      region.scrollLeft += (rootBox.left + rootBox.right - regionBox.left - regionBox.right) / 2;
    });`;
  const styles = `
    *{box-sizing:border-box}body{margin:0;color:var(--vscode-foreground);background:var(--vscode-editor-background);font:var(--vscode-font-size) var(--vscode-font-family)}
    header{display:flex;align-items:center;justify-content:space-between;gap:16px;padding:18px 24px;border-bottom:1px solid var(--vscode-panel-border)}
    h1{font-size:1.25rem;margin:0}h2{font-size:1.05rem;margin:0}p{margin:4px 0;color:var(--vscode-descriptionForeground);overflow-wrap:anywhere}
    main{padding:20px 24px}section+section{margin-top:28px}.tree-heading{display:flex;align-items:baseline;gap:12px;padding-bottom:10px;border-bottom:1px solid var(--vscode-panel-border)}
    .tree-heading span,.node small{color:var(--vscode-descriptionForeground);font-size:0.85em}.note{padding:12px 0}
    button{font:inherit;cursor:pointer}#reload{padding:6px 10px;color:var(--vscode-button-foreground);background:var(--vscode-button-background);border:0}
    .tree-scroll{overflow:auto;padding:24px 16px 32px;min-height:144px}.tree-scroll:focus-visible{outline:2px solid var(--vscode-focusBorder);outline-offset:-2px}
    .tree,.tree ul{display:flex;justify-content:center;list-style:none;margin:0;padding:0;min-width:max-content}.tree ul{position:relative;padding-top:32px}
    .tree ul::before{content:"";position:absolute;left:50%;top:0;height:32px;border-left:1px solid var(--vscode-panel-border)}
    .tree li{display:flex;flex-direction:column;align-items:center;position:relative;padding:32px 12px 0}
    .tree>li{padding-top:0}.tree li::before,.tree li::after{content:"";position:absolute;top:0;width:50%;height:32px;border-top:1px solid var(--vscode-panel-border)}
    .tree li::before{right:50%}.tree li::after{left:50%;border-left:1px solid var(--vscode-panel-border)}
    .tree li:first-child::before{display:none}.tree li:last-child::after{border-top:0}
    .tree li:only-child::before{display:none}.tree li:only-child::after{border-top:0}.tree>li::before,.tree>li::after{display:none}
    .tree>li:only-child{padding-top:0}
    .node{display:flex;align-items:center;min-width:148px;max-width:190px;padding:10px 12px;text-align:left;color:var(--vscode-foreground);background:var(--vscode-editorWidget-background);border:1px solid var(--vscode-panel-border);border-radius:5px}
    .node:hover{background:var(--vscode-list-hoverBackground);border-color:var(--vscode-focusBorder)}.node:focus-visible,#reload:focus-visible{outline:2px solid var(--vscode-focusBorder);outline-offset:2px}
    .node-copy{display:flex;flex-direction:column;gap:2px;min-width:0}.node strong{font-weight:600;white-space:nowrap}.node small{white-space:nowrap;text-transform:capitalize}
    .node--computed{border-style:dashed}
  `;
  return webviewDocument(webview, { title: "Behavior Tree Preview", body, styles, script });
}

module.exports = { behaviorTreePreviewHtml, registerBehaviorTreePreview };
