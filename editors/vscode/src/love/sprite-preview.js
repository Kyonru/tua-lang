"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { ensureExperimentalPreviewEnabled } = require("../shared/preview-gate");
const { inspectSpriteSheets } = require("./sprite-preview/inspector");
const { createSpritePreviewState } = require("./sprite-preview/state");
const {
  spritePreviewViewModel,
  spritePreviewWebviewOptions,
} = require("./sprite-preview/view-model");
const {
  spritePreviewErrorHtml,
  spritePreviewHtml,
  spritePreviewLoadingHtml,
} = require("./sprite-preview/template");

function registerSpritePreviewFeature({
  configurationTarget,
  getClient,
  isTuaDocument,
}) {
  const state = createSpritePreviewState();

  return vscode.Disposable.from(
    vscode.commands.registerCommand(COMMANDS.previewSprites, () =>
      previewSpriteAnimations({ configurationTarget, getClient, isTuaDocument, state }),
    ),
    vscode.commands.registerCommand(COMMANDS.reloadSpritePreview, () =>
      reloadSpritePreview({ getClient, state }),
    ),
    {
      dispose() {
        state.dispose();
      },
    },
  );
}

async function previewSpriteAnimations({
  configurationTarget,
  getClient,
  isTuaDocument,
  state,
}) {
  if (
    !(await ensureExperimentalPreviewEnabled(
      configurationTarget,
      "Tua sprite animation previews",
    ))
  ) {
    return undefined;
  }

  const document = activePreviewDocument(state, isTuaDocument);

  if (!document) {
    vscode.window.showWarningMessage(
      "Open a Tua file with spriteSheet definitions to preview animations.",
    );
    return undefined;
  }

  if (document.uri?.scheme !== "file") {
    vscode.window.showWarningMessage(
      "Sprite animation preview currently supports file-backed Tua documents.",
    );
    return undefined;
  }

  const client = getClient();
  if (!client) {
    vscode.window.showWarningMessage("The Tua language server is not running.");
    return undefined;
  }

  const existingPanel = state.panel();
  const panel =
    existingPanel ||
    createPreviewPanel(state, () => reloadSpritePreview({ getClient, state }));

  existingPanel?.reveal?.(vscode.ViewColumn.Beside, false);

  return renderSpritePreviewPanel({ client, document, panel, state });
}

async function reloadSpritePreview({ getClient, state }) {
  const panel = state.panel();
  const document = state.document();
  const client = getClient();

  if (!panel || !document) {
    return undefined;
  }

  if (!client) {
    vscode.window.showWarningMessage("The Tua language server is not running.");
    return undefined;
  }

  const result = await renderSpritePreviewPanel({ client, document, panel, state });

  if (result) {
    panel.reveal?.(vscode.ViewColumn.Beside, false);
  }

  return result;
}

async function renderSpritePreviewPanel({ client, document, panel, state }) {
  const renderId = state.beginRender(document);

  panel.webview.html = spritePreviewLoadingHtml(panel.webview, document.uri.fsPath);

  try {
    const inspection = await inspectSpriteSheets({ client, document });

    if (!state.isCurrentRender(renderId)) {
      return undefined;
    }

    const viewModel = spritePreviewViewModel(panel.webview, document, inspection);

    if (viewModel.sprites.length === 0) {
      vscode.window.showWarningMessage(
        "No spriteSheet definitions were found in the active Tua file.",
      );
      panel.webview.html = spritePreviewErrorHtml(
        panel.webview,
        document.uri.fsPath,
        "No spriteSheet definitions were found.",
      );
      return undefined;
    }

    panel.webview.options = spritePreviewWebviewOptions(viewModel);
    panel.webview.html = spritePreviewHtml(panel.webview, viewModel);

    return viewModel.summary;
  } catch (error) {
    if (!state.isCurrentRender(renderId)) {
      return undefined;
    }

    panel.webview.html = spritePreviewErrorHtml(
      panel.webview,
      document.uri.fsPath,
      error instanceof Error ? error.message : String(error),
    );
    return undefined;
  }
}

function createPreviewPanel(state, onReload) {
  const panel = vscode.window.createWebviewPanel(
    "tuaSpritePreview",
    "Tua Sprite Preview",
    vscode.ViewColumn.Beside,
    spritePreviewWebviewOptions({ localResourceRoots: [] }),
  );

  state.attach(panel);
  panel.onDidDispose?.(() => state.clearPanel(panel));
  panel.webview.onDidReceiveMessage?.(async (message) => {
    if (message?.command === "reload") {
      try {
        return await onReload();
      } finally {
        if (state.panel() === panel) {
          await panel.webview.postMessage?.({ command: "reloadComplete" });
        }
      }
    }

    return undefined;
  });

  return panel;
}

function activePreviewDocument(state, isTuaDocument) {
  const document = vscode.window.activeTextEditor?.document;

  if (document && isTuaDocument(document)) {
    return document;
  }

  return state.document();
}

module.exports = {
  registerSpritePreviewFeature,
};
