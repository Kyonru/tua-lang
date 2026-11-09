"use strict";

const assert = require("assert/strict");
const path = require("path");
const vscode = require("vscode");

const fixtures = JSON.parse(process.env.TUA_REAL_SERVER_FIXTURES);

async function run() {
  const extension = vscode.extensions.getExtension("SolenodonteLabs.tua");
  assert(extension, "the Tua extension should be available in the extension host");

  await extension.activate();
  await waitForServer();

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

  const protocolPosition = positionAfter(document, "value:update(dt)", "value:");
  const protocolHover = await retry(
    () => vscode.commands.executeCommand("vscode.executeHoverProvider", document.uri, protocolPosition),
    (result) => hoverText(result).includes("advances the actor by one frame"),
    "imported protocol method hover documentation",
  );
  assert.match(hoverText(protocolHover), /update.*dt: number/s);
  assert.match(hoverText(protocolHover), /advances the actor by one frame/);

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

  console.log(
    `REAL_SERVER_EVIDENCE ${JSON.stringify({
      cacheStats: true,
      completions: completionEvidence,
      definitions: definitions.length,
      diagnostics: errors.length,
      protocolDefinition: protocolDefinitions.some((location) => location.uri.fsPath === fixtures.contractsPath),
      protocolImplementations: protocolImplementations.length,
      protocolRenameFiles: protocolRename.entries().map(([uri]) => path.basename(uri.fsPath)),
      protocolHover: "documented update(dt: number)",
      formattingEdits: formatting.length,
      hover: "Player",
      genericAliasHover: "Box<number>.value: number",
      importedGenericFunctionHover: hoverText(importedGenericHover),
      importedGenericFunctionSignature: importedGenericSignature.signatures[0].label,
      importedGenericResultHover: hoverText(importedGenericResultHover),
      importedGenericClassHover: hoverText(importedBoxHover),
      importedGenericClassAliasHover: hoverText(importedBoxAliasHover),
      importedGenericClassFieldHover: hoverText(importedBoxFieldHover),
      importedGenericClassCompletion: importedBoxGetCompletion.detail,
      importedGenericClassMethodSignature: importedBoxSignature.signatures[0].label,
      importedGenericClassConstructorSignature:
        importedBoxConstructorSignature.signatures[0].label,
      genericAliasDefinition: genericDefinitions.some(
        (location) => location.uri.fsPath === fixtures.contractsPath,
      ),
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
