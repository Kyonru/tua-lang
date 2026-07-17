"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { serverHealthViewModel } = require("./view-model");
const {
  serverHealthErrorHtml,
  serverHealthHtml,
  serverHealthLoadingHtml,
} = require("./template");

let healthPanel;
let lastHealth;

function registerServerHealthFeature({ getClient }) {
  const command = vscode.commands.registerCommand(COMMANDS.showServerHealth, () =>
    showServerHealth({ getClient }),
  );

  return vscode.Disposable.from(command, {
    dispose() {
      healthPanel?.dispose?.();
      healthPanel = undefined;
      lastHealth = undefined;
    },
  });
}

async function showServerHealth({ getClient }) {
  const client = getClient();

  if (!client) {
    await vscode.window.showWarningMessage("The Tua language server is not running.");
    return;
  }

  if (healthPanel) {
    healthPanel.reveal?.(vscode.ViewColumn.Beside, false);
  } else {
    healthPanel = vscode.window.createWebviewPanel(
      "tuaServerHealth",
      "Tua Server Health",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      },
    );
    healthPanel.onDidDispose?.(() => {
      healthPanel = undefined;
      lastHealth = undefined;
    });
    healthPanel.webview.onDidReceiveMessage?.((message) =>
      handleHealthMessage(message, { getClient }),
    );
  }

  await renderServerHealth(healthPanel, client);
}

async function handleHealthMessage(message, { getClient }) {
  if (message?.command === "reload") {
    const client = getClient();

    if (client && healthPanel) {
      await renderServerHealth(healthPanel, client);
    }
    return;
  }

  if (message?.command === "copyJson" && lastHealth) {
    await vscode.env.clipboard.writeText(`${JSON.stringify(lastHealth.raw, null, 2)}\n`);
    await vscode.window.showInformationMessage("Tua language-server health copied to clipboard.");
  }
}

async function renderServerHealth(panel, client) {
  panel.webview.html = serverHealthLoadingHtml(panel.webview);

  try {
    const response = await client.sendRequest("tua/health", null);
    const viewModel = serverHealthViewModel(response);

    lastHealth = viewModel;
    panel.webview.html = serverHealthHtml(panel.webview, viewModel);
  } catch (error) {
    panel.webview.html = serverHealthErrorHtml(panel.webview, error);
  }
}

module.exports = {
  registerServerHealthFeature,
};
