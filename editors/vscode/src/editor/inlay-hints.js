"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { formatError } = require("../shared/errors");

const MAXIMUM_LENGTH_SETTING = "inlayHints.maximumLength";
const COMPACT_MAXIMUM_LENGTH = 1;
const MODE_STATE = "tua.inlayHints.mode";
const PREVIOUS_MAXIMUM_LENGTH_STATE = "tua.inlayHints.previousMaximumLength";
const REFRESH_COMMAND = "editor.action.inlayHints.refresh";

const MODES = [
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

function registerInlayHintPresentation({ context, configurationTarget, isTuaDocument }) {
  const options = { context, configurationTarget, isTuaDocument };
  const command = vscode.commands.registerCommand(COMMANDS.selectInlayHintMode, (requestedMode) =>
    selectInlayHintMode(options, requestedMode),
  );

  return {
    middleware: {
      provideInlayHints: (document, range, token, next) =>
        provideInlayHints(options, document, range, token, next),
    },
    dispose() {
      command.dispose();
    },
  };
}

async function selectInlayHintMode(
  { context, configurationTarget, isTuaDocument },
  requestedMode,
) {
  const document = vscode.window.activeTextEditor?.document;
  if (!document || !isTuaDocument(document)) {
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

  const configuration = editorConfiguration(document);

  try {
    if (mode.id === "compact") {
      await enableCompactMode(configuration, state, target);
    } else {
      await restoreMaximumLength(configuration, state, target);
    }

    await state.update(MODE_STATE, mode.id);
    await vscode.commands.executeCommand("setContext", "tua.inlayHints.mode", mode.id);
    await refreshInlayHints();
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

async function refreshInlayHints() {
  try {
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
