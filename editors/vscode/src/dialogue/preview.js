"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { isStaleDocumentResponse } = require("../shared/documents");
const { displayUriPath, openLocationMessage } = require("../shared/locations");
const { ensureExperimentalPreviewEnabled } = require("../shared/preview-gate");
const { createVisiblePreviewRefresh } = require("../shared/visible-preview-refresh");
const { escapeHtml, jsonForHtml, webviewDocument } = require("../shared/webview");

const DEFAULT_MOCK = '{\n  "context": {},\n  "locals": {}\n}';

const STYLES = `
  body{color:var(--vscode-foreground);background:var(--vscode-editor-background);font:var(--vscode-font-size) var(--vscode-font-family);margin:0;padding:20px}
  header{display:flex;align-items:start;justify-content:space-between;gap:16px}
  h1{font-size:1.4em;margin:0}h2{font-size:1.15em;margin:0 0 12px}
  p{line-height:1.5;max-width:75ch}header p{margin:6px 0}
  button{font:inherit;cursor:pointer;color:var(--vscode-button-foreground);background:var(--vscode-button-background);border:1px solid var(--vscode-contrastBorder,transparent);padding:6px 10px}
  button:hover{background:var(--vscode-button-hoverBackground)}
  textarea:focus-visible,button:focus-visible{outline:2px solid var(--vscode-focusBorder);outline-offset:2px}
  .source{max-width:100%;overflow-wrap:anywhere;background:none;color:var(--vscode-textLink-foreground);text-align:left;padding:2px 0;border:0;text-decoration:underline;text-underline-offset:3px}
  .source:hover{background:none;color:var(--vscode-textLink-activeForeground)}
  section{border-top:1px solid var(--vscode-panel-border);padding:20px 0}
  .speech{white-space:pre-wrap;overflow-wrap:anywhere;margin:8px 0}
  .meta{color:var(--vscode-descriptionForeground)}
  ol{padding-left:24px}li{margin:10px 0;overflow-wrap:anywhere}
  small{display:block;margin-top:4px}code{font-family:var(--vscode-editor-font-family)}
  textarea{display:block;box-sizing:border-box;width:100%;max-width:75ch;min-height:110px;margin:8px 0;color:var(--vscode-input-foreground);background:var(--vscode-input-background);border:1px solid var(--vscode-input-border,var(--vscode-contrastBorder,transparent));padding:8px;font:var(--vscode-editor-font-size) var(--vscode-editor-font-family);resize:vertical}
  .mock-controls{margin:20px 0}.mock-actions{display:flex;gap:8px;flex-wrap:wrap}
  .error{color:var(--vscode-errorForeground)}
  @media(max-width:400px){body{padding:12px}header{flex-wrap:wrap}}
`;

function dialoguePreviewHtml(webview, uri, preview, message, mock = {}) {
  const ranges = [];
  const link = (label, range) => {
    const index = ranges.push(range) - 1;
    return `<button type="button" class="source" data-source="${index}">${escapeHtml(label)}</button>`;
  };
  const conditionResults = new Map();
  for (const condition of preview?.mock?.conditions || []) {
    const key = `${condition.section}:${condition.line ?? ""}:${condition.choice ?? ""}`;
    if (!conditionResults.has(key)) conditionResults.set(key, []);
    conditionResults.get(key).push(condition);
  }
  const resultsHtml = (section, line = "", choice = "") => {
    const results = conditionResults.get(`${section}:${line}:${choice}`) || [];
    return results.map(result => `<small class="${result.error ? "error" : "meta"}">Mock ${escapeHtml(result.kind)}: ${result.error ? escapeHtml(result.error) : String(result.value)} · ${link("Open condition", result.range)}</small>`).join("");
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
      return `<li>${escapeHtml(label)} → ${link(route.target, route.range)}${conditions && !conditionResults.has(`${index}:${route.line}:${route.choice}`) ? `<small class="meta">${conditions} (not evaluated)</small>` : ""}${choice ? resultsHtml(index, route.line, route.choice) : ""}${choice?.reason ? `<small>${escapeHtml(choice.reason)}</small>` : ""}</li>`;
    }).join("");
    const limited = section.lines.length > 100 || outgoing.length > 200;
    return `<section><h2>${link(section.label, section.range)}${index === 0 ? " · Default entry" : ""}</h2>${resultsHtml(index)}${section.action ? `<p>Action: <code>${escapeHtml(section.action)}</code></p>` : ""}${section.wait != null ? `<p>Wait: ${escapeHtml(section.wait)} seconds</p>` : ""}${lines}${routeHtml ? `<ol aria-label="Outgoing routes">${routeHtml}</ol>` : ""}${limited ? '<p class="meta">Large section: showing the first 100 lines and 200 routes. Open source for the full section.</p>' : ""}</section>`;
  }).join("");
  return webviewDocument(webview, {
    title: "Dialogue Preview",
    styles: STYLES,
    body: `<header><div><h1>Dialogue preview</h1><p>${escapeHtml(displayUriPath(uri))}</p></div><button type="button" id="reload">Reload</button></header>
      <p class="meta">${preview?.mock ? "Each condition is tested independently against the last applied JSON. All routes remain visible; no conversation is simulated." : "Authored structure. Conditions are not evaluated until you apply mock JSON."} Placeholders and rich-text marks stay authored. No handlers or audio run.</p>
      <div class="mock-controls"><label for="mock-context">Mock context (JSON)</label><textarea id="mock-context" maxlength="65536" spellcheck="false" aria-describedby="mock-help">${escapeHtml(mock.draft ?? DEFAULT_MOCK)}</textarea>
      <p id="mock-help" class="meta">Use context and locals objects. Missing fields and null mean nil. Changes apply only when you choose Evaluate conditions. Input stays in memory and clears when this preview closes.</p>
      <div class="mock-actions"><button id="apply-mock" type="button">Evaluate conditions</button><button id="clear-mock" type="button">Use authored view</button></div>
      ${mock.error ? `<p class="error" role="alert">${escapeHtml(mock.error)}</p>` : ""}</div>
      <main>${message ? `<p role="status">${escapeHtml(message)}</p>` : content || '<p>No dialogue sections found.</p>'}${preview?.mock?.truncated ? '<p>Only the first 4,096 conditions were tested.</p>' : ""}${sections.length > 200 ? '<p>Showing the first 200 sections. Open source for the full conversation.</p>' : ""}</main>`,
    script: `const ranges = ${jsonForHtml(ranges)};
      const uri = ${jsonForHtml(uri)};
      const input = document.getElementById("mock-context");
      input.addEventListener("input", () => vscode.postMessage({command:"mockDraft", uri, text:input.value}));
      document.getElementById("apply-mock").addEventListener("click", () => vscode.postMessage({command:"applyMock", uri, text:input.value}));
      document.getElementById("clear-mock").addEventListener("click", () => vscode.postMessage({command:"clearMock", uri}));
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
  let mockDraft = DEFAULT_MOCK;
  let appliedMock;
  const showMessage = (message) => {
    renderedVersion = undefined;
    if (panel && document) panel.webview.html = dialoguePreviewHtml(panel.webview, document.uri.toString(), null, message, { draft: mockDraft });
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
      if (appliedMock !== undefined) params.mockContext = appliedMock;
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
      const unsupportedMock = params.mockContext !== undefined && !("mock" in response.preview);
      const mockError = response.mockError || (unsupportedMock
        ? "This compiler does not support mock evaluation. Update Tua, then reload."
        : undefined);
      panel.webview.html = dialoguePreviewHtml(panel.webview, document.uri.toString(), response.preview, undefined, { draft: mockDraft, error: mockError });
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
    if (document?.uri.toString() !== active.uri.toString()) {
      mockDraft = DEFAULT_MOCK;
      appliedMock = undefined;
    }
    document = active;
    refresh.cancel();
    if (!panel) {
      panel = vscode.window.createWebviewPanel("tuaDialoguePreview", "Tua Dialogue Preview", vscode.ViewColumn.Beside, { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [] });
      panel.webview.onDidReceiveMessage(message => {
        if (message?.command === "reload") return render();
        if (message?.uri !== document?.uri.toString()) return;
        if (message.command === "mockDraft" || message.command === "applyMock") {
          if (typeof message.text !== "string" || message.text.length > 65536) {
            if (message.command === "applyMock") {
              appliedMock = undefined;
              sequence += 1;
              showMessage("Mock input is too large. Use at most 64 KiB of JSON.");
            }
            return;
          }
          mockDraft = message.text;
          if (message.command === "applyMock") {
            appliedMock = mockDraft;
            return render();
          }
          return;
        }
        if (message.command === "clearMock") {
          appliedMock = undefined;
          return render();
        }
        if (renderedVersion === document?.version && message?.uri === document?.uri.toString()) return openLocationMessage(message);
      });
      panel.onDidDispose(() => {
        sequence += 1;
        panel = undefined;
        document = undefined;
        mockDraft = DEFAULT_MOCK;
        appliedMock = undefined;
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
