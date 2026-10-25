"use strict";

const vscode = require("vscode");
const { contextualHints } = require("./contextual-hints");
const { COMMANDS } = require("../constants");
const { formatError } = require("../shared/errors");

const MAXIMUM_LENGTH_SETTING = "inlayHints.maximumLength";
const COMPACT_MAXIMUM_LENGTH = 1;
const MODE_STATE = "tua.inlayHints.mode";
const PREVIOUS_MAXIMUM_LENGTH_STATE = "tua.inlayHints.previousMaximumLength";
const REFRESH_COMMAND = "editor.action.inlayHints.refresh";

const MODES = [
  { id: "contextual", label: "Contextual", description: "Show hints for selected code, inspected symbols, and Lua debugging" },
  {
    id: "full",
    label: "Full",
    description: "Show complete type and parameter hints",
  },
  {
    id: "compact",
    label: "Compact",
    description: "Show type and parameter hints as compact ellipses",
  },
  {
    id: "types",
    label: "Types Only",
    description: "Show inferred type hints only",
  },
  {
    id: "parameters",
    label: "Parameters Only",
    description: "Show parameter name hints only",
  },
  {
    id: "off",
    label: "Off",
    description: "Hide Tua inlay hints",
  },
];

function registerInlayHintPresentation({ context, configurationTarget, isTuaDocument, getClient }) {
  let inspected;
  let timer;
  const options = { context, configurationTarget, isTuaDocument, getClient, getInspected: () => inspected };
  const schedule = () => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      if (modeFromState(stateForTarget(context, configurationTarget())) === "contextual") void refreshInlayHints(getClient);
    }, 100);
  };
  const inspect = (document, position) => {
    if (!document || !isTuaDocument(document)) return;
    inspected = { uri: document.uri.toString(), line: position.line };
    schedule();
  };
  const listeners = [
    vscode.window.onDidChangeTextEditorSelection?.(() => { inspected = undefined; schedule(); }),
    vscode.window.onDidChangeActiveTextEditor?.(() => { inspected = undefined; schedule(); }),
    vscode.debug.onDidChangeActiveDebugSession?.(schedule),
    vscode.commands.registerCommand("tua.inlayHints.inspect", () => {
      const editor = vscode.window.activeTextEditor;
      if (editor) inspect(editor.document, editor.selection.active);
    }),
  ].filter(Boolean);
  const command = vscode.commands.registerCommand(COMMANDS.selectInlayHintMode, (requestedMode, fromSetup = false) =>
    selectInlayHintMode({ ...options, fromSetup: fromSetup === true }, requestedMode),
  );

  return {
    inspect,
    middleware: {
      provideInlayHints: (document, range, token, next) =>
        provideInlayHints(options, document, range, token, next),
    },
    dispose() {
      clearTimeout(timer);
      for (const listener of listeners) listener.dispose();
      command.dispose();
    },
  };
}

async function selectInlayHintMode(
  { context, configurationTarget, isTuaDocument, getClient, fromSetup = false },
  requestedMode,
) {
  const document = vscode.window.activeTextEditor?.document;
  if (!fromSetup && (!document || !isTuaDocument(document))) {
    vscode.window.showWarningMessage("Open a Tua document to select an inlay hint mode.");
    return undefined;
  }

  const target = configurationTarget();
  const state = stateForTarget(context, target);
  const currentMode = modeFromState(state);
  const mode = requestedMode
    ? normalizeMode(requestedMode)
    : await pickMode(currentMode);
  if (!mode) {
    if (requestedMode) {
      vscode.window.showWarningMessage(`Unknown Tua inlay hint mode: ${requestedMode}`);
    }
    return undefined;
  }

  const configuration = fromSetup
    ? vscode.workspace.getConfiguration("editor", {
        uri: vscode.workspace.workspaceFolders?.[0]?.uri,
        languageId: "tua",
      })
    : editorConfiguration(document);

  try {
    if (mode.id === "compact") {
      await enableCompactMode(configuration, state, target);
    } else {
      await restoreMaximumLength(configuration, state, target);
    }

    await state.update(MODE_STATE, mode.id);
    await vscode.commands.executeCommand("setContext", "tua.inlayHints.mode", mode.id);
    await refreshInlayHints(getClient);
    vscode.window.setStatusBarMessage(`Tua inlay hints: ${mode.label}`, 1800);

    return mode.id;
  } catch (error) {
    vscode.window.showErrorMessage(`Could not change Tua inlay hints: ${formatError(error)}`);
    return undefined;
  }
}

async function provideInlayHints(options, document, range, token, next) {
  const mode = modeForDocument(options, document);
  if (mode === "off") {
    return [];
  }

  const hints = await next(document, range, token);
  if (!Array.isArray(hints)) {
    return hints;
  }

  if (mode === "contextual") {
    const editor = vscode.window.activeTextEditor;
    const inspection = options.getInspected?.();
    const session = vscode.debug.activeDebugSession;
    const sameWorkspace = !session?.workspaceFolder || session.workspaceFolder.uri.toString() === vscode.workspace.getWorkspaceFolder(document.uri)?.uri.toString();
    return contextualHints(hints, {
      debugging: session?.type === "lua-local" && sameWorkspace,
      selections: editor?.document.uri.toString() === document.uri.toString() ? editor.selections || [editor.selection] : [],
      inspectedLine: inspection?.uri === document.uri.toString() ? inspection.line : undefined,
    });
  }
  if (mode === "types") {
    return hints.filter((hint) => hint.kind === vscode.InlayHintKind.Type);
  }
  if (mode === "parameters") {
    return hints.filter((hint) => hint.kind === vscode.InlayHintKind.Parameter);
  }

  return hints;
}

function modeForDocument({ context, configurationTarget, isTuaDocument }, document) {
  if (!isTuaDocument(document)) {
    return "full";
  }

  return modeFromState(stateForTarget(context, configurationTarget()));
}

async function pickMode(currentMode) {
  const selected = await vscode.window.showQuickPick(
    MODES.map((mode) => ({
      ...mode,
      picked: mode.id === currentMode,
      detail: mode.id === currentMode ? "Current mode" : undefined,
    })),
    {
      placeHolder: "Choose how Tua inlay hints are shown",
      ignoreFocusOut: true,
    },
  );

  return selected;
}

function normalizeMode(value) {
  const normalized = String(value).trim().toLowerCase();

  return MODES.find((mode) => mode.id === normalized);
}

function modeFromState(state) {
  return normalizeMode(state.get(MODE_STATE, "full"))?.id || "full";
}

async function enableCompactMode(configuration, state, target) {
  const previous = state.get(PREVIOUS_MAXIMUM_LENGTH_STATE);
  let captured = false;

  if (!previous) {
    const inspection = configuration.inspect(MAXIMUM_LENGTH_SETTING);
    const previousValue = languageValueForTarget(inspection, target);
    await state.update(PREVIOUS_MAXIMUM_LENGTH_STATE, {
      hadValue: previousValue !== undefined,
      value: previousValue,
    });
    captured = true;
  }

  try {
    await configuration.update(MAXIMUM_LENGTH_SETTING, COMPACT_MAXIMUM_LENGTH, target, true);
  } catch (error) {
    if (captured) {
      await state.update(PREVIOUS_MAXIMUM_LENGTH_STATE, undefined);
    }
    throw error;
  }
}

async function restoreMaximumLength(configuration, state, target) {
  const previous = state.get(PREVIOUS_MAXIMUM_LENGTH_STATE);
  if (!previous) {
    return;
  }

  await configuration.update(
    MAXIMUM_LENGTH_SETTING,
    previous.hadValue ? previous.value : undefined,
    target,
    true,
  );
  await state.update(PREVIOUS_MAXIMUM_LENGTH_STATE, undefined);
}

async function refreshInlayHints(getClient) {
  try {
    // Use the language client's existing refresh emitters, without changing user settings.
    const providers = getClient?.()?.getFeature?.("textDocument/inlayHint")?.getProviders?.();
    if (providers) {
      for (const provider of providers) provider.onDidChangeInlayHints.fire();
      return;
    }
    await vscode.commands.executeCommand(REFRESH_COMMAND);
  } catch {
    // Older hosts refresh on the next viewport or document change.
  }
}

function editorConfiguration(document) {
  return vscode.workspace.getConfiguration("editor", {
    uri: document.uri,
    languageId: "tua",
  });
}

function stateForTarget(context, target) {
  return target === vscode.ConfigurationTarget.Global ? context.globalState : context.workspaceState;
}

function languageValueForTarget(inspection, target) {
  if (!inspection) {
    return undefined;
  }

  if (target === vscode.ConfigurationTarget.Global) {
    return inspection.globalLanguageValue;
  }
  if (target === vscode.ConfigurationTarget.WorkspaceFolder) {
    return inspection.workspaceFolderLanguageValue;
  }

  return inspection.workspaceLanguageValue;
}

module.exports = {
  registerInlayHintPresentation,
};
