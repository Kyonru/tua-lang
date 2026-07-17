"use strict";

const assert = require("assert/strict");
const vscode = require("vscode");
const {
  closeActiveEditor,
  ecsExplorerTabs,
  eventSequence,
  openFixture,
  waitForEvent,
  waitUntil,
} = require("./support");

async function runEcsExplorerTests() {
  const { document } = await openFixture("ecs");
  const beforeRequest = eventSequence();

  await vscode.commands.executeCommand("tua.showEcsExplorer");
  const request = await waitForEvent(
    (event) => event.type === "ecsGraph.request" && event.rootUri === document.uri.toString(),
    "the compiler-backed ECS graph request for the active workspace file",
    beforeRequest,
  );
  assert(request.sequence > beforeRequest);
  await waitUntil(
    () => ecsExplorerTabs().length === 1 && ecsExplorerTabs()[0].isActive,
    "one active ECS Explorer tab",
  );

  await vscode.commands.executeCommand("tua.showEcsExplorer");
  await waitUntil(
    () => ecsExplorerTabs().length === 1 && ecsExplorerTabs()[0].isActive,
    "the existing ECS Explorer tab to be reused",
  );

  await closeActiveEditor();
  await vscode.window.showTextDocument(document);
  await closeActiveEditor();
}

module.exports = {
  runEcsExplorerTests,
};
