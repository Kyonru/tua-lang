"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { activeFileUri, openLocationMessage } = require("../shared/locations");
const { createRenderGate } = require("../shared/render-gate");
const { ecsExplorerViewModel } = require("./view-model");
const {
  ecsExplorerErrorHtml,
  ecsExplorerHtml,
  ecsExplorerLoadingHtml,
} = require("./template");

let explorerPanel;
let explorerFocusUri;
let explorerComponentName;
const renderGate = createRenderGate();

function registerEcsExplorerFeature({ getClient }) {
  const command = vscode.commands.registerCommand(COMMANDS.showEcsExplorer, target =>
    showEcsExplorer({ getClient, focusUri: target?.focusUri || activeFileUri(), componentName: target?.componentName }),
  );

  return vscode.Disposable.from(command, {
    dispose() {
      renderGate.invalidate();
      explorerPanel?.dispose?.();
      explorerPanel = undefined;
      explorerFocusUri = undefined;
      explorerComponentName = undefined;
    },
  });
}

async function showEcsExplorer({ getClient, focusUri, componentName }) {
  const client = getClient();

  if (!client) {
    await vscode.window.showWarningMessage("The Tua language server is not running.");
    return;
  }

  explorerFocusUri = focusUri;
  explorerComponentName = typeof componentName === "string" ? componentName : undefined;

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
      renderGate.invalidate();
      explorerPanel = undefined;
      explorerFocusUri = undefined;
      explorerComponentName = undefined;
    });
    explorerPanel.webview.onDidReceiveMessage?.((message) =>
      handleExplorerMessage(message, { getClient }),
    );
  }

  await renderEcsExplorer(explorerPanel, client, explorerFocusUri, explorerComponentName);
}

async function handleExplorerMessage(message, { getClient }) {
  if (message?.command === "reload") {
    const client = getClient();

    if (client && explorerPanel) {
      await renderEcsExplorer(explorerPanel, client, explorerFocusUri, explorerComponentName);
    }
    return;
  }

  await openLocationMessage(message);
}

async function renderEcsExplorer(panel, client, focusUri, componentName) {
  const request = renderGate.begin();
  panel.webview.html = ecsExplorerLoadingHtml(panel.webview);

  try {
    const response = await client.sendRequest("tua/ecsGraph", null);
    if (!renderGate.isCurrent(request) || explorerPanel !== panel) return;
    const viewModel = ecsExplorerViewModel(response, focusUri, componentName);

    panel.webview.html = ecsExplorerHtml(panel.webview, viewModel);
  } catch (error) {
    if (!renderGate.isCurrent(request) || explorerPanel !== panel) return;
    panel.webview.html = ecsExplorerErrorHtml(panel.webview, error);
  }
}

module.exports = {
  registerEcsExplorerFeature,
};
