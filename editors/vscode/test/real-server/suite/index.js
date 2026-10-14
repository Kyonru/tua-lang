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
  };

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
  assert.equal(definitions[0].range.start.line, 5);

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
      formattingEdits: formatting.length,
      hover: "Player",
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
