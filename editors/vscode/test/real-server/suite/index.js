"use strict";

const assert = require("assert/strict");
const path = require("path");
const vscode = require("vscode");
const { previewContext, verifyGraphPreviews } = require("./graph-previews");

const fixtures = JSON.parse(process.env.TUA_REAL_SERVER_FIXTURES);

async function run() {
  const extension = vscode.extensions.getExtension("SolenodonteLabs.tua");
  assert(extension, "the Tua extension should be available in the extension host");

  await extension.activate();
  await waitForServer();

  const protocolStubDocument = await vscode.workspace.openTextDocument(fixtures.protocolStubsPath);
  await vscode.window.showTextDocument(protocolStubDocument, { preview: false });
  await retry(
    () => vscode.languages.getDiagnostics(protocolStubDocument.uri),
    (diagnostics) => diagnostics.some((diagnostic) => diagnostic.message.includes("missing method `draw`")),
    "imported protocol requirement diagnostic for the inheriting class",
  );
  const protocolStubActions = await vscode.commands.executeCommand(
    "vscode.executeCodeActionProvider",
    protocolStubDocument.uri,
    new vscode.Range(
      new vscode.Position(0, 0),
      protocolStubDocument.positionAt(protocolStubDocument.getText().length),
    ),
    vscode.CodeActionKind.QuickFix.value,
  );
  const protocolStubAction = (protocolStubActions || []).find(
    (action) => action.title === "Implement missing protocol methods",
  );
  assert(protocolStubAction?.edit, "the installed extension should offer the protocol stub quick fix");
  assert(await vscode.workspace.applyEdit(protocolStubAction.edit));
  await retry(
    () => vscode.languages.getDiagnostics(protocolStubDocument.uri),
    (diagnostics) => !diagnostics.some((diagnostic) => diagnostic.message.includes("missing method `draw`")),
    "protocol stub edit to clear the missing-method diagnostic",
  );
  assert.match(protocolStubDocument.getText(), /error\("TODO: implement Actor\.draw"\)/);

  const document = await vscode.workspace.openTextDocument(fixtures.mainPath);
  await vscode.window.showTextDocument(document, { preview: false });
  assert.equal(document.languageId, "tua");

  const completionEvidence = {
    tua: await expectCompletion(document, "player.health", "player.", "health"),
    lua: await expectCompletion(document, "string.upper", "string.", "upper"),
    love: await expectCompletion(
      document,
      "love.graphics.getWidth",
      "love.graphics.",
      "getWidth",
    ),
    genericAlias: await expectCompletion(document, "score.value", "score.", "value"),
    protocol: await expectCompletion(document, "value:update(dt)", "value:up", "update"),
  };

  const headerCompletionDocument = await vscode.workspace.openTextDocument(
    fixtures.headerCompletionPath,
  );
  await vscode.window.showTextDocument(headerCompletionDocument, { preview: false });
  const genericBoundCompletion = await expectCompletion(
    headerCompletionDocument,
    "T: Up",
    "T: Up",
    "Updatable",
  );
  const implementsCompletion = await expectCompletion(
    headerCompletionDocument,
    "> implements Up",
    "> implements Up",
    "Updatable",
  );

  const genericFieldPosition = positionAfter(document, "score.value", "score.");
  const genericFieldHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        document.uri,
        genericFieldPosition,
      ),
    (result) => hoverText(result).includes("number"),
    "generic alias instantiated field hover",
  );
  assert.match(hoverText(genericFieldHover), /number/);

  const importedGenericPosition = positionAfter(
    document,
    'Generic.identity("ready")',
    "Generic.identity",
  );
  const importedGenericHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        document.uri,
        importedGenericPosition,
      ),
    (result) => hoverText(result).includes("identity<T>"),
    "imported generic function hover",
  );
  assert.match(hoverText(importedGenericHover), /identity<T>\(value: T\): T/);

  const importedGenericDefinitionPosition = positionAfter(
    document,
    'Generic.identity("ready")',
    "Generic.identity",
  );
  const importedGenericDefinition = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeDefinitionProvider",
        document.uri,
        importedGenericDefinitionPosition,
      ),
    (result) =>
      Array.isArray(result) &&
      result.some((location) => locationPath(location) === fixtures.genericPath),
    "imported generic function definition",
  );
  const genericFunctionLocation = importedGenericDefinition.find(
    (location) => locationPath(location) === fixtures.genericPath,
  );
  const genericFunctionDocument = await vscode.workspace.openTextDocument(fixtures.genericPath);
  const genericFunctionDeclarationOffset = genericFunctionDocument
    .getText()
    .indexOf("function identity<T>");
  assert(genericFunctionDeclarationOffset >= 0);
  assert.equal(
    genericFunctionLocation.range.start.line,
    genericFunctionDocument.positionAt(genericFunctionDeclarationOffset).line,
    "the imported generic function definition should reach its declaration",
  );

  const importedGenericSignaturePosition = positionAfter(
    document,
    'Generic.identity("ready")',
    'Generic.identity("ready"',
  );
  const importedGenericSignature = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeSignatureHelpProvider",
        document.uri,
        importedGenericSignaturePosition,
        "(",
      ),
    (result) => result?.signatures?.length > 0,
    "imported generic function signature help",
  );
  assert.match(
    importedGenericSignature.signatures[0].label,
    /Generic\.identity\(value: string\): string/,
  );
  const importedGenericResultPosition = positionAfter(
    document,
    "generic_text = generic_text",
    "generic_text = ",
  );
  const importedGenericResultHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        document.uri,
        importedGenericResultPosition,
      ),
    (result) => hoverText(result).includes("string"),
    "imported generic call result hover",
  );
  assert.match(hoverText(importedGenericResultHover), /local generic_text: string/);

  const importedBoxHoverPosition = positionAfter(
    document,
    "local imported_box = BoxModule.Box.new(42)",
    "local imported_box",
  );
  const importedBoxHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        document.uri,
        importedBoxHoverPosition,
      ),
    (result) => /Box<number>/.test(hoverText(result)),
    "imported generic class constructor result hover",
  );
  assert.match(hoverText(importedBoxHover), /Box<number>/);

  const importedBoxAliasPosition = positionAfter(
    document,
    "typed_box: BoxInstance<number>",
    "typed_box: ",
  );
  const importedBoxAliasHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        document.uri,
        importedBoxAliasPosition,
      ),
    (result) => /BoxInstance<T>|Box<T>/.test(hoverText(result)),
    "imported generic instance alias hover",
  );
  assert.match(hoverText(importedBoxAliasHover), /BoxInstance<T>/);
  assert.match(hoverText(importedBoxAliasHover), /Box<T>/);

  const importedBoxValueHoverPosition = positionAfter(
    document,
    "typed_box: BoxInstance<number>",
    "typed_box",
  );
  const importedBoxValueHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        document.uri,
        importedBoxValueHoverPosition,
      ),
    (result) => hoverText(result).includes("number"),
    "imported generic instance value hover",
  );
  assert.match(hoverText(importedBoxValueHover), /number/);

  const importedBoxFieldPosition = positionAfter(
    document,
    "typed_box.value",
    "typed_box.",
  );
  const importedBoxFieldHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        document.uri,
        importedBoxFieldPosition,
      ),
    (result) => hoverText(result).includes("number"),
    "imported generic class field hover",
  );
  assert.match(hoverText(importedBoxFieldHover), /value: number/);

  const importedBoxCompletionPosition = positionAfter(
    document,
    "typed_box:get()",
    "typed_box:",
  );
  const importedBoxCompletion = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeCompletionItemProvider",
        document.uri,
        importedBoxCompletionPosition,
        ":",
      ),
    (result) =>
      (result?.items || []).some(
        (item) =>
          (typeof item.label === "string" ? item.label : item.label.label) === "get",
      ),
    "imported generic class member completion",
  );
  const importedBoxGetCompletion = importedBoxCompletion.items.find(
    (item) => (typeof item.label === "string" ? item.label : item.label.label) === "get",
  );
  assert.ok(importedBoxGetCompletion, "the instantiated get member should be offered");

  const importedBoxSignaturePosition = positionAfter(
    document,
    "typed_box:get()",
    "typed_box:get(",
  );
  const importedBoxSignature = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeSignatureHelpProvider",
        document.uri,
        importedBoxSignaturePosition,
        "(",
      ),
    (result) => result?.signatures?.length > 0,
    "imported generic class method signature help",
  );
  assert.match(importedBoxSignature.signatures[0].label, /get\(\): number/);

  const importedBoxMethodDefinitionPosition = positionAfter(
    document,
    "typed_box:get()",
    "typed_box:",
  );
  const importedBoxMethodDefinition = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeDefinitionProvider",
        document.uri,
        importedBoxMethodDefinitionPosition,
      ),
    (result) =>
      Array.isArray(result) &&
      result.some((location) => locationPath(location) === fixtures.genericClassPath),
    "imported generic class method definition",
  );
  assert(
    importedBoxMethodDefinition.some(
      (location) => locationPath(location) === fixtures.genericClassPath,
    ),
    "the imported generic class method should navigate to its declaration",
  );

  const importedBoxConstructorSignaturePosition = positionAfter(
    document,
    "BoxModule.Box.new(42)",
    "BoxModule.Box.new(42",
  );
  const importedBoxConstructorSignature = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeSignatureHelpProvider",
        document.uri,
        importedBoxConstructorSignaturePosition,
        "(",
      ),
    (result) => result?.signatures?.length > 0,
    "imported generic class constructor signature help",
  );
  assert.match(importedBoxConstructorSignature.signatures[0].label, /value: number/);
  assert.match(importedBoxConstructorSignature.signatures[0].label, /Box<number>/);

  const specializedParentHoverPosition = positionAfter(
    document,
    "local specialized_box = BoxModule.NumberBox.new(42)",
    "local specialized_box",
  );
  const specializedParentHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        document.uri,
        specializedParentHoverPosition,
      ),
    (result) => /NumberBox/.test(hoverText(result)),
    "concrete generic-parent subclass hover",
  );
  assert.match(hoverText(specializedParentHover), /NumberBox/);

  const specializedParentFieldHoverPosition = positionAfter(
    document,
    "typed_specialized_box.value",
    "typed_specialized_box.",
  );
  const specializedParentFieldHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        document.uri,
        specializedParentFieldHoverPosition,
      ),
    (result) => /number/.test(hoverText(result)),
    "concrete generic-parent inherited field hover",
  );
  assert.match(hoverText(specializedParentFieldHover), /value: number/);

  const specializedParentCompletionPosition = positionAfter(
    document,
    "typed_specialized_box:get()",
    "typed_specialized_box:",
  );
  const specializedParentCompletion = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeCompletionItemProvider",
        document.uri,
        specializedParentCompletionPosition,
        ":",
      ),
    (result) =>
      (result?.items || []).some(
        (item) =>
          (typeof item.label === "string" ? item.label : item.label.label) === "get",
      ),
    "concrete generic-parent inherited method completion",
  );
  const specializedParentGetCompletion = specializedParentCompletion.items.find(
    (item) => (typeof item.label === "string" ? item.label : item.label.label) === "get",
  );
  assert.ok(specializedParentGetCompletion, "the specialized inherited method should complete");

  const specializedParentSignaturePosition = positionAfter(
    document,
    "typed_specialized_box:get()",
    "typed_specialized_box:get(",
  );
  const specializedParentSignature = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeSignatureHelpProvider",
        document.uri,
        specializedParentSignaturePosition,
        "(",
      ),
    (result) => result?.signatures?.length > 0,
    "concrete generic-parent inherited method signature",
  );
  assert.match(specializedParentSignature.signatures[0].label, /get\(\): number/);

  const genericTypePosition = positionAfter(document, "Box<number>", "Box");
  const genericDefinitions = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeDefinitionProvider",
        document.uri,
        genericTypePosition,
      ),
    (result) =>
      Array.isArray(result) &&
      result.some((location) => location.uri.fsPath === fixtures.contractsPath),
    "imported generic alias definition",
  );
  assert(genericDefinitions.some((location) => location.uri.fsPath === fixtures.contractsPath));

  const genericDocument = await vscode.workspace.openTextDocument(fixtures.genericPath);
  const genericBinderPosition = positionAfter(
    genericDocument,
    "identity<T>(value: T): T",
    "identity<",
  );
  const genericParameterTypePosition = positionAfter(
    genericDocument,
    "identity<T>(value: T): T",
    "identity<T>(value: ",
  );
  const genericBinderDefinition = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeDefinitionProvider",
        genericDocument.uri,
        genericParameterTypePosition,
      ),
    (result) => Array.isArray(result) && result.length > 0,
    "generic function binder definition",
  );
  assert.equal(genericBinderDefinition[0].uri.fsPath, fixtures.genericPath);
  assert.equal(
    genericBinderDefinition[0].range.start.line,
    genericDocument.positionAt(
      genericDocument.getText().indexOf("identity<T>") + "identity<".length,
    ).line,
  );

  const genericBinderReferences = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeReferenceProvider",
        genericDocument.uri,
        genericBinderPosition,
      ),
    (result) => Array.isArray(result) && result.length >= 3,
    "generic function binder references",
  );
  assert(
    genericBinderReferences.length >= 3,
    "the binder, parameter, and return type should be references",
  );

  const genericBinderRename = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeDocumentRenameProvider",
        genericDocument.uri,
        genericParameterTypePosition,
        "Item",
      ),
    (result) =>
      result &&
      typeof result.entries === "function" &&
      result.entries().some(([uri]) => uri.fsPath === fixtures.genericPath),
    "generic function binder rename",
  );
  const genericBinderRenameEntry = genericBinderRename
    .entries()
    .find(([uri]) => uri.fsPath === fixtures.genericPath);
  assert.equal(
    genericBinderRenameEntry[1].length,
    3,
    "renaming T should update its binder, parameter type, and return type",
  );

  const protocolPosition = positionAfter(document, "value:update(dt)", "value:");
  const protocolHover = await retry(
    () => vscode.commands.executeCommand("vscode.executeHoverProvider", document.uri, protocolPosition),
    (result) => hoverText(result).includes("advances the actor by one frame"),
    "imported protocol method hover documentation",
  );
  assert.match(hoverText(protocolHover), /update.*dt: number/s);
  assert.match(hoverText(protocolHover), /advances the actor by one frame/);

  const cyclicProtocolDocument = await vscode.workspace.openTextDocument(
    fixtures.cyclicProtocolUsePath,
  );
  await vscode.window.showTextDocument(cyclicProtocolDocument, { preview: false });
  const cyclicProtocolParameterPosition = positionAfter(
    cyclicProtocolDocument,
    "function consume(value: B, paired: A)",
    "function consume(",
  );
  const cyclicProtocolParameterHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        cyclicProtocolDocument.uri,
        cyclicProtocolParameterPosition,
      ),
    (result) => hoverText(result).includes("parameter value: B"),
    "cyclic imported protocol parameter hover",
  );
  assert.match(hoverText(cyclicProtocolParameterHover), /parameter value: B/);

  const cyclicProtocolReturnHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        cyclicProtocolDocument.uri,
        positionAfter(cyclicProtocolDocument, "local target: B = a:next()", "local "),
      ),
    (result) => hoverText(result).includes("local target: B"),
    "recursive protocol method return hover",
  );
  assert.match(hoverText(cyclicProtocolReturnHover), /local target: B/);

  const cyclicProtocolReturnCompletionPosition = positionAfter(
    cyclicProtocolDocument,
    "target:previous()",
    "target:",
  );
  const cyclicProtocolReturnCompletion = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeCompletionItemProvider",
        cyclicProtocolDocument.uri,
        cyclicProtocolReturnCompletionPosition,
        ":",
      ),
    (result) => completionLabels(result).includes("previous"),
    "member completion on a recursively returned protocol",
  );
  assert(completionLabels(cyclicProtocolReturnCompletion).includes("previous"));

  const cyclicProtocolCompletionPosition = positionAfter(
    cyclicProtocolDocument,
    "value:g(paired)",
    "value:",
  );
  const cyclicProtocolCompletion = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeCompletionItemProvider",
        cyclicProtocolDocument.uri,
        cyclicProtocolCompletionPosition,
        ":",
      ),
    (result) => completionLabels(result).includes("g"),
    "method completion through a recursive imported protocol",
  );
  assert(completionLabels(cyclicProtocolCompletion).includes("g"));

  const cyclicProtocolMethodPosition = positionAfter(
    cyclicProtocolDocument,
    "value:g(paired)",
    "value:",
  );
  const cyclicProtocolMethodHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        cyclicProtocolDocument.uri,
        cyclicProtocolMethodPosition,
      ),
    (result) => (result || []).length > 0,
    "recursive imported protocol method hover",
  );
  assert.match(hoverText(cyclicProtocolMethodHover), /g: \(value: A\) -> nil/);
  assert.deepEqual(
    vscode.languages
      .getDiagnostics(cyclicProtocolDocument.uri)
      .filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error)
      .map((diagnostic) => `${diagnostic.code}: ${diagnostic.message}`),
    [],
    "mutually recursive protocol signatures should not produce false errors in VS Code",
  );

  const protocolDefinitions = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeDefinitionProvider",
        document.uri,
        protocolPosition,
      ),
    (result) => Array.isArray(result) && result.some((location) => location.uri.fsPath === fixtures.contractsPath),
    "imported protocol method definition",
  );
  assert(protocolDefinitions.some((location) => location.uri.fsPath === fixtures.contractsPath));

  const contractsDocument = await vscode.workspace.openTextDocument(fixtures.contractsPath);
  const originalContractsSource = contractsDocument.getText();
  const nullableContractsSource = originalContractsSource.replace(
    "value: T }",
    "value: T | nil }",
  );
  assert.notEqual(nullableContractsSource, originalContractsSource);
  const contractEdit = new vscode.WorkspaceEdit();
  contractEdit.replace(
    contractsDocument.uri,
    new vscode.Range(
      contractsDocument.positionAt(0),
      contractsDocument.positionAt(originalContractsSource.length),
    ),
    nullableContractsSource,
  );
  assert(await vscode.workspace.applyEdit(contractEdit));
  assert(contractsDocument.isDirty, "the contract edit should remain unsaved");

  const scoreFieldPosition = positionAfter(document, "score.value", "score.");
  const nullableGenericFieldHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        document.uri,
        scoreFieldPosition,
      ),
    (result) => /number/.test(hoverText(result)) && /nil/.test(hoverText(result)),
    "unsaved imported generic alias edit propagation",
  );
  assert.match(hoverText(nullableGenericFieldHover), /number/);
  assert.match(hoverText(nullableGenericFieldHover), /nil/);

  const restoreContracts = new vscode.WorkspaceEdit();
  restoreContracts.replace(
    contractsDocument.uri,
    new vscode.Range(
      contractsDocument.positionAt(0),
      contractsDocument.positionAt(contractsDocument.getText().length),
    ),
    originalContractsSource,
  );
  assert(await vscode.workspace.applyEdit(restoreContracts));
  const restoredGenericFieldHover = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        document.uri,
        scoreFieldPosition,
      ),
    (result) => /number/.test(hoverText(result)) && !/nil/.test(hoverText(result)),
    "unsaved imported generic alias edit restoration",
  );
  assert.match(hoverText(restoredGenericFieldHover), /number/);

  const protocolRequirementPosition = positionAfter(
    contractsDocument,
    "update(dt: number)",
    "up",
  );
  const protocolImplementations = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeImplementationProvider",
        document.uri,
        protocolPosition,
      ),
    (result) =>
      Array.isArray(result) &&
      result.some((location) => locationPath(location) === fixtures.actorsPath),
    "installed-editor protocol implementation navigation",
  );
  assert(
    protocolImplementations.some((location) => locationPath(location) === fixtures.actorsPath),
    "implementation navigation should reach the imported protocol conformer",
  );

  const protocolRename = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeDocumentRenameProvider",
        contractsDocument.uri,
        protocolRequirementPosition,
        "advance",
      ),
    (result) =>
      result &&
      typeof result.entries === "function" &&
      result.entries().some(([uri]) => uri.fsPath === fixtures.actorsPath),
    "installed-editor protocol rename across files",
  );
  assert(
    protocolRename.entries().some(([uri]) => uri.fsPath === fixtures.actorsPath),
    "renaming a protocol requirement should include its implementation edit",
  );

  const playerUse = positionAfter(document, "player.name", "player");
  const hover = await retry(
    () => vscode.commands.executeCommand("vscode.executeHoverProvider", document.uri, playerUse),
    (result) => hoverText(result).includes("Player"),
    "Player hover",
  );
  assert.match(hoverText(hover), /Player/);

  const definitions = await retry(
    () =>
      vscode.commands.executeCommand("vscode.executeDefinitionProvider", document.uri, playerUse),
    (result) => Array.isArray(result) && result.length > 0,
    "local definition",
  );
  assert.equal(definitions[0].uri.fsPath, fixtures.mainPath);
  assert.equal(
    definitions[0].range.start.line,
    document.positionAt(document.getText().indexOf("local player: Player")).line,
  );

  const references = await retry(
    () =>
      vscode.commands.executeCommand("vscode.executeReferenceProvider", document.uri, playerUse),
    (result) => Array.isArray(result) && result.length >= 4,
    "local references",
  );

  const signaturePosition = positionAfter(document, "helper.greet(", "helper.greet(");
  const signatures = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeSignatureHelpProvider",
        document.uri,
        signaturePosition,
        "(",
      ),
    (result) => result?.signatures?.length > 0,
    "LuaDoc signature help",
  );
  assert.match(signatures.signatures[0].label, /greet\(name: string\).*string/);

  const symbols = await retry(
    () =>
      vscode.commands.executeCommand("vscode.executeDocumentSymbolProvider", document.uri),
    (result) => Array.isArray(result) && result.length > 0,
    "document symbols",
  );
  assert(symbolNames(symbols).includes("Player"));

  const hints = await retry(
    () =>
      vscode.commands.executeCommand(
        "vscode.executeInlayHintProvider",
        document.uri,
        new vscode.Range(0, 0, document.lineCount, 0),
      ),
    (result) => Array.isArray(result) && result.length > 0,
    "inlay hints",
  );

  const formatting = await vscode.commands.executeCommand(
    "vscode.executeFormatDocumentProvider",
    document.uri,
    { insertSpaces: true, tabSize: 2 },
  );
  assert(
    Array.isArray(formatting) && formatting.length > 0,
    "the real server should provide document formatting edits",
  );

  await delay(300);
  const errors = vscode.languages
    .getDiagnostics(document.uri)
    .filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error);
  assert.deepEqual(
    errors.map((diagnostic) => `${diagnostic.code}: ${diagnostic.message}`),
    [],
    "the representative project should have no error diagnostics",
  );

  await vscode.commands.executeCommand("tua.copyCacheStats");
  const cacheStats = JSON.parse(await vscode.env.clipboard.readText());
  assert(cacheStats.parse, "the real server should answer custom cache-stat requests");

  await verifyGraphPreviews(fixtures.graphPath);

  const fontUri = vscode.Uri.file(fixtures.fontPath);
  assert.equal(
    await vscode.commands.executeCommand("tua.assets.preview", { uri: fontUri.toString() }),
    fontUri.toString(),
  );
  const { browser: fontBrowser, context: fontContext } = await previewContext(
    process.env.TUA_REAL_SERVER_CDP_PORT,
    "sniglet.otf font preview",
  );
  try {
    const rendered = await retry(
      () => fontBrowser.evaluate(`({
        status: document.getElementById('status')?.textContent,
        size: getComputedStyle(document.querySelector('.size-18')).fontSize,
        glyphs: document.querySelector('.glyphs')?.textContent
      })`, fontContext),
      (result) => result.status === "Font loaded",
      "valid font to render in the editor preview",
    );
    assert.equal(rendered.size, "18px");
    assert.match(rendered.glyphs, /0123456789/);
  } finally {
    fontBrowser.dispose();
  }
  await vscode.commands.executeCommand("workbench.action.closeActiveEditor");

  console.log(
    `REAL_SERVER_EVIDENCE ${JSON.stringify({
      cacheStats: true,
      completions: completionEvidence,
      genericBoundCompletion,
      implementsCompletion,
      definitions: definitions.length,
      diagnostics: errors.length,
      protocolDefinition: protocolDefinitions.some((location) => location.uri.fsPath === fixtures.contractsPath),
      protocolImplementations: protocolImplementations.length,
      protocolRenameFiles: protocolRename.entries().map(([uri]) => path.basename(uri.fsPath)),
      protocolHover: "documented update(dt: number)",
      cyclicProtocolHover: hoverText(cyclicProtocolParameterHover),
      cyclicProtocolMethodCompletion: completionLabels(cyclicProtocolCompletion).includes("g"),
      cyclicProtocolMethodHover: hoverText(cyclicProtocolMethodHover),
      cyclicProtocolReturnHover: hoverText(cyclicProtocolReturnHover),
      cyclicProtocolReturnCompletion: completionLabels(cyclicProtocolReturnCompletion).includes("previous"),
      protocolStubActionApplied: true,
      formattingEdits: formatting.length,
      fontPreview: "sniglet.otf rendered at 18px",
      hover: "Player",
      genericAliasHover: "Box<number>.value: number",
      importedGenericFunctionHover: hoverText(importedGenericHover),
      importedGenericFunctionDefinition: genericFunctionLocation.uri.fsPath,
      importedGenericFunctionSignature: importedGenericSignature.signatures[0].label,
      importedGenericResultHover: hoverText(importedGenericResultHover),
      importedGenericClassHover: hoverText(importedBoxHover),
      importedGenericClassAliasHover: hoverText(importedBoxAliasHover),
      importedGenericClassFieldHover: hoverText(importedBoxFieldHover),
      importedGenericClassCompletion: importedBoxGetCompletion.detail,
      importedGenericClassMethodSignature: importedBoxSignature.signatures[0].label,
      importedGenericClassMethodDefinition: importedBoxMethodDefinition.some(
        (location) => locationPath(location) === fixtures.genericClassPath,
      ),
      importedGenericClassConstructorSignature:
        importedBoxConstructorSignature.signatures[0].label,
      concreteGenericParentHover: hoverText(specializedParentHover),
      concreteGenericParentFieldHover: hoverText(specializedParentFieldHover),
      concreteGenericParentCompletion: specializedParentGetCompletion.detail,
      concreteGenericParentSignature: specializedParentSignature.signatures[0].label,
      genericAliasDefinition: genericDefinitions.some(
        (location) => location.uri.fsPath === fixtures.contractsPath,
      ),
      genericBinderDefinition: true,
      genericBinderReferences: genericBinderReferences.length,
      genericBinderRenameEdits: genericBinderRenameEntry[1].length,
      unsavedGenericFieldTypeUpdated: /nil/.test(hoverText(nullableGenericFieldHover)),
      unsavedGenericFieldTypeRestored: !/nil/.test(hoverText(restoredGenericFieldHover)),
      inlayHints: hints.length,
      references: references.length,
      server: path.basename(process.env.TUA_REAL_SERVER_PATH),
      signature: signatures.signatures[0].label,
      symbols: symbolNames(symbols),
    })}`,
  );
}

async function waitForServer() {
  await retry(
    async () => {
      await vscode.commands.executeCommand("tua.copyDebugInfo");
      return vscode.env.clipboard.readText();
    },
    (debugInfo) =>
      debugInfo.includes("Server state: running") &&
      debugInfo.includes(`Server path: ${process.env.TUA_REAL_SERVER_PATH}`) &&
      !debugInfo.includes("Last server error:"),
    "the staged real server to reach the running state",
    15000,
  );
}

async function expectCompletion(document, needle, prefix, expectedLabel) {
  const position = positionAfter(document, needle, prefix);
  const observedLabels = new Set();
  let result;

  try {
    result = await retry(
      async () => {
        const completionList = await vscode.commands.executeCommand(
          "vscode.executeCompletionItemProvider",
          document.uri,
          position,
          prefix.endsWith(".") ? "." : undefined,
        );
        for (const label of completionLabels(completionList)) {
          observedLabels.add(label);
        }
        return completionList;
      },
      (completionList) => completionLabels(completionList).includes(expectedLabel),
      `${expectedLabel} completion`,
      15000,
    );
  } catch (error) {
    throw new Error(
      `${error.message} Observed completion labels: ${JSON.stringify([...observedLabels].slice(0, 80))}`,
    );
  }

  assert(completionLabels(result).includes(expectedLabel));
  return expectedLabel;
}

function positionAfter(document, needle, prefix) {
  const source = document.getText();
  const needleOffset = source.indexOf(needle);
  assert(needleOffset >= 0, `fixture should contain ${JSON.stringify(needle)}`);
  return document.positionAt(needleOffset + prefix.length);
}

function completionLabels(result) {
  return (result?.items || []).map((item) =>
    typeof item.label === "string" ? item.label : item.label.label,
  );
}

function hoverText(hovers) {
  return (hovers || [])
    .flatMap((hover) => hover.contents || [])
    .map((content) => (typeof content === "string" ? content : content.value || ""))
    .join("\n");
}

function locationPath(location) {
  return (location.uri || location.targetUri)?.fsPath;
}

function symbolNames(symbols) {
  return symbols.flatMap((symbol) => [symbol.name, ...symbolNames(symbol.children || [])]);
}

async function retry(operation, predicate, description, timeoutMs = 10000) {
  const deadline = Date.now() + timeoutMs;
  let result;
  let lastError;

  while (Date.now() < deadline) {
    try {
      result = await operation();
      if (predicate(result)) {
        return result;
      }
    } catch (error) {
      lastError = error;
    }
    await delay(50);
  }

  throw new Error(
    `Timed out waiting for ${description}.${lastError ? ` Last error: ${lastError.message}` : ""}`,
  );
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

module.exports = {
  run,
};
