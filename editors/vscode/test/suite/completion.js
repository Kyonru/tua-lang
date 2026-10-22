"use strict";

const assert = require("assert/strict");
const vscode = require("vscode");
const {
  discardActiveEditor,
  eventSequence,
  insertText,
  openFixture,
  readEvents,
  waitForEvent,
  waitUntil,
} = require("./support");

async function runCompletionTests() {
  const { document, editor } = await openFixture("suggestions");
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

  const inlineConfiguration = vscode.workspace.getConfiguration("tua", document);
  assert.equal(
    inlineConfiguration.get("completion.inline.enabled"),
    true,
    "the integration workspace must enable local inline completion before activation",
  );
  const beforeInline = eventSequence();
  await vscode.commands.executeCommand("editor.action.inlineSuggest.trigger");
  const inlineRequest = await waitForEvent(
    (event) =>
      event.type === "inline-completion.request" &&
      event.uri === document.uri.toString() &&
      event.documentVersion === document.version,
    "a version-bound local inline-completion request",
    beforeInline,
  );
  assert.deepEqual(inlineRequest.position, {
    character: editor.selection.active.character,
    line: editor.selection.active.line,
  });
  await inlineConfiguration.update(
    "completion.inline.enabled",
    false,
    vscode.ConfigurationTarget.Workspace,
  );

  const insertion = document.lineAt(document.lineCount - 1).range.end;
  editor.selection = new vscode.Selection(insertion, insertion);
  const beforeTyping = eventSequence();

  await insertText(document, editor, "o");
  await vscode.commands.executeCommand("editor.action.triggerSuggest");
  const staleRequest = await waitForEvent(
    (event) => event.type === "completion.request" && event.prefix === "o",
    "an initial completion request for `o`",
    beforeTyping,
  );

  await insertText(document, editor, "u");
  await insertText(document, editor, "t");

  const currentVersion = document.version;
  const currentPosition = editor.selection.active;
  await waitForEvent(
    (event) =>
      event.type === "document.change" &&
      event.uri === document.uri.toString() &&
      event.version === currentVersion,
    "the rapid edits to synchronize before retriggering suggestions",
    staleRequest.sequence,
  );
  await vscode.commands.executeCommand("editor.action.triggerSuggest");
  const freshRequest = await waitForEvent(
    (event) =>
      event.type === "completion.request" &&
      event.prefix === "out" &&
      event.docVersion === currentVersion,
    "a completion request synchronized to `out`",
    staleRequest.sequence,
  );

  assert.deepEqual(freshRequest.position, {
    character: currentPosition.character,
    line: currentPosition.line,
  });
  assert.equal(freshRequest.line, "out");

  const freshResponse = await waitForEvent(
    (event) => event.type === "completion.response" && event.id === freshRequest.id,
    "the current completion response",
    freshRequest.sequence,
  );
  const cancellation = await waitForEvent(
    (event) => event.type === "request.cancel" && event.id === staleRequest.id,
    "the obsolete completion request to be cancelled",
    staleRequest.sequence,
  );
  const staleResponse = await waitForEvent(
    (event) => event.type === "completion.response" && event.id === staleRequest.id,
    "the deliberately late obsolete response",
    cancellation.sequence,
  );

  assert(
    staleResponse.docVersion < freshResponse.docVersion,
    "the first response should belong to an obsolete document version",
  );
  assert(
    freshResponse.sequence < staleResponse.sequence,
    "the obsolete response should arrive after the synchronized result",
  );
  assert.equal(staleResponse.cancelled, true);

  const currentRequests = readEvents().filter(
    (event) =>
      event.type === "completion.request" &&
      event.docVersion === currentVersion &&
      event.position.line === currentPosition.line &&
      event.position.character === currentPosition.character,
  );
  assert(currentRequests.length >= 1, "the synchronized edit state should request completion");

  const typingChanges = readEvents().filter(
    (event) =>
      event.type === "document.change" &&
      event.uri === document.uri.toString() &&
      event.sequence > beforeTyping,
  );
  assert(
    typingChanges.length >= 2,
    "rapid single-character edits should synchronize the initial and coalesced final states",
  );
  assert(
    typingChanges.every(
      (change, index) => index === 0 || change.version > typingChanges[index - 1].version,
    ),
    "synchronized document versions should increase monotonically",
  );
  assert.equal(typingChanges.at(-1).version, currentVersion);
  assert(typingChanges.at(-1).text.endsWith("out"));

  await vscode.commands.executeCommand("acceptSelectedSuggestion");
  await waitUntil(
    () => document.getText().endsWith("outside_from_lsp"),
    "the fresh visible suggestion to be accepted",
  );
  assert(
    !document.getText().endsWith("outdated_result"),
    "the older response must not replace the current suggestion",
  );

  await discardActiveEditor();
}

module.exports = {
  runCompletionTests,
};
