"use strict";

const assert = require("assert/strict");
const vscode = require("vscode");

async function runSetupTests() {
  const config = () => vscode.workspace.getConfiguration(undefined, null);
  const before = (key) => config().inspect(key)?.globalValue;
  const appearanceKeys = [
    "workbench.colorTheme", "workbench.iconTheme", "window.commandCenter",
    "workbench.editor.editorActionsLocation", "workbench.layoutControl.enabled",
    "workbench.layoutControl.type", "tua.editor.toolbarMode",
  ];
  for (const key of appearanceKeys) {
    assert.equal(before(key), undefined, `activation must not write ${key}`);
  }

  await vscode.commands.executeCommand("tua.openSetup");
  for (const key of appearanceKeys) {
    assert.equal(before(key), undefined, `opening setup must not write ${key}`);
  }

  await vscode.commands.executeCommand("tua.setup.applyTheme");
  assert.equal(config().get("workbench.colorTheme"), "Indigo Carbon HC");
  await vscode.commands.executeCommand("tua.setup.applyTheme");
  await vscode.commands.executeCommand("tua.setup.restoreTheme");
  assert.equal(before("workbench.colorTheme"), undefined);

  await vscode.commands.executeCommand("tua.setup.applyLayout");
  assert.equal(config().get("workbench.editor.editorActionsLocation"), "titleBar");
  assert.equal(config().get("workbench.layoutControl.enabled"), true);
  assert.equal(config().get("workbench.layoutControl.type"), "both");
  assert.equal(config().get("window.commandCenter"), true);
  await config().update("workbench.layoutControl.type", "toggles", vscode.ConfigurationTarget.Global);
  await vscode.commands.executeCommand("tua.setup.restoreLayout");
  assert.equal(before("workbench.editor.editorActionsLocation"), undefined);
  assert.equal(before("workbench.layoutControl.type"), "toggles", "restore must preserve manual edits");
  await config().update("workbench.layoutControl.type", undefined, vscode.ConfigurationTarget.Global);

  // A walkthrough is active, with no Tua document in the editor.
  assert.equal(await vscode.commands.executeCommand("tua.inlayHints.selectMode", "compact", true), "compact");
  const editor = () => vscode.workspace.getConfiguration("editor", { languageId: "tua" });
  assert.equal(editor().get("inlayHints.maximumLength"), 1);
  assert.equal(await vscode.commands.executeCommand("tua.inlayHints.selectMode", "full", true), "full");
  assert.equal(editor().inspect("inlayHints.maximumLength").workspaceLanguageValue, undefined);

  if (process.env.TUA_SETUP_PREVIEW === "1") {
    await vscode.commands.executeCommand("tua.setup.applyTheme");
    await vscode.commands.executeCommand("tua.openSetup");
    // Give the external screenshot harness time to capture the native page.
    await new Promise((resolve) => setTimeout(resolve, 30000));
  }
  console.log("Setup integration passed: optional activation, native walkthrough, presets, restore, and workspace hints.");
}

module.exports = { runSetupTests };
