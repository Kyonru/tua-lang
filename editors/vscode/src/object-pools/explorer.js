"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { activeFileUri, openLocationMessage } = require("../shared/locations");
const { objectPoolExplorerViewModel } = require("./view-model");
const {
  objectPoolExplorerErrorHtml,
  objectPoolExplorerHtml,
  objectPoolExplorerLoadingHtml,
} = require("./template");

let explorerPanel;
let explorerFocusUri;
let explorerFocusTarget;
let renderSequence = 0;

function registerObjectPoolExplorerFeature({ getClient }) {
  const command = vscode.commands.registerCommand(COMMANDS.showObjectPoolExplorer, (target) =>
    showObjectPoolExplorer({ getClient, focusUri: target?.uri || activeFileUri(), target }),
  );

  return vscode.Disposable.from(command, {
    dispose() {
      explorerPanel?.dispose?.();
      explorerPanel = undefined;
      explorerFocusUri = undefined;
      explorerFocusTarget = undefined;
      renderSequence += 1;
    },
  });
}

async function showObjectPoolExplorer({ getClient, focusUri, target }) {
  const client = getClient();

  if (!client) {
    await vscode.window.showWarningMessage("The Tua language server is not running.");
    return;
  }

  explorerFocusUri = focusUri;
  explorerFocusTarget = target;

  if (explorerPanel) {
    explorerPanel.reveal?.(vscode.ViewColumn.Beside, false);
  } else {
    explorerPanel = vscode.window.createWebviewPanel(
      "tuaObjectPoolExplorer",
      "Tua Object Pool Explorer",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      },
    );
    explorerPanel.onDidDispose?.(() => {
      explorerPanel = undefined;
      explorerFocusUri = undefined;
      explorerFocusTarget = undefined;
      renderSequence += 1;
    });
    explorerPanel.webview.onDidReceiveMessage?.((message) =>
      handleExplorerMessage(message, { getClient }),
    );
  }

  await renderObjectPoolExplorer(explorerPanel, client, explorerFocusUri);
}

async function handleExplorerMessage(message, { getClient }) {
  if (message?.command === "reload") {
    const client = getClient();

    if (client && explorerPanel) {
      await renderObjectPoolExplorer(explorerPanel, client, explorerFocusUri);
    }
    return;
  }

  await openLocationMessage(message);
}

async function renderObjectPoolExplorer(panel, client, focusUri) {
  const sequence = ++renderSequence;
  const target = explorerFocusTarget;
  panel.webview.html = objectPoolExplorerLoadingHtml(panel.webview);

  try {
    const response = await client.sendRequest("tua/objectPoolGraph", null);
    if (sequence !== renderSequence) return;
    const viewModel = objectPoolExplorerViewModel(response, focusUri);
    viewModel.focusId = viewModel.pools.find(pool => pool.uri === focusUri && pool.name === target?.name && pool.line === target?.line)?.id;

    panel.webview.html = objectPoolExplorerHtml(panel.webview, viewModel);
  } catch (error) {
    if (sequence !== renderSequence) return;
    panel.webview.html = objectPoolExplorerErrorHtml(panel.webview, error);
  }
}

module.exports = {
  registerObjectPoolExplorerFeature,
};
