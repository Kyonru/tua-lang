"use strict";

const fs = require("fs");
const path = require("path");
const vscode = require("vscode");

const CONFIG_FILE_NAME = "tua.toml";
const DEFAULT_SOURCE_DIRECTORY = "src";
const DEFAULT_OUTPUT_DIRECTORY = "build";

function generatedLuaPathForUri(uri) {
  const layout = projectLayoutForUri(uri);
  if (!layout) {
    return { kind: "missingWorkspace" };
  }

  const sourcePath = path.resolve(uri.fsPath);
  if (!isPathInside(layout.sourceDirectory, sourcePath)) {
    return { kind: "outsideSource", sourceDirectory: layout.sourceDirectory };
  }

  const relativeSource = path.relative(layout.sourceDirectory, sourcePath);
  const relativeLua = replaceExtension(relativeSource, ".lua");

  return {
    kind: "ok",
    path: path.join(layout.outputDirectory, relativeLua),
    layout,
  };
}

function moduleNameForUri(uri) {
  const layout = projectLayoutForUri(uri);
  if (!layout) {
    return { kind: "missingWorkspace" };
  }

  const sourcePath = path.resolve(uri.fsPath);
  if (!isPathInside(layout.sourceDirectory, sourcePath)) {
    return { kind: "outsideSource", sourceDirectory: layout.sourceDirectory };
  }

  const relativeSource = path.relative(layout.sourceDirectory, sourcePath);
  const withoutExtension = relativeSource.slice(
    0,
    relativeSource.length - path.extname(relativeSource).length,
  );
  const moduleName = withoutExtension
    .split(path.sep)
    .filter(Boolean)
    .join(".");

  return {
    kind: "ok",
    moduleName,
    layout,
  };
}

function projectLayoutForUri(uri) {
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(uri);
  if (!workspaceFolder) {
    return undefined;
  }

  const sourcePath = path.resolve(uri.fsPath);
  const workspaceRoot = path.resolve(workspaceFolder.uri.fsPath);
  const configPath = findNearestConfig(path.dirname(sourcePath), workspaceRoot);

  return projectLayoutForWorkspaceRoot(workspaceRoot, configPath);
}

function projectLayoutForWorkspaceRoot(workspaceRoot, configPath = undefined) {
  const root = path.resolve(workspaceRoot);
  const resolvedConfigPath = configPath || defaultWorkspaceConfigPath(root);
  const configDirectory = resolvedConfigPath ? path.dirname(resolvedConfigPath) : root;
  const config = resolvedConfigPath ? readProjectConfig(resolvedConfigPath) : {};

  return {
    configDirectory,
    configPath: resolvedConfigPath,
    outputDirectory: path.resolve(configDirectory, config.out || DEFAULT_OUTPUT_DIRECTORY),
    sourceDirectory: path.resolve(configDirectory, config.source || DEFAULT_SOURCE_DIRECTORY),
    workspaceRoot: root,
  };
}

function defaultWorkspaceConfigPath(workspaceRoot) {
  const candidate = path.join(workspaceRoot, CONFIG_FILE_NAME);

  return fs.existsSync(candidate) ? candidate : undefined;
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

function normalizeRelativePath(filePath) {
  const normalized = path.normalize(filePath).replace(/[\\/]+/gu, path.sep);

  return process.platform === "win32" ? normalized.toLowerCase() : normalized;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

module.exports = {
  generatedLuaPathForUri,
  isPathInside,
  moduleNameForUri,
  normalizePath,
  normalizeRelativePath,
  projectLayoutForUri,
  projectLayoutForWorkspaceRoot,
  replaceExtension,
};
