"use strict";

const assert = require("assert/strict");
const vscode = require("vscode");
const {
  discardActiveEditor,
  eventSequence,
  hoverText,
  insertText,
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
  await vscode.commands.executeCommand("tua.inlayHints.selectMode", "full");
  const insertion = document.lineAt(document.lineCount - 1).range.end;
  editor.selection = new vscode.Selection(insertion, insertion);
  const beforeTyping = eventSequence();

  await insertText(document, editor, "local live_probe = target_value");
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
  assert.match(hoverText(hovers), /command:tua\.hover\.references/);
  assert.match(hoverText(hovers), /command:tua\.hover\.implementations/);

  const referenceCount = await vscode.commands.executeCommand("tua.hover.references", {
    uri: document.uri.toString(),
    position: {
      line: targetPosition.line,
      character: targetPosition.character,
    },
  });
  assert(referenceCount >= 2);
  const implementationCount = await vscode.commands.executeCommand("tua.hover.implementations", {
    uri: document.uri.toString(),
    position: {
      line: targetPosition.line,
      character: targetPosition.character,
    },
  });
  assert.equal(implementationCount, 1);

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

  const greetPosition = positionAfter(document, "local function gre");
  editor.selection = new vscode.Selection(greetPosition, greetPosition);
  const qualifiedName = await vscode.commands.executeCommand("tua.copyQualifiedName");
  assert.equal(qualifiedName, "intelligence.greet");
  assert.equal(await vscode.env.clipboard.readText(), "intelligence.greet");

  const pickedName = await vscode.commands.executeCommand("tua.showWorkspaceSymbolsByModule", {
    query: "greet",
    pickFirst: true,
  });
  assert.equal(pickedName, "intelligence.greet");
  assert.equal(vscode.window.activeTextEditor?.document.uri.toString(), document.uri.toString());
  assert.equal(vscode.window.activeTextEditor?.selection.active.line, 1);

  const signaturePosition = positionAfter(document, "call_probe = greet(");
  const signatures = await vscode.commands.executeCommand(
    "vscode.executeSignatureHelpProvider",
    document.uri,
    signaturePosition,
    "(",
  );
  assert.equal(signatures.signatures[0].label, "greet(name: string): string");
  assert.equal(signatures.activeParameter, 0);

  const eventPosition = positionAfter(document, 'events:emit("player.hit');
  const eventHovers = await vscode.commands.executeCommand(
    "vscode.executeHoverProvider",
    document.uri,
    eventPosition,
  );
  const eventHoverText = hoverText(eventHovers);
  assert.match(eventHoverText, /command:tua\.hover\.references/);
  assert.match(eventHoverText, /command:tua\.showSignalGraph/);
  assert.doesNotMatch(eventHoverText, /command:tua\.hover\.implementations/);

  const hints = await vscode.commands.executeCommand(
    "vscode.executeInlayHintProvider",
    document.uri,
    new vscode.Range(0, 0, document.lineCount, 0),
  );
  assert(hints.some((hint) => hint.label === ": number"));

  await testInlayHintModes(document);

  for (const type of [
    "hover.request",
    "definition.request",
    "references.request",
    "implementation.request",
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

async function testInlayHintModes(document) {
  const configuration = () =>
    vscode.workspace.getConfiguration("editor", { uri: document.uri, languageId: "tua" });
  const previousMaximumLength = configuration().inspect(
    "inlayHints.maximumLength",
  )?.workspaceLanguageValue;
  const previousEnabled = configuration().inspect("inlayHints.enabled")?.workspaceLanguageValue;
  const range = new vscode.Range(0, 0, document.lineCount, 0);
  const selectMode = (mode) =>
    vscode.commands.executeCommand("tua.inlayHints.selectMode", mode);
  const hints = () =>
    vscode.commands.executeCommand("vscode.executeInlayHintProvider", document.uri, range);

  try {
    await configuration().update(
      "inlayHints.enabled",
      "on",
      vscode.ConfigurationTarget.Workspace,
      true,
    );
    await configuration().update(
      "inlayHints.maximumLength",
      27,
      vscode.ConfigurationTarget.Workspace,
      true,
    );
    assert.equal(
      configuration().get("inlayHints.maximumLength"),
      27,
      "the test host should apply Tua language-specific inlay settings",
    );

    assert.equal(await selectMode("compact"), "compact");
    assert.equal(configuration().get("inlayHints.maximumLength"), 1);

    const compactHints = await hints();
    assert(
      compactHints.some((hint) => hint.label === ": number"),
      "compact presentation must not replace the LSP's full inlay hint labels",
    );
    assert(
      compactHints.some((hint) => hint.label === "value:"),
      "compact mode should retain parameter hints",
    );

    assert.equal(await selectMode("types"), "types");
    assert.equal(
      configuration().get("inlayHints.maximumLength"),
      27,
      "leaving compact mode should restore the previous Tua language override",
    );
    const typeHints = await hints();
    assert(typeHints.length > 0, "types-only mode should retain type hints");
    assert(
      typeHints.every((hint) => hint.kind === vscode.InlayHintKind.Type),
      "types-only mode should filter parameter hints",
    );

    assert.equal(await selectMode("parameters"), "parameters");
    const parameterHints = await hints();
    assert(parameterHints.length > 0, "parameters-only mode should retain parameter hints");
    assert(
      parameterHints.every((hint) => hint.kind === vscode.InlayHintKind.Parameter),
      "parameters-only mode should filter type hints",
    );

    assert.equal(await selectMode("off"), "off");
    assert.deepEqual(await hints(), [], "off mode should suppress Tua inlay hints");

    assert.equal(await selectMode("full"), "full");
    const fullHints = await hints();
    assert(fullHints.some((hint) => hint.kind === vscode.InlayHintKind.Type));
    assert(fullHints.some((hint) => hint.kind === vscode.InlayHintKind.Parameter));
    assert.equal(
      configuration().inspect("inlayHints.enabled")?.workspaceLanguageValue,
      "on",
      "mode changes should preserve the existing VS Code inlay hint setting",
    );

    await configuration().update(
      "inlayHints.maximumLength",
      undefined,
      vscode.ConfigurationTarget.Workspace,
      true,
    );
    assert.equal(await selectMode("compact"), "compact");
    assert.equal(await selectMode("full"), "full");
    assert.equal(
      configuration().inspect("inlayHints.maximumLength")?.workspaceLanguageValue,
      undefined,
      "leaving compact mode should remove the temporary override when none existed before",
    );
  } finally {
    await selectMode("full");
    await configuration().update(
      "inlayHints.maximumLength",
      previousMaximumLength,
      vscode.ConfigurationTarget.Workspace,
      true,
    );
    await configuration().update(
      "inlayHints.enabled",
      previousEnabled,
      vscode.ConfigurationTarget.Workspace,
      true,
    );
  }
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
