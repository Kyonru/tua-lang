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
} = require("./support");

async function runCompletionTests() {
  let { document, editor } = await openFixture("suggestions");
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
  const insertion = document.lineAt(document.lineCount - 1).range.end;
  editor.selection = new vscode.Selection(insertion, insertion);
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  const beforeTyping = eventSequence();

  await insertText(document, editor, "o");
  const staleCompletion = vscode.commands.executeCommand(
    "vscode.executeCompletionItemProvider",
    document.uri,
    editor.selection.active,
  );
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
  const freshCompletion = vscode.commands.executeCommand(
    "vscode.executeCompletionItemProvider",
    document.uri,
    currentPosition,
  );
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
  const staleResponse = await waitForEvent(
    (event) => event.type === "completion.response" && event.id === staleRequest.id,
    "the deliberately late obsolete response",
    freshResponse.sequence,
  );

  assert(
    staleResponse.docVersion < freshResponse.docVersion,
    "the first response should belong to an obsolete document version",
  );
  assert(
    freshResponse.sequence < staleResponse.sequence,
    "the obsolete response should arrive after the synchronized result",
  );
  const [freshItems, staleItems] = await Promise.all([freshCompletion, staleCompletion]);
  assert.equal(freshItems.items[0].label, "outside_from_lsp");
  // The execute-provider command owns neither the suggest widget's cancellation
  // token nor its stale-result filtering. Keep this late response assertion to
  // prove document versions crossed the real language-client boundary; the
  // editor-facing newest-wins behavior is covered by the inline controller and
  // server scheduler tests.
  assert.equal(staleItems.items[0].label, "outdated_result");

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

  await discardActiveEditor();
  ({ document, editor } = await openFixture("suggestions"));
  const inlinePosition = document.lineAt(1).range.end;
  editor.selection = new vscode.Selection(inlinePosition, inlinePosition);
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  const beforeInline = eventSequence();
  await insertText(document, editor, "l");
  const extension = vscode.extensions.getExtension("SolenodonteLabs.tua");
  assert(extension, "the Tua extension should be available");
  const api = await extension.activate();
  // Headless Extension Hosts do not reliably ask VS Code's inline-completion
  // UI for ghost text. This test-only API invokes the same registered
  // controller with the real document and language client, and activation only
  // exposes it in ExtensionMode.Test with an explicit environment opt-in.
  assert.equal(typeof api?.requestInlineCompletion, "function");
  const inlineItems = await api.requestInlineCompletion(document, editor.selection.active);
  assert.equal(inlineItems?.length, 1);
  assert.equal(inlineItems[0].insertText, "late_result");
  const inlineRequest = await waitForEvent(
    (event) =>
      event.type === "inline-completion.request" &&
      event.uri === document.uri.toString() &&
      event.documentVersion === document.version,
    "a version-bound local inline-completion request after typing",
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
  await discardActiveEditor();
}

module.exports = {
  runCompletionTests,
};
