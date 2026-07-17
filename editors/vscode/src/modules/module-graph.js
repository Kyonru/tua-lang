"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { activeFileUri, openLocationMessage } = require("../shared/locations");
const { moduleGraphViewModel } = require("./view-model");
const {
  moduleGraphErrorHtml,
  moduleGraphHtml,
  moduleGraphLoadingHtml,
} = require("./template");

let graphPanel;
let graphFocus;

function registerModuleGraphFeature({ getClient }) {
  const showCommand = vscode.commands.registerCommand(COMMANDS.showModuleGraph, () =>
    showModuleGraph({ getClient, focusUri: activeFileUri(), initialTab: "overview" }),
  );
  const whyCommand = vscode.commands.registerCommand(COMMANDS.whyModuleIncluded, () =>
    showModuleGraph({ getClient, focusUri: activeFileUri(), initialTab: "why" }),
  );

  return vscode.Disposable.from(showCommand, whyCommand, {
    dispose() {
      graphPanel?.dispose?.();
      graphPanel = undefined;
      graphFocus = undefined;
    },
  });
}

async function showModuleGraph({ getClient, focusUri, initialTab }) {
  const client = getClient();

  if (!client) {
    await vscode.window.showWarningMessage("The Tua language server is not running.");
    return;
  }

  graphFocus = { uri: focusUri, initialTab };

  if (graphPanel) {
    graphPanel.reveal?.(vscode.ViewColumn.Beside, false);
  } else {
    graphPanel = vscode.window.createWebviewPanel(
      "tuaModuleGraph",
      "Tua Module Graph",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      },
    );
    graphPanel.onDidDispose?.(() => {
      graphPanel = undefined;
      graphFocus = undefined;
    });
    graphPanel.webview.onDidReceiveMessage?.((message) =>
      handleGraphMessage(message, { getClient }),
    );
  }

  await renderModuleGraph(graphPanel, client, graphFocus);
}

async function handleGraphMessage(message, { getClient }) {
  if (message?.command === "reload") {
    const client = getClient();

    if (client && graphPanel) {
      await renderModuleGraph(graphPanel, client, graphFocus);
    }
    return;
  }

  await openLocationMessage(message);
}

async function renderModuleGraph(panel, client, focus) {
  panel.webview.html = moduleGraphLoadingHtml(panel.webview);

  try {
    const response = await client.sendRequest("tua/moduleGraph", null);
    const viewModel = moduleGraphViewModel(response, focus);

    panel.webview.html = moduleGraphHtml(panel.webview, viewModel);
  } catch (error) {
    panel.webview.html = moduleGraphErrorHtml(panel.webview, error);
  }
}

module.exports = {
  registerModuleGraphFeature,
};
