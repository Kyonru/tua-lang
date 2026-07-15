"use strict";

const assert = require("assert/strict");
const fs = require("fs");
const vscode = require("vscode");

async function run() {
  const extension = vscode.extensions.getExtension("tua-lang.tua");
  assert(extension, "the Tua extension should be available in the extension host");
  await extension.activate();

  const document = await vscode.workspace.openTextDocument(process.env.TUA_TEST_DOCUMENT);
  const editor = await vscode.window.showTextDocument(document);
  assert.equal(document.languageId, "tua");

  const quickSuggestions = vscode.workspace
    .getConfiguration("editor", document)
    .get("quickSuggestions");
  assert.equal(quickSuggestions.other, "on", "Tua code should use native quick suggestions");
  assert.equal(
    vscode.workspace.getConfiguration("editor", document).get("quickSuggestionsDelay"),
    0,
    "Tua quick suggestions should have no extension-added delay",
  );

  const insertion = document.lineAt(document.lineCount - 1).range.end;
  editor.selection = new vscode.Selection(insertion, insertion);

  await vscode.commands.executeCommand("type", { text: "o" });
  await waitForEvent(
    (event) => event.type === "completion.request" && event.prefix === "o",
    "an initial completion request for `o`",
  );
  await waitForEvent(
    (event) => event.type === "completion.response" && event.prefix === "o",
    "the initial incomplete completion response",
  );
  await delay(100);

  await vscode.commands.executeCommand("type", { text: "ut" });

  const currentVersion = document.version;
  const currentPosition = editor.selection.active;
  const freshRequest = await waitForEvent(
    (event) =>
      event.type === "completion.request" &&
      event.prefix === "out" &&
      event.docVersion === currentVersion,
    "a completion request synchronized to `out`",
  );

  assert.deepEqual(freshRequest.position, {
    character: currentPosition.character,
    line: currentPosition.line,
  });
  assert.equal(freshRequest.line, "out");

  await waitForEvent(
    (event) => event.type === "completion.response" && event.prefix === "out",
    "the current completion response",
  );
  await delay(100);

  const requests = readEvents().filter(
    (event) =>
      event.type === "completion.request" &&
      event.docVersion === currentVersion &&
      event.position.line === currentPosition.line &&
      event.position.character === currentPosition.character,
  );
  assert.equal(requests.length, 1, "one edit state should issue one completion request");

  await vscode.commands.executeCommand("acceptSelectedSuggestion");
  await waitUntil(
    () => document.getText().endsWith("outside_from_lsp"),
    "the fresh visible suggestion to be accepted",
  );
  assert(
    !document.getText().endsWith("outdated_result"),
    "the older result must not replace the current suggestion",
  );

  await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
}

function readEvents() {
  try {
    return fs
      .readFileSync(process.env.TUA_TEST_LSP_LOG, "utf8")
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  } catch (error) {
    if (error.code === "ENOENT") {
      return [];
    }
    throw error;
  }
}

async function waitForEvent(predicate, description) {
  let matched;
  await waitUntil(() => {
    matched = readEvents().find(predicate);
    return Boolean(matched);
  }, description);
  return matched;
}

async function waitUntil(predicate, description, timeoutMs = 5000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) {
      return;
    }
    await delay(25);
  }
  throw new Error(`Timed out waiting for ${description}. Events: ${JSON.stringify(readEvents())}`);
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

module.exports = {
  run,
};
