"use strict";

const assert = require("assert/strict");
const vscode = require("vscode");
const { runCompletionTests } = require("./completion");
const { runEditorIntelligenceTests } = require("./editor-intelligence");
const { runModuleGraphTests } = require("./module-graph");
const { runSpritePreviewTests } = require("./sprite-preview");
const { fixtures, waitForEvent } = require("./support");
const { runWorkspaceTests } = require("./workspace");

async function run() {
  const selectedSuite = process.env.TUA_TEST_SUITE || "all";
  const supportedSuites = new Set([
    "all",
    "completion",
    "editor-intelligence",
    "module-graph",
    "workspace",
    "sprite-preview",
  ]);

  assert(
    supportedSuites.has(selectedSuite),
    `unsupported TUA_TEST_SUITE value: ${selectedSuite}`,
  );

  const extension = vscode.extensions.getExtension("tua-lang.tua");
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

  if (selectedSuite === "all" || selectedSuite === "completion") {
    await runCompletionTests();
  }
  if (selectedSuite === "all" || selectedSuite === "editor-intelligence") {
    await runEditorIntelligenceTests();
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
