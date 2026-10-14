"use strict";

const path = require("path");
const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { formatError } = require("../shared/errors");
const { moduleNameForUri, normalizePath } = require("./project-layout");

const SYMBOL_KIND = {
  Class: vscode.SymbolKind?.Class ?? 4,
  Constant: vscode.SymbolKind?.Constant ?? 13,
  Field: vscode.SymbolKind?.Field ?? 7,
  Function: vscode.SymbolKind?.Function ?? 11,
  Interface: vscode.SymbolKind?.Interface ?? 10,
  Method: vscode.SymbolKind?.Method ?? 5,
  Module: vscode.SymbolKind?.Module ?? 1,
  Namespace: vscode.SymbolKind?.Namespace ?? 2,
  Property: vscode.SymbolKind?.Property ?? 6,
  Struct: vscode.SymbolKind?.Struct ?? 22,
  TypeParameter: vscode.SymbolKind?.TypeParameter ?? 25,
  Variable: vscode.SymbolKind?.Variable ?? 12,
};

const SYMBOL_ICONS = new Map([
  [SYMBOL_KIND.Module, "package"],
  [SYMBOL_KIND.Namespace, "symbol-namespace"],
  [SYMBOL_KIND.Class, "symbol-class"],
  [SYMBOL_KIND.Interface, "symbol-interface"],
  [SYMBOL_KIND.Struct, "symbol-struct"],
  [SYMBOL_KIND.TypeParameter, "symbol-type-parameter"],
  [SYMBOL_KIND.Function, "symbol-function"],
  [SYMBOL_KIND.Method, "symbol-method"],
  [SYMBOL_KIND.Field, "symbol-field"],
  [SYMBOL_KIND.Property, "symbol-property"],
  [SYMBOL_KIND.Variable, "symbol-variable"],
  [SYMBOL_KIND.Constant, "symbol-constant"],
]);

function registerSymbolCommands({ getClient, isTuaDocument }) {
  return vscode.Disposable.from(
    vscode.commands.registerCommand(COMMANDS.copyQualifiedName, () =>
      copyQualifiedName({ getClient, isTuaDocument }),
    ),
    vscode.commands.registerCommand(COMMANDS.showWorkspaceSymbolsByModule, (options) =>
      showWorkspaceSymbolsByModule(options),
    ),
  );
}

async function copyQualifiedName({ getClient, isTuaDocument }) {
  const editor = vscode.window.activeTextEditor;
  const document = editor?.document;
  if (!document || !isTuaDocument(document) || document.uri.scheme !== "file") {
    vscode.window.showWarningMessage("Open a saved Tua source file to copy a qualified name.");
    return undefined;
  }

  try {
    const moduleResolution = await moduleNameForUri(document.uri, getClient);
    if (moduleResolution.kind === "missingWorkspace") {
      vscode.window.showWarningMessage("Tua source file is not inside an open workspace folder.");
      return undefined;
    }
    if (moduleResolution.kind === "outsideSource") {
      vscode.window.showWarningMessage(
        `Tua source file is outside configured source directory ${moduleResolution.sourceDirectory}.`,
      );
      return undefined;
    }

    const symbols = await vscode.commands.executeCommand(
      "vscode.executeDocumentSymbolProvider",
      document.uri,
    );
    const selected = symbolAtPosition(symbols || [], editor.selection.active);
    const qualified = selected
      ? `${moduleResolution.moduleName}.${selected.path.join(".")}`
      : moduleResolution.moduleName;

    await vscode.env.clipboard.writeText(qualified);
    vscode.window.setStatusBarMessage(`Copied ${qualified}`, 1800);
    return qualified;
  } catch (error) {
    vscode.window.showErrorMessage(`Could not copy Tua symbol name: ${formatError(error)}`);
    return undefined;
  }
}

async function showWorkspaceSymbolsByModule(options = {}) {
  try {
    if (options.pickFirst) {
      const items = await workspaceSymbolItems(options.query || "");
      const first = items.find((item) => !isSeparator(item));
      if (!first) {
        return undefined;
      }
      await openWorkspaceSymbolItem(first);
      return first.qualifiedName;
    }

    return showWorkspaceSymbolQuickPick();
  } catch (error) {
    vscode.window.showErrorMessage(`Could not show Tua workspace symbols: ${formatError(error)}`);
    return undefined;
  }
}

async function showWorkspaceSymbolQuickPick() {
  const quickPick = vscode.window.createQuickPick();
  let accepted = false;
  let request = 0;

  quickPick.title = "Tua Workspace Symbols by Module";
  quickPick.placeholder = "Type a module or symbol name";
  quickPick.matchOnDescription = true;
  quickPick.matchOnDetail = true;

  const refresh = async (query) => {
    const currentRequest = ++request;
    quickPick.busy = true;
    try {
      const items = await workspaceSymbolItems(query);
      if (currentRequest === request) {
        quickPick.items = items.length > 0
          ? items
          : [{ label: "No Tua workspace symbols found", alwaysShow: true }];
      }
    } catch (error) {
      if (currentRequest === request) {
        quickPick.items = [{
          label: "Could not load Tua workspace symbols",
          detail: formatError(error),
          alwaysShow: true,
        }];
      }
    } finally {
      if (currentRequest === request) {
        quickPick.busy = false;
      }
    }
  };

  return new Promise((resolve) => {
    const subscriptions = [
      quickPick.onDidChangeValue((value) => {
        void refresh(value);
      }),
      quickPick.onDidAccept(async () => {
        const selected = quickPick.selectedItems[0];
        if (!selected || isSeparator(selected) || !selected.symbol) {
          return;
        }
        accepted = true;
        await openWorkspaceSymbolItem(selected);
        quickPick.hide();
        resolve(selected.qualifiedName);
      }),
      quickPick.onDidHide(() => {
        subscriptions.forEach((subscription) => subscription.dispose());
        quickPick.dispose();
        if (!accepted) {
          resolve(undefined);
        }
      }),
    ];

    void refresh("");
    quickPick.show();
  });
}

async function workspaceSymbolItems(query) {
  const symbols = await vscode.commands.executeCommand(
    "vscode.executeWorkspaceSymbolProvider",
    query,
  );
  const normalized = (symbols || [])
    .map(normalizeWorkspaceSymbol)
    .filter(Boolean)
    .sort(compareWorkspaceSymbols);
  const grouped = [];
  let currentModule;

  for (const symbol of normalized) {
    if (symbol.moduleName !== currentModule) {
      currentModule = symbol.moduleName;
      grouped.push({
        kind: vscode.QuickPickItemKind.Separator,
        label: currentModule,
      });
    }

    grouped.push(symbolQuickPickItem(symbol));
  }

  return grouped;
}

function normalizeWorkspaceSymbol(symbol) {
  const location = symbol?.location;
  const uri = location?.uri;
  if (!uri || uri.scheme !== "file" || path.extname(uri.fsPath) !== ".tua") {
    return undefined;
  }

  const name = typeof symbol.name === "string" ? symbol.name : "";
  if (!name) {
    return undefined;
  }

  const containerName = typeof symbol.containerName === "string"
    ? symbol.containerName
    : "";
  const moduleSymbol = symbol.kind === SYMBOL_KIND.Module;
  const moduleName = moduleSymbol
    ? name
    : containerName;
  const qualifiedName = moduleSymbol || !moduleName
    ? name
    : `${moduleName}.${name}`;

  return {
    containerName,
    kind: symbol.kind,
    location,
    moduleName: moduleName || "(unknown module)",
    name,
    qualifiedName,
    uri,
  };
}

function compareWorkspaceSymbols(left, right) {
  return left.moduleName.localeCompare(right.moduleName)
    || kindOrder(left.kind) - kindOrder(right.kind)
    || left.name.localeCompare(right.name)
    || normalizePath(left.uri.fsPath).localeCompare(normalizePath(right.uri.fsPath));
}

function symbolQuickPickItem(symbol) {
  const icon = SYMBOL_ICONS.get(symbol.kind) || "symbol-misc";

  return {
    label: `$(${icon}) ${symbol.name}`,
    description: symbol.qualifiedName,
    detail: vscode.workspace.asRelativePath(symbol.uri, false),
    alwaysShow: true,
    qualifiedName: symbol.qualifiedName,
    symbol,
  };
}

async function openWorkspaceSymbolItem(item) {
  const document = await vscode.workspace.openTextDocument(item.symbol.uri);
  await vscode.window.showTextDocument(document, {
    preview: true,
    selection: item.symbol.location.range,
  });
}

function symbolAtPosition(symbols, position) {
  return flattenDocumentSymbols(symbols)
    .filter((symbol) => rangeContainsPosition(symbol.range, position))
    .sort((left, right) => rangeSize(left.range) - rangeSize(right.range))
    .at(0);
}

function flattenDocumentSymbols(symbols, parentPath = []) {
  return symbols.flatMap((symbol) => {
    const pathParts = [...parentPath, symbol.name];
    const entry = {
      path: pathParts,
      range: symbol.selectionRange || symbol.range,
    };

    return [
      entry,
      ...flattenDocumentSymbols(symbol.children || [], pathParts),
    ];
  });
}

function rangeContainsPosition(range, position) {
  return (
    range &&
    position.compareTo(range.start) >= 0 &&
    position.compareTo(range.end) <= 0
  );
}

function rangeSize(range) {
  return (
    (range.end.line - range.start.line) * 100000 +
    (range.end.character - range.start.character)
  );
}

function kindOrder(kind) {
  if (kind === SYMBOL_KIND.Module) {
    return 0;
  }
  if (kind === SYMBOL_KIND.TypeParameter || kind === SYMBOL_KIND.Class) {
    return 1;
  }
  if (kind === SYMBOL_KIND.Function || kind === SYMBOL_KIND.Method) {
    return 2;
  }
  return 3;
}

function isSeparator(item) {
  return item.kind === vscode.QuickPickItemKind.Separator;
}

module.exports = {
  registerSymbolCommands,
};
