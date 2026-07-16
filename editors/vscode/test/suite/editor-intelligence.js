"use strict";

const assert = require("assert/strict");
const vscode = require("vscode");
const {
  discardActiveEditor,
  eventSequence,
  hoverText,
  openFixture,
  positionAfter,
  waitForEvent,
  waitUntil,
} = require("./support");

async function runEditorIntelligenceTests() {
  await testLiveLanguageFeatures();
  await testUtf16AndUntitledDocuments();
}

async function testLiveLanguageFeatures() {
  const { document, editor } = await openFixture("intelligence");
  const insertion = document.lineAt(document.lineCount - 1).range.end;
  editor.selection = new vscode.Selection(insertion, insertion);
  const beforeTyping = eventSequence();

  await vscode.commands.executeCommand("type", {
    text: "local live_probe = target_value",
  });
  await waitForEvent(
    (event) =>
      event.type === "document.change" &&
      event.uri === document.uri.toString() &&
      event.version === document.version,
    "the live intelligence edit to reach the language server",
    beforeTyping,
  );

  const targetPosition = positionAfter(document, "hover_probe = target_valu");
  const requestStart = eventSequence();
  const hovers = await vscode.commands.executeCommand(
    "vscode.executeHoverProvider",
    document.uri,
    targetPosition,
  );
  assert.match(hoverText(hovers), /target_value.*number/s);

  const definitions = await vscode.commands.executeCommand(
    "vscode.executeDefinitionProvider",
    document.uri,
    targetPosition,
  );
  assert.equal(definitions.length, 1);
  assert.equal(definitions[0].uri.toString(), document.uri.toString());
  assert.deepEqual(definitions[0].range.start, new vscode.Position(0, 6));

  editor.selection = new vscode.Selection(targetPosition, targetPosition);
  const beforeGoToDefinition = eventSequence();
  await vscode.commands.executeCommand("editor.action.revealDefinition");
  await waitForEvent(
    (event) =>
      event.type === "definition.request" &&
      event.uri === document.uri.toString() &&
      event.sequence > beforeGoToDefinition,
    "Cmd+click-equivalent definition navigation",
    beforeGoToDefinition,
  );
  await waitUntil(
    () =>
      vscode.window.activeTextEditor?.document.uri.toString() === document.uri.toString() &&
      vscode.window.activeTextEditor.selection.active.line === 0,
    "definition navigation to reveal the declaration",
  );

  const signaturePosition = positionAfter(document, "call_probe = greet(");
  const signatures = await vscode.commands.executeCommand(
    "vscode.executeSignatureHelpProvider",
    document.uri,
    signaturePosition,
    "(",
  );
  assert.equal(signatures.signatures[0].label, "greet(name: string): string");
  assert.equal(signatures.activeParameter, 0);

  const hints = await vscode.commands.executeCommand(
    "vscode.executeInlayHintProvider",
    document.uri,
    new vscode.Range(0, 0, document.lineCount, 0),
  );
  assert(hints.some((hint) => hint.label === ": number"));

  for (const type of [
    "hover.request",
    "definition.request",
    "signature.request",
    "inlay.request",
  ]) {
    await waitForEvent(
      (event) =>
        event.type === type &&
        event.uri === document.uri.toString() &&
        event.docVersion === document.version,
      `${type} at the current document version`,
      requestStart,
    );
  }

  await discardActiveEditor();
}

async function testUtf16AndUntitledDocuments() {
  const source = 'local face = "😀"; local utf_target = 1\nlocal use = utf_target';
  const document = await vscode.workspace.openTextDocument({
    content: source,
    language: "tua",
  });
  await vscode.window.showTextDocument(document, { preview: false });
  const line = document.lineAt(0).text;
  const position = new vscode.Position(0, line.indexOf("utf_target") + "utf_target".length);
  const beforeHover = eventSequence();

  const hovers = await vscode.commands.executeCommand(
    "vscode.executeHoverProvider",
    document.uri,
    position,
  );
  assert.match(hoverText(hovers), /utf_target/);

  const request = await waitForEvent(
    (event) =>
      event.type === "hover.request" &&
      event.uri === document.uri.toString() &&
      event.docVersion === document.version,
    "hover for an untitled UTF-16 document",
    beforeHover,
  );
  assert.equal(document.uri.scheme, "untitled");
  assert.equal(
    request.position.character,
    line.indexOf("utf_target") + "utf_target".length,
    "LSP character offsets should use UTF-16 code units after an emoji",
  );

  await discardActiveEditor();
}

module.exports = {
  runEditorIntelligenceTests,
};
