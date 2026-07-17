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

function registerObjectPoolExplorerFeature({ getClient }) {
  const command = vscode.commands.registerCommand(COMMANDS.showObjectPoolExplorer, () =>
    showObjectPoolExplorer({ getClient, focusUri: activeFileUri() }),
  );

  return vscode.Disposable.from(command, {
    dispose() {
      explorerPanel?.dispose?.();
      explorerPanel = undefined;
      explorerFocusUri = undefined;
    },
  });
}

async function showObjectPoolExplorer({ getClient, focusUri }) {
  const client = getClient();

  if (!client) {
    await vscode.window.showWarningMessage("The Tua language server is not running.");
    return;
  }

  explorerFocusUri = focusUri;

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
  panel.webview.html = objectPoolExplorerLoadingHtml(panel.webview);

  try {
    const response = await client.sendRequest("tua/objectPoolGraph", null);
    const viewModel = objectPoolExplorerViewModel(response, focusUri);

    panel.webview.html = objectPoolExplorerHtml(panel.webview, viewModel);
  } catch (error) {
    panel.webview.html = objectPoolExplorerErrorHtml(panel.webview, error);
  }
}

module.exports = {
  registerObjectPoolExplorerFeature,
};
