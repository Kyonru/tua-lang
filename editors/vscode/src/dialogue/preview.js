"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { isStaleDocumentResponse } = require("../shared/documents");
const { openLocationMessage } = require("../shared/locations");
const { ensureExperimentalPreviewEnabled } = require("../shared/preview-gate");
const { createVisiblePreviewRefresh } = require("../shared/visible-preview-refresh");

const { dialoguePreviewHtml, DEFAULT_MOCK } = require("./presentation");

function registerDialoguePreview({ configurationTarget, getClient }) {
  let panel;
  let document;
  let sequence = 0;
  let renderedVersion;
  let watcher;
  let sourceDeleted = false;
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
    let requestedDocument = document;
    showMessage("Reading dialogue…");
    if (!client) return showMessage("The Tua language server is not running. Start it, then reload.");
    if (sourceDeleted && !document.isDirty) return showMessage("Dialogue file was deleted. Recreate it or open another dialogue.");
    try {
      if (document.isClosed) {
        const reopened = await vscode.workspace.openTextDocument(document.uri);
        if (current !== sequence || client !== getClient()) return;
        document = reopened;
        requestedDocument = reopened;
      }
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
      const unsupportedText = response.preview.mock && !("text" in response.preview.mock);
      const mockError = response.mockError || (unsupportedMock
        ? "This compiler does not support mock evaluation. Update Tua, then reload."
        : unsupportedText ? "This compiler supports condition probes only. Update Tua for text previews." : undefined);
      panel.webview.html = dialoguePreviewHtml(panel.webview, document.uri.toString(), response.preview, undefined, { draft: mockDraft, applied: appliedMock, error: mockError });
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
      sourceDeleted = false;
    }
    document = active;
    refresh.cancel();
    if (!panel) {
      panel = vscode.window.createWebviewPanel("tuaDialoguePreview", "Tua Dialogue Preview", vscode.ViewColumn.Beside, { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [] });
      watcher = vscode.workspace.createFileSystemWatcher("**/*.tdialogue");
      const onDiskChange = (uri, deleted) => {
        if (uri.toString() !== document?.uri.toString()) return;
        sourceDeleted = deleted;
        if (document.isDirty) return;
        sequence += 1;
        renderedVersion = undefined;
        if (panel?.visible) showMessage(deleted
          ? "Dialogue file was deleted. Recreate it or open another dialogue."
          : "Dialogue file changed. Updating preview…");
        refresh.schedule();
      };
      watcher.onDidChange(uri => onDiskChange(uri, false));
      watcher.onDidCreate(uri => onDiskChange(uri, false));
      watcher.onDidDelete(uri => onDiskChange(uri, true));
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
        watcher?.dispose();
        watcher = undefined;
        sourceDeleted = false;
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
  const closes = vscode.workspace.onDidCloseTextDocument(closed => {
    if (closed.uri.toString() !== document?.uri.toString()) return;
    sequence += 1;
    renderedVersion = undefined;
    if (panel?.visible) showMessage("Dialogue buffer closed. Updating preview…");
    refresh.schedule();
  });
  return vscode.Disposable.from(command, changes, closes, { dispose() { sequence += 1; refresh.cancel(); panel?.dispose(); } });
}

module.exports = { dialoguePreviewHtml, registerDialoguePreview };
