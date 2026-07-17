"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");

const MAXIMUM_LENGTH_SETTING = "inlayHints.maximumLength";
const COMPACT_MAXIMUM_LENGTH = 1;
const EXPANDED_DEFAULT_MAXIMUM_LENGTH = 43;
const PREVIOUS_MAXIMUM_LENGTH_STATE = "tua.inlayHints.previousMaximumLength";

function registerInlayHintPresentation({ context, configurationTarget, isTuaDocument }) {
  return vscode.commands.registerCommand(COMMANDS.toggleCompactInlayHints, () =>
    toggleCompactInlayHints({ context, configurationTarget, isTuaDocument }),
  );
}

async function toggleCompactInlayHints({ context, configurationTarget, isTuaDocument }) {
  const document = vscode.window.activeTextEditor?.document;
  if (!document || !isTuaDocument(document)) {
    vscode.window.showWarningMessage("Open a Tua document to toggle compact inlay hints.");
    return undefined;
  }

  const target = configurationTarget();
  const state = stateForTarget(context, target);
  const configuration = vscode.workspace.getConfiguration("editor", {
    uri: document.uri,
    languageId: "tua",
  });
  const currentMaximumLength = configuration.get(
    MAXIMUM_LENGTH_SETTING,
    EXPANDED_DEFAULT_MAXIMUM_LENGTH,
  );

  try {
    if (currentMaximumLength === COMPACT_MAXIMUM_LENGTH) {
      const previous = state.get(PREVIOUS_MAXIMUM_LENGTH_STATE);
      const restoredValue = previous
        ? previous.hadValue
          ? previous.value
          : undefined
        : EXPANDED_DEFAULT_MAXIMUM_LENGTH;

      await configuration.update(MAXIMUM_LENGTH_SETTING, restoredValue, target, true);
      await state.update(PREVIOUS_MAXIMUM_LENGTH_STATE, undefined);
      await vscode.commands.executeCommand("setContext", "tua.inlayHints.compact", false);
      vscode.window.setStatusBarMessage("Tua inlay hints expanded", 1800);
      return false;
    }

    const inspection = configuration.inspect(MAXIMUM_LENGTH_SETTING);
    const previousValue = languageValueForTarget(inspection, target);
    await state.update(PREVIOUS_MAXIMUM_LENGTH_STATE, {
      hadValue: previousValue !== undefined,
      value: previousValue,
    });

    try {
      await configuration.update(MAXIMUM_LENGTH_SETTING, COMPACT_MAXIMUM_LENGTH, target, true);
    } catch (error) {
      await state.update(PREVIOUS_MAXIMUM_LENGTH_STATE, undefined);
      throw error;
    }

    await vscode.commands.executeCommand("setContext", "tua.inlayHints.compact", true);
    vscode.window.setStatusBarMessage("Tua inlay hints compacted", 1800);
    return true;
  } catch (error) {
    vscode.window.showErrorMessage(`Could not toggle compact Tua inlay hints: ${formatError(error)}`);
    return undefined;
  }
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

function formatError(error) {
  return error instanceof Error ? error.message : String(error);
}

module.exports = {
  registerInlayHintPresentation,
};
