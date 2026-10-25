"use strict";

const assert = require("assert/strict");
const vscode = require("vscode");
const { runCompletionTests } = require("./completion");
const { runEditorIntelligenceTests } = require("./editor-intelligence");
const { runEcsExplorerTests } = require("./ecs-explorer");
const { runModuleGraphTests } = require("./module-graph");
const { runSpritePreviewTests } = require("./sprite-preview");
const { fixtures, waitForEvent } = require("./support");
const { runSetupTests } = require("./setup");
const { runWorkspaceTests } = require("./workspace");

async function run() {
  const selectedSuite = process.env.TUA_TEST_SUITE || "all";
  const supportedSuites = new Set([
    "all",
    "setup",
    "completion",
    "editor-intelligence",
    "ecs-explorer",
    "module-graph",
    "workspace",
    "sprite-preview",
  ]);

  assert(
    supportedSuites.has(selectedSuite),
    `unsupported TUA_TEST_SUITE value: ${selectedSuite}`,
  );

  const extension = vscode.extensions.getExtension("SolenodonteLabs.tua");
  assert(extension, "the Tua extension should be available in the extension host");

  await extension.activate();
  const initialized = await waitForEvent(
    (event) => event.type === "initialize",
    "the language client to initialize",
  );

  assert.deepEqual(
    initialized.workspaceFolders.map((folder) => folder.uri).sort(),
    [
      vscode.Uri.file(fixtures.workspaceOne).toString(),
      vscode.Uri.file(fixtures.workspaceTwo).toString(),
    ].sort(),
    "the language client should initialize with both workspace folders",
  );
  assert.equal(
    initialized.initializationOptions?.combinedInlineCompletionProvider,
    true,
    "the extension should identify its combined inline provider so the client does not register a competing standard provider",
  );

  await vscode.commands.executeCommand("tua.copyDebugInfo");
  assert.match(
    await vscode.env.clipboard.readText(),
    /Server state: running/,
    "the real language client should remain running after initialization",
  );

  if (selectedSuite === "all" || selectedSuite === "setup") {
    await runSetupTests();
  }
  if (selectedSuite === "all" || selectedSuite === "completion") {
    await runCompletionTests();
  }
  if (selectedSuite === "all" || selectedSuite === "editor-intelligence") {
    await runEditorIntelligenceTests();
  }
  if (selectedSuite === "all" || selectedSuite === "ecs-explorer") {
    await runEcsExplorerTests();
  }
  if (selectedSuite === "all" || selectedSuite === "module-graph") {
    await runModuleGraphTests();
  }
  if (selectedSuite === "all" || selectedSuite === "workspace") {
    await runWorkspaceTests();
  }
  if (selectedSuite === "all" || selectedSuite === "sprite-preview") {
    await runSpritePreviewTests();
  }
}

module.exports = {
  run,
};
