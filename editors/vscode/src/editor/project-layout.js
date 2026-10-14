"use strict";

const path = require("path");
const vscode = require("vscode");

async function generatedLuaPathForUri(uri, getClient) {
  const layout = await projectLayoutForUri(uri, getClient);
  if (!layout) {
    return { kind: "missingWorkspace" };
  }
  if (!layout.generatedPath) {
    return { kind: "outsideSource", sourceDirectory: layout.sourceDirectory };
  }

  return { kind: "ok", path: layout.generatedPath, layout };
}

async function moduleNameForUri(uri, getClient) {
  const layout = await projectLayoutForUri(uri, getClient);
  if (!layout) {
    return { kind: "missingWorkspace" };
  }
  if (!layout.moduleName) {
    return { kind: "outsideSource", sourceDirectory: layout.sourceDirectory };
  }

  return { kind: "ok", moduleName: layout.moduleName, layout };
}

async function projectLayoutForUri(uri, getClient) {
  if (!vscode.workspace.getWorkspaceFolder(uri)) {
    return undefined;
  }

  return requestProjectLayout(uri, getClient);
}

async function projectLayoutForWorkspaceRoot(workspaceRoot, getClient) {
  return requestProjectLayout(vscode.Uri.file(path.join(workspaceRoot, "tua.toml")), getClient);
}

async function requestProjectLayout(uri, getClient) {
  const client = getClient?.();
  if (!client) {
    throw new Error("Tua language server is not running.");
  }

  const response = await client.sendRequest("tua/projectLayout", { uri: uri.toString() });
  if (!response) {
    return undefined;
  }

  return {
    configDirectory: filePath(response.projectUri),
    configPath: optionalFilePath(response.configUri),
    generatedPath: optionalFilePath(response.generatedUri),
    mainSource: response.mainSource,
    moduleName: response.moduleName,
    outputDirectory: filePath(response.outputUri),
    sourceDirectory: filePath(response.sourceUri),
    workspaceRoot: filePath(response.workspaceUri),
  };
}

function filePath(uri) {
  if (typeof uri !== "string" || !uri) {
    throw new Error("Tua language server returned an invalid project layout.");
  }

  return vscode.Uri.parse(uri).fsPath;
}

function optionalFilePath(uri) {
  return typeof uri === "string" && uri ? filePath(uri) : undefined;
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
