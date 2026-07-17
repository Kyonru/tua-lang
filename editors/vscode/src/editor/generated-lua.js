"use strict";

const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { formatError } = require("../shared/errors");

const CONFIG_FILE_NAME = "tua.toml";
const DEFAULT_SOURCE_DIRECTORY = "src";
const DEFAULT_OUTPUT_DIRECTORY = "build";

function registerGeneratedLuaCommand({ isTuaDocument }) {
  return vscode.commands.registerCommand(COMMANDS.openGeneratedLua, () =>
    openGeneratedLuaBesideActiveDocument({ isTuaDocument }),
  );
}

async function openGeneratedLuaBesideActiveDocument({ isTuaDocument }) {
  const document = vscode.window.activeTextEditor?.document;
  if (!document || !isTuaDocument(document)) {
    vscode.window.showWarningMessage("Open a Tua source file to open generated Lua.");
    return undefined;
  }
  if (document.uri.scheme !== "file") {
    vscode.window.showWarningMessage("Generated Lua can only be opened for saved Tua files.");
    return undefined;
  }

  try {
    const generatedPath = resolveGeneratedLuaPath(document.uri);
    if (!generatedPath) {
      return undefined;
    }
    if (!fs.existsSync(generatedPath)) {
      vscode.window.showWarningMessage(
        `Generated Lua was not found at ${generatedPath}. Run tua build first.`,
      );
      return undefined;
    }

    const luaDocument = await vscode.workspace.openTextDocument(vscode.Uri.file(generatedPath));
    await vscode.window.showTextDocument(luaDocument, {
      preview: true,
      viewColumn: vscode.ViewColumn.Beside,
    });

    return generatedPath;
  } catch (error) {
    vscode.window.showErrorMessage(`Could not open generated Lua: ${formatError(error)}`);
    return undefined;
  }
}

function resolveGeneratedLuaPath(uri) {
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(uri);
  if (!workspaceFolder) {
    vscode.window.showWarningMessage("Tua source file is not inside an open workspace folder.");
    return undefined;
  }

  const sourcePath = path.resolve(uri.fsPath);
  const workspaceRoot = path.resolve(workspaceFolder.uri.fsPath);
  const configPath = findNearestConfig(path.dirname(sourcePath), workspaceRoot);
  const configDirectory = configPath ? path.dirname(configPath) : workspaceRoot;
  const config = configPath ? readProjectConfig(configPath) : {};
  const sourceDirectory = path.resolve(
    configDirectory,
    config.source || DEFAULT_SOURCE_DIRECTORY,
  );

  if (!isPathInside(sourceDirectory, sourcePath)) {
    vscode.window.showWarningMessage(
      `Tua source file is outside configured source directory ${sourceDirectory}.`,
    );
    return undefined;
  }

  const outputDirectory = path.resolve(
    configDirectory,
    config.out || DEFAULT_OUTPUT_DIRECTORY,
  );
  const relativeSource = path.relative(sourceDirectory, sourcePath);
  const relativeLua = replaceExtension(relativeSource, ".lua");

  return path.join(outputDirectory, relativeLua);
}

function findNearestConfig(startDirectory, workspaceRoot) {
  let current = path.resolve(startDirectory);
  const root = path.resolve(workspaceRoot);

  while (isPathInside(root, current)) {
    const candidate = path.join(current, CONFIG_FILE_NAME);
    if (fs.existsSync(candidate)) {
      return candidate;
    }

    const parent = path.dirname(current);
    if (parent === current) {
      break;
    }
    current = parent;
  }

  return undefined;
}

function readProjectConfig(configPath) {
  const text = fs.readFileSync(configPath, "utf8");

  return {
    out: topLevelStringSetting(text, "out"),
    source: topLevelStringSetting(text, "source"),
  };
}

function topLevelStringSetting(text, key) {
  let inTopLevel = true;
  const name = escapeRegExp(key);
  const basicStringPattern = new RegExp(
    `^${name}\\s*=\\s*"([^"\\\\]*(?:\\\\.[^"\\\\]*)*)"\\s*(?:#.*)?$`,
  );
  const literalStringPattern = new RegExp(`^${name}\\s*=\\s*'([^']*)'\\s*(?:#.*)?$`);

  for (const rawLine of text.split(/\r?\n/u)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) {
      continue;
    }
    if (line.startsWith("[")) {
      inTopLevel = false;
      continue;
    }
    if (!inTopLevel) {
      continue;
    }

    const basicStringMatch = line.match(basicStringPattern);
    if (basicStringMatch) {
      return unescapeBasicTomlString(basicStringMatch[1]);
    }

    const literalStringMatch = line.match(literalStringPattern);
    if (literalStringMatch) {
      return literalStringMatch[1];
    }
  }

  return undefined;
}

function unescapeBasicTomlString(value) {
  return value.replace(/\\([btnfr"\\])/gu, (_, escape) => {
    switch (escape) {
      case "b":
        return "\b";
      case "t":
        return "\t";
      case "n":
        return "\n";
      case "f":
        return "\f";
      case "r":
        return "\r";
      default:
        return escape;
    }
  });
}

function replaceExtension(filePath, extension) {
  const parsed = path.parse(filePath);

  return path.join(parsed.dir, `${parsed.name}${extension}`);
}

function isPathInside(parentPath, childPath) {
  const parent = normalizePath(parentPath);
  const child = normalizePath(childPath);
  const relative = path.relative(parent, child);

  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

function normalizePath(filePath) {
  const resolved = path.resolve(filePath);

  return process.platform === "win32" ? resolved.toLowerCase() : resolved;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

module.exports = {
  registerGeneratedLuaCommand,
};
