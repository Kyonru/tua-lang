"use strict";

const vscode = require("vscode");

async function ensureExperimentalPreviewEnabled(configurationTarget, featureLabel) {
  const config = vscode.workspace.getConfiguration("tua");

  if (config.get("preview.enabled", false)) {
    return true;
  }

  const choice = await vscode.window.showInformationMessage(
    `${featureLabel} is experimental. Enable preview features for this workspace?`,
    "Enable",
    "Open Settings",
  );

  if (choice === "Enable") {
    await config.update("preview.enabled", true, configurationTarget());
    return true;
  }

  if (choice === "Open Settings") {
    await vscode.commands.executeCommand(
      "workbench.action.openSettings",
      "tua.preview.enabled",
    );
  }

  return false;
}

module.exports = {
  ensureExperimentalPreviewEnabled,
};
