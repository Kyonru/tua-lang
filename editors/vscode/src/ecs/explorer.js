"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { activeFileUri, openLocationMessage } = require("../shared/locations");
const { ecsExplorerViewModel } = require("./view-model");
const {
  ecsExplorerErrorHtml,
  ecsExplorerHtml,
  ecsExplorerLoadingHtml,
} = require("./template");

let explorerPanel;
let explorerFocusUri;

function registerEcsExplorerFeature({ getClient }) {
  const command = vscode.commands.registerCommand(COMMANDS.showEcsExplorer, () =>
    showEcsExplorer({ getClient, focusUri: activeFileUri() }),
  );

  return vscode.Disposable.from(command, {
    dispose() {
      explorerPanel?.dispose?.();
      explorerPanel = undefined;
      explorerFocusUri = undefined;
    },
  });
}

async function showEcsExplorer({ getClient, focusUri }) {
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
      "tuaEcsExplorer",
      "Tua ECS Explorer",
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

  await renderEcsExplorer(explorerPanel, client, explorerFocusUri);
}

async function handleExplorerMessage(message, { getClient }) {
  if (message?.command === "reload") {
    const client = getClient();

    if (client && explorerPanel) {
      await renderEcsExplorer(explorerPanel, client, explorerFocusUri);
    }
    return;
  }

  await openLocationMessage(message);
}

async function renderEcsExplorer(panel, client, focusUri) {
  panel.webview.html = ecsExplorerLoadingHtml(panel.webview);

  try {
    const response = await client.sendRequest("tua/ecsGraph", null);
    const viewModel = ecsExplorerViewModel(response, focusUri);

    panel.webview.html = ecsExplorerHtml(panel.webview, viewModel);
  } catch (error) {
    panel.webview.html = ecsExplorerErrorHtml(panel.webview, error);
  }
}

module.exports = {
  registerEcsExplorerFeature,
};
