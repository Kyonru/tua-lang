"use strict";

const assert = require("assert/strict");
const vscode = require("vscode");
const {
  closeActiveEditor,
  eventSequence,
  moduleGraphTabs,
  openFixture,
  waitForEvent,
  waitUntil,
} = require("./support");

async function runModuleGraphTests() {
  const { document } = await openFixture("moduleGraphRoot");
  const beforeRequest = eventSequence();

  await vscode.commands.executeCommand("tua.whyModuleIncluded");
  const request = await waitForEvent(
    (event) =>
      event.type === "moduleGraph.request" && event.rootUri === document.uri.toString(),
    "the compiler-backed module graph request for the active workspace file",
    beforeRequest,
  );
  assert(request.sequence > beforeRequest);
  await waitUntil(
    () => moduleGraphTabs().length === 1 && moduleGraphTabs()[0].isActive,
    "one active module graph tab",
  );

  await vscode.commands.executeCommand("tua.showModuleGraph");
  await waitUntil(
    () => moduleGraphTabs().length === 1 && moduleGraphTabs()[0].isActive,
    "the existing module graph tab to be reused",
  );

  await closeActiveEditor();
  await vscode.window.showTextDocument(document);
  await closeActiveEditor();
}

module.exports = {
  runModuleGraphTests,
};
