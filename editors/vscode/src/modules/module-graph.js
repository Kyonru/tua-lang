"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
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

  if (message?.command !== "openLocation" || typeof message.uri !== "string") {
    return;
  }

  const range = message.range || {
    start: { line: 0, character: 0 },
    end: { line: 0, character: 0 },
  };
  const start = new vscode.Position(
    Number(range.start?.line) || 0,
    Number(range.start?.character) || 0,
  );
  const end = new vscode.Position(
    Number(range.end?.line) || start.line,
    Number(range.end?.character) || start.character,
  );

  await vscode.commands.executeCommand("vscode.open", vscode.Uri.parse(message.uri), {
    preview: true,
    selection: new vscode.Range(start, end),
  });
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

function activeFileUri() {
  const uri = vscode.window.activeTextEditor?.document?.uri;
  return uri?.scheme === "file" ? uri.toString() : undefined;
}

module.exports = {
  registerModuleGraphFeature,
};
