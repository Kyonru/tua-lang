"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { isStaleDocumentResponse } = require("../shared/documents");
const { displayUriPath, openLocationMessage } = require("../shared/locations");
const { ensureExperimentalPreviewEnabled } = require("../shared/preview-gate");
const { createVisiblePreviewRefresh } = require("../shared/visible-preview-refresh");
const { escapeHtml, jsonForHtml, webviewDocument } = require("../shared/webview");

const STYLES = `
  body{color:var(--vscode-foreground);background:var(--vscode-editor-background);font:var(--vscode-font-size) var(--vscode-font-family);margin:0;padding:20px}
  header{display:flex;align-items:start;justify-content:space-between;gap:16px}
  h1{font-size:1.4em;margin:0}h2{font-size:1.15em;margin:0 0 12px}
  p{line-height:1.5;max-width:75ch}header p{margin:6px 0}
  button{font:inherit;cursor:pointer;color:var(--vscode-button-foreground);background:var(--vscode-button-background);border:1px solid var(--vscode-contrastBorder,transparent);padding:6px 10px}
  button:hover{background:var(--vscode-button-hoverBackground)}
  button:focus-visible{outline:2px solid var(--vscode-focusBorder);outline-offset:2px}
  .source{background:none;color:var(--vscode-textLink-foreground);text-align:left;padding:2px 0;border:0;text-decoration:underline;text-underline-offset:3px}
  .source:hover{background:none;color:var(--vscode-textLink-activeForeground)}
  section{border-top:1px solid var(--vscode-panel-border);padding:20px 0}
  .speech{white-space:pre-wrap;overflow-wrap:anywhere;margin:8px 0}
  .meta{color:var(--vscode-descriptionForeground)}
  ol{padding-left:24px}li{margin:10px 0;overflow-wrap:anywhere}
  small{display:block;margin-top:4px}code{font-family:var(--vscode-editor-font-family)}
  @media(max-width:400px){body{padding:12px}header{flex-wrap:wrap}}
`;

function dialoguePreviewHtml(webview, uri, preview, message) {
  const ranges = [];
  const link = (label, range) => {
    const index = ranges.push(range) - 1;
    return `<button type="button" class="source" data-source="${index}">${escapeHtml(label)}</button>`;
  };
  const sections = preview?.sections || [];
  const routes = new Map();
  for (const route of preview?.routes || []) {
    if (!routes.has(route.section)) routes.set(route.section, []);
    routes.get(route.section).push(route);
  }
  const labels = { fallback: "No available choices", wait: "After wait", branchTrue: "Condition true", branchFalse: "Condition false", actionSuccess: "Action succeeds", actionFailure: "Action fails" };
  const content = sections.slice(0, 200).map((section, index) => {
    const lines = section.lines.slice(0, 100).map((line) => `<div class="speech">${line.speaker ? `<strong>${escapeHtml(line.speaker)}</strong><br>` : ""}${escapeHtml(line.text)}<small>${link("Open text", line.range)}${line.end ? " · Ends conversation" : ""}</small></div>`).join("");
    const outgoing = routes.get(index) || [];
    const routeHtml = outgoing.slice(0, 200).map((route) => {
      const choice = section.lines[route.line]?.choices?.[route.choice];
      const label = choice ? choice.text : labels[route.kind] || route.kind;
      const conditions = choice ? [choice.visibleCondition && "Visibility condition", choice.enabledCondition && "Availability condition"].filter(Boolean).join(" · ") : "";
      return `<li>${escapeHtml(label)} → ${link(route.target, route.range)}${conditions ? `<small class="meta">${conditions} (not evaluated)</small>` : ""}${choice?.reason ? `<small>${escapeHtml(choice.reason)}</small>` : ""}</li>`;
    }).join("");
    const limited = section.lines.length > 100 || outgoing.length > 200;
    return `<section><h2>${link(section.label, section.range)}${index === 0 ? " · Default entry" : ""}</h2>${section.action ? `<p>Action: <code>${escapeHtml(section.action)}</code></p>` : ""}${section.wait != null ? `<p>Wait: ${escapeHtml(section.wait)} seconds</p>` : ""}${lines}${routeHtml ? `<ol aria-label="Outgoing routes">${routeHtml}</ol>` : ""}${limited ? '<p class="meta">Large section: showing the first 100 lines and 200 routes. Open source for the full section.</p>' : ""}</section>`;
  }).join("");
  return webviewDocument(webview, {
    title: "Dialogue Preview",
    styles: STYLES,
    body: `<header><div><h1>Dialogue preview</h1><p>${escapeHtml(displayUriPath(uri))}</p></div><button type="button" id="reload">Reload</button></header><p class="meta">Authored structure. Conditions, placeholders and rich-text marks are not evaluated. No handlers or audio run.</p><main>${message ? `<p role="status">${escapeHtml(message)}</p>` : content || '<p>No dialogue sections found.</p>'}${sections.length > 200 ? '<p>Showing the first 200 sections. Open source for the full conversation.</p>' : ""}</main>`,
    script: `const ranges = ${jsonForHtml(ranges)};
      document.getElementById("reload").addEventListener("click", () => vscode.postMessage({command:"reload"}));
      document.querySelectorAll("[data-source]").forEach(button => button.addEventListener("click", () => {
        vscode.postMessage({command:"openLocation", uri:${jsonForHtml(uri)}, range:ranges[Number(button.dataset.source)]});
      }));`,
  });
}

function registerDialoguePreview({ configurationTarget, getClient }) {
  let panel;
  let document;
  let sequence = 0;
  let renderedVersion;
  const showMessage = (message) => {
    renderedVersion = undefined;
    if (panel && document) panel.webview.html = dialoguePreviewHtml(panel.webview, document.uri.toString(), null, message);
  };
  const render = async () => {
    const client = getClient();
    if (!panel || !document) return;
    const current = ++sequence;
    const requestedDocument = document;
    showMessage("Reading dialogue…");
    if (!client) return showMessage("The Tua language server is not running. Start it, then reload.");
    try {
      const params = { textDocument: { uri: document.uri.toString() } };
      let response = await client.sendRequest("tua/dialoguePreview", params);
      if (current !== sequence || client !== getClient()) return;
      if (isStaleDocumentResponse(response, requestedDocument)) {
        response = await client.sendRequest("tua/dialoguePreview", params);
      }
      if (current !== sequence || client !== getClient()) return;
      if (isStaleDocumentResponse(response, requestedDocument)) {
        return showMessage("Source changed during analysis. Reload after synchronization.");
      }
      if (!response?.preview) return showMessage("No valid dialogue preview. Check Problems for syntax errors, then reload.");
      renderedVersion = response.documentVersion;
      panel.webview.html = dialoguePreviewHtml(panel.webview, document.uri.toString(), response.preview);
    } catch (error) {
      if (current !== sequence || client !== getClient()) return;
      showMessage(`Could not read dialogue. Reload to retry. ${error instanceof Error ? error.message : String(error)}`);
    }
  };
  const refresh = createVisiblePreviewRefresh(() => panel, render, 180);
  const command = vscode.commands.registerCommand(COMMANDS.previewDialogue, async () => {
    const active = vscode.window.activeTextEditor?.document;
    if (!active || active.languageId !== "tdialogue") {
      vscode.window.showWarningMessage("Open a .tdialogue file to preview its conversation.");
      return;
    }
    if (!(await ensureExperimentalPreviewEnabled(configurationTarget, "Tua dialogue previews"))) return;
    document = active;
    refresh.cancel();
    if (!panel) {
      panel = vscode.window.createWebviewPanel("tuaDialoguePreview", "Tua Dialogue Preview", vscode.ViewColumn.Beside, { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [] });
      panel.webview.onDidReceiveMessage(message => {
        if (message?.command === "reload") return render();
        if (renderedVersion === document?.version && message?.uri === document?.uri.toString()) return openLocationMessage(message);
      });
      panel.onDidDispose(() => {
        sequence += 1;
        panel = undefined;
        document = undefined;
        renderedVersion = undefined;
        refresh.cancel();
      });
      panel.onDidChangeViewState(() => refresh.onVisible());
    } else {
      panel.reveal(vscode.ViewColumn.Beside, false);
    }
    await render();
  });
  const changes = vscode.workspace.onDidChangeTextDocument(event => {
    if (event.document.uri.toString() !== document?.uri.toString()) return;
    sequence += 1;
    renderedVersion = undefined;
    if (panel?.visible) showMessage("Source changed. Updating preview…");
    refresh.schedule();
  });
  return vscode.Disposable.from(command, changes, { dispose() { sequence += 1; refresh.cancel(); panel?.dispose(); } });
}

module.exports = { dialoguePreviewHtml, registerDialoguePreview };
