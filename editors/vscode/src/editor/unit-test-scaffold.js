"use strict";

const path = require("path");
const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { projectLayoutForUri } = require("./project-layout");

function registerUnitTestScaffold({ getClient, isTuaDocument }) {
  return vscode.commands.registerCommand(COMMANDS.createUnitTestForFunction, async () => {
    const editor = vscode.window.activeTextEditor;
    const document = editor?.document;
    if (!document || !isTuaDocument(document) || document.uri.scheme !== "file") {
      return vscode.window.showWarningMessage("Open a saved Tua source file and place the cursor on a function.");
    }

    const symbols = await vscode.commands.executeCommand(
      "vscode.executeDocumentSymbolProvider",
      document.uri,
    );
    const target = functionAtPosition(symbols || [], editor.selection.active);
    if (!target) {
      return vscode.window.showInformationMessage("Place the cursor inside a function to create its unit test.");
    }

    try {
      const layout = await projectLayoutForUri(document.uri, getClient);
      if (!layout?.moduleName) {
        return vscode.window.showWarningMessage("This function is outside the configured Tua source directory.");
      }

      const relativeSource = path.relative(layout.sourceDirectory, document.uri.fsPath);
      if (!isRelativePathInside(relativeSource)) {
        return vscode.window.showWarningMessage("This function is outside the configured Tua source directory.");
      }

      const parsed = path.parse(relativeSource);
      const extension = parsed.ext || ".tua";
      const testPath = path.join(
        layout.testSourceDirectory,
        parsed.dir,
        `${parsed.name}_spec${extension}`,
      );
      const testUri = vscode.Uri.file(testPath);
      if (await exists(testUri)) {
        const choice = await vscode.window.showWarningMessage(
          `A unit test already exists at ${path.relative(layout.configDirectory, testPath)}.`,
          "Open Existing Test",
        );
        if (choice === "Open Existing Test") {
          await openDocument(testUri);
        }
        return;
      }

      await vscode.workspace.fs.createDirectory(vscode.Uri.file(path.dirname(testPath)));
      await vscode.workspace.fs.writeFile(
        testUri,
        Buffer.from(scaffoldSource(layout.moduleName, target.name), "utf8"),
      );
      await openDocument(testUri);
      vscode.window.setStatusBarMessage(`Created unit test for ${target.name}`, 2500);
    } catch (error) {
      vscode.window.showErrorMessage(`Could not create unit test: ${error.message || String(error)}`);
    }
  });
}

function functionAtPosition(symbols, position) {
  return flattenSymbols(symbols)
    .filter((symbol) =>
      (symbol.kind === vscode.SymbolKind.Function || symbol.kind === vscode.SymbolKind.Method) &&
      rangeContainsPosition(symbol.range, position),
    )
    .sort((left, right) => rangeSize(left.range) - rangeSize(right.range))
    .at(0);
}

function flattenSymbols(symbols) {
  return symbols.flatMap((symbol) => [
    { name: symbol.name, kind: symbol.kind, range: symbol.range },
    ...flattenSymbols(symbol.children || []),
  ]);
}

function rangeContainsPosition(range, position) {
  return range && position.compareTo(range.start) >= 0 && position.compareTo(range.end) <= 0;
}

function rangeSize(range) {
  return (range.end.line - range.start.line) * 100000 + range.end.character - range.start.character;
}

function isRelativePathInside(relativePath) {
  return relativePath !== ".." && !relativePath.startsWith(`..${path.sep}`) && !path.isAbsolute(relativePath);
}

async function exists(uri) {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}

function scaffoldSource(moduleName, functionName) {
  const qualifiedName = `${moduleName}.${functionName}`;
  return [
    `describe(${luaString(qualifiedName)}, function()`,
    `  it(${luaString(`handles ${functionName}`)}, function()`,
    "    -- Arrange dependencies (including LOVE test doubles) before loading the module.",
    `    -- local subject = require(${luaString(moduleName)})`,
    `    pending(${luaString(`Add inputs and assertions for ${functionName}.`)})`,
    "  end)",
    "end)",
    "",
  ].join("\n");
}

function luaString(value) {
  return `"${String(value).replaceAll("\\", "\\\\").replaceAll('"', '\\"').replaceAll("\n", "\\n").replaceAll("\r", "\\r")}"`;
}

async function openDocument(uri) {
  const document = await vscode.workspace.openTextDocument(uri);
  await vscode.window.showTextDocument(document, { preview: false });
}

module.exports = {
  functionAtPosition,
  registerUnitTestScaffold,
  scaffoldSource,
};
