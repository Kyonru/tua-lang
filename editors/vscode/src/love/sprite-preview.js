"use strict";

const vscode = require("vscode");
const { COMMANDS, STATUS_SELECTOR } = require("../constants");
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
    registerSpritePreviewHover({ getClient, isTuaDocument }),
    {
      dispose() {
        state.dispose();
      },
    },
  );
}

function registerSpritePreviewHover({ getClient, isTuaDocument }) {
  return vscode.languages.registerHoverProvider(STATUS_SELECTOR, {
    async provideHover(document, position) {
      if (!isTuaDocument(document)) {
        return undefined;
      }

      try {
        const inspection = await inspectSpriteSheets({
          client: getClient(),
          document,
          position,
        });
        const sprite = inspection.items[0];

        if (!sprite) {
          return undefined;
        }

        const label = `Preview ${sprite.name}`;
        const markdown = new vscode.MarkdownString(
          `[${label}](command:${COMMANDS.previewSprites})`,
          true,
        );

        markdown.isTrusted = { enabledCommands: [COMMANDS.previewSprites] };

        return new vscode.Hover(markdown, lspRangeToVscode(sprite.selectionRange));
      } catch {
        return undefined;
      }
    },
  });
}

async function previewSpriteAnimations({
  configurationTarget,
  getClient,
  isTuaDocument,
  state,
}) {
  if (!(await ensurePreviewEnabled(configurationTarget))) {
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
  panel.webview.onDidReceiveMessage?.((message) => {
    if (message?.command === "reload") {
      return onReload();
    }

    return undefined;
  });

  return panel;
}

async function ensurePreviewEnabled(configurationTarget) {
  const config = vscode.workspace.getConfiguration("tua");

  if (config.get("preview.enabled", false)) {
    return true;
  }

  const choice = await vscode.window.showInformationMessage(
    "Tua sprite animation previews are experimental. Enable preview features for this workspace?",
    "Enable",
    "Open Settings",
  );

  if (choice === "Enable") {
    await config.update("preview.enabled", true, configurationTarget());
    return true;
  }

  if (choice === "Open Settings") {
    await vscode.commands.executeCommand(
      "workbench.action.openSettings",
      "tua.preview.enabled",
    );
  }

  return false;
}

function activePreviewDocument(state, isTuaDocument) {
  const document = vscode.window.activeTextEditor?.document;

  if (document && isTuaDocument(document)) {
    return document;
  }

  return state.document();
}

function lspRangeToVscode(range) {
  if (!range?.start || !range?.end) {
    return undefined;
  }

  return new vscode.Range(
    new vscode.Position(range.start.line, range.start.character),
    new vscode.Position(range.end.line, range.end.character),
  );
}

module.exports = {
  registerSpritePreviewFeature,
};
