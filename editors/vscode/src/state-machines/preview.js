"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { isStaleDocumentResponse } = require("../shared/documents");
const { openLocationMessage } = require("../shared/locations");
const { ensureExperimentalPreviewEnabled } = require("../shared/preview-gate");
const {
  stateMachinePreviewErrorHtml,
  stateMachinePreviewHtml,
  stateMachinePreviewLoadingHtml,
} = require("./template");
const { stateMachinePreviewViewModel } = require("./view-model");

const REFRESH_DELAY_MS = 180;

function registerStateMachinePreviewFeature({
  configurationTarget,
  getClient,
  isTuaDocument,
}) {
  let panel;
  let document;
  let renderSequence = 0;
  let refreshTimer;

  const render = async ({ reveal = false, position } = {}) => {
    const client = getClient();
    if (!panel || !document || !client) {
      return undefined;
    }

    const sequence = ++renderSequence;
    panel.webview.html = stateMachinePreviewLoadingHtml(panel.webview, document.uri.toString());

    try {
      const inspection = await inspectStateMachines({ client, document, position });
      if (sequence !== renderSequence) {
        return undefined;
      }

      const viewModel = stateMachinePreviewViewModel(document, inspection);
      if (viewModel.machines.length === 0) {
        panel.webview.html = stateMachinePreviewErrorHtml(
          panel.webview,
          document.uri.toString(),
          "No static stateMachine({...}) definitions were found.",
        );
        return undefined;
      }

      panel.webview.html = stateMachinePreviewHtml(panel.webview, viewModel);
      if (reveal) {
        panel.reveal?.(vscode.ViewColumn.Beside, false);
      }
      return viewModel.summary;
    } catch (error) {
      if (sequence !== renderSequence) {
        return undefined;
      }

      panel.webview.html = stateMachinePreviewErrorHtml(
        panel.webview,
        document.uri.toString(),
        error instanceof Error ? error.message : String(error),
      );
      return undefined;
    }
  };

  const show = async (target) => {
    if (
      !(await ensureExperimentalPreviewEnabled(
        configurationTarget,
        "Tua state-machine previews",
      ))
    ) {
      return undefined;
    }

    const editor = vscode.window.activeTextEditor;
    const activeDocument =
      editor?.document && isTuaDocument(editor.document) ? editor.document : document;

    if (!activeDocument) {
      vscode.window.showWarningMessage(
        "Open a Tua file with a static stateMachine({...}) definition to preview it.",
      );
      return undefined;
    }

    const client = getClient();
    if (!client) {
      vscode.window.showWarningMessage("The Tua language server is not running.");
      return undefined;
    }

    document = activeDocument;
    if (!panel) {
      panel = createPanel(async (message) => {
        if (message?.command === "reload") {
          return render({ reveal: true });
        }
        return openLocationMessage(message);
      });
      panel.onDidDispose?.(() => {
        panel = undefined;
        document = undefined;
        renderSequence += 1;
        clearTimeout(refreshTimer);
      });
    } else {
      panel.reveal?.(vscode.ViewColumn.Beside, false);
    }

    const position =
      target?.uri === document.uri.toString() && target.position
        ? target.position
        : editor?.selection?.active;
    return render({ position });
  };

  const command = vscode.commands.registerCommand(COMMANDS.previewStateMachine, show);
  const reload = vscode.commands.registerCommand(COMMANDS.reloadStateMachinePreview, () =>
    render({ reveal: true }),
  );
  const changes = vscode.workspace.onDidChangeTextDocument?.((event) => {
    if (!panel || !document || event?.document?.uri?.toString() !== document.uri.toString()) {
      return;
    }

    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => {
      void render();
    }, REFRESH_DELAY_MS);
  });

  return vscode.Disposable.from(command, reload, changes, {
    dispose() {
      clearTimeout(refreshTimer);
      renderSequence += 1;
      panel?.dispose?.();
      panel = undefined;
      document = undefined;
    },
  });
}

function createPanel(onMessage) {
  const panel = vscode.window.createWebviewPanel(
    "tuaStateMachinePreview",
    "Tua State Machine Preview",
    vscode.ViewColumn.Beside,
    {
      enableScripts: true,
      retainContextWhenHidden: true,
    },
  );

  panel.webview.onDidReceiveMessage?.(onMessage);
  return panel;
}

async function inspectStateMachines({ client, document, position }) {
  const params = {
    textDocument: {
      uri: document.uri.toString(),
    },
  };
  if (position) {
    params.position = {
      line: position.line,
      character: position.character,
    };
  }

  let response = await freshInspection(client, document, params);
  let items = stateMachineItems(response);

  if (position && items.length === 0) {
    delete params.position;
    response = await freshInspection(client, document, params);
    items = stateMachineItems(response);
  }

  return {
    analysisRevision: response?.analysisRevision,
    documentVersion: response?.documentVersion,
    items,
  };
}

async function freshInspection(client, document, params) {
  let response = await client.sendRequest("tua/inspect", params);

  if (isStaleDocumentResponse(response, document)) {
    response = await client.sendRequest("tua/inspect", params);
  }
  if (isStaleDocumentResponse(response, document)) {
    throw new Error("State-machine inspection is stale. Reload after synchronization.");
  }

  return response;
}

function stateMachineItems(response) {
  return (Array.isArray(response?.items) ? response.items : []).filter(
    (item) => item?.kind === "stateMachine",
  );
}

module.exports = {
  registerStateMachinePreviewFeature,
};
