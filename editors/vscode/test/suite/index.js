"use strict";

const assert = require("assert/strict");
const vscode = require("vscode");
const { runCompletionTests } = require("./completion");
const { runEditorIntelligenceTests } = require("./editor-intelligence");
const { runSpritePreviewTests } = require("./sprite-preview");
const { fixtures, waitForEvent } = require("./support");
const { runWorkspaceTests } = require("./workspace");

async function run() {
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

  await runCompletionTests();
  await runEditorIntelligenceTests();
  await runWorkspaceTests();
  await runSpritePreviewTests();
}

module.exports = {
  run,
};
