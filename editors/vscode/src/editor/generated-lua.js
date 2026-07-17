"use strict";

const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { formatError } = require("../shared/errors");
const {
  generatedLuaPathForUri,
  normalizePath,
  normalizeRelativePath,
  replaceExtension,
} = require("./project-layout");

function registerGeneratedLuaCommand({ isTuaDocument }) {
  return vscode.Disposable.from(
    vscode.commands.registerCommand(COMMANDS.openGeneratedLua, () =>
      openGeneratedLuaBesideActiveDocument({ isTuaDocument }),
    ),
    vscode.commands.registerCommand(COMMANDS.jumpBetweenSourceAndGenerated, () =>
      jumpBetweenSourceAndGenerated({ isTuaDocument }),
    ),
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

async function jumpBetweenSourceAndGenerated({ isTuaDocument }) {
  const document = vscode.window.activeTextEditor?.document;
  if (!document || document.uri.scheme !== "file") {
    vscode.window.showWarningMessage("Open a saved Tua or generated Lua file to jump positions.");
    return undefined;
  }

  try {
    if (isTuaDocument(document)) {
      return jumpFromSourceToGenerated(document);
    }

    if (path.extname(document.uri.fsPath) === ".lua") {
      return jumpFromGeneratedToSource(document);
    }

    vscode.window.showWarningMessage("Open a Tua source file or generated Lua file to jump.");
    return undefined;
  } catch (error) {
    vscode.window.showErrorMessage(
      `Could not jump between Tua and generated Lua: ${formatError(error)}`,
    );
    return undefined;
  }
}

async function jumpFromSourceToGenerated(document) {
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

  const sourceMap = loadSourceMapForGeneratedPath(generatedPath);
  if (!sourceMap) {
    return undefined;
  }

  const sourceLine = activeLineOneBased();
  const mapping = nearestMapping(sourceMap.lineMappings, "sourceLine", sourceLine);
  if (!mapping) {
    vscode.window.showWarningMessage("Generated Lua source map has no line mappings.");
    return undefined;
  }

  await openFileAtMappedLine(generatedPath, mapping.generatedLine);
  return generatedPath;
}

async function jumpFromGeneratedToSource(document) {
  const generatedPath = path.resolve(document.uri.fsPath);
  const sourceMap = loadSourceMapForGeneratedPath(generatedPath);
  if (!sourceMap) {
    return undefined;
  }

  const sourcePath = resolveSourceMapPath(sourceMap.root, sourceMap.source);
  if (!sourcePath || !fs.existsSync(sourcePath)) {
    vscode.window.showWarningMessage(
      `Mapped Tua source was not found at ${sourcePath || sourceMap.source}.`,
    );
    return undefined;
  }

  const generatedLine = activeLineOneBased();
  const mapping = nearestMapping(sourceMap.lineMappings, "generatedLine", generatedLine);
  if (!mapping) {
    vscode.window.showWarningMessage("Generated Lua source map has no line mappings.");
    return undefined;
  }

  await openFileAtMappedLine(sourcePath, mapping.sourceLine);
  return sourcePath;
}

function resolveGeneratedLuaPath(uri) {
  const resolved = generatedLuaPathForUri(uri);
  if (resolved.kind === "missingWorkspace") {
    vscode.window.showWarningMessage("Tua source file is not inside an open workspace folder.");
    return undefined;
  }
  if (resolved.kind === "outsideSource") {
    vscode.window.showWarningMessage(
      `Tua source file is outside configured source directory ${resolved.sourceDirectory}.`,
    );
    return undefined;
  }

  return resolved.path;
}

function loadSourceMapForGeneratedPath(generatedPath) {
  const mapPath = sourceMapPathForGeneratedPath(generatedPath);
  if (!fs.existsSync(mapPath)) {
    vscode.window.showWarningMessage(
      `Generated Lua source map was not found at ${mapPath}. Enable emit_maps and run tua build.`,
    );
    return undefined;
  }

  const parsed = JSON.parse(fs.readFileSync(mapPath, "utf8"));
  if (parsed?.version !== 1) {
    throw new Error(`unsupported source map version in ${mapPath}`);
  }

  const lineMappings = Array.isArray(parsed.lineMappings)
    ? parsed.lineMappings
        .map((mapping) => ({
          generatedLine: integerLine(mapping?.generatedLine),
          sourceLine: integerLine(mapping?.sourceLine),
        }))
        .filter((mapping) => mapping.generatedLine && mapping.sourceLine)
    : [];
  const root = sourceMapRoot(generatedPath, parsed.generated) || path.dirname(mapPath);

  return {
    generated: typeof parsed.generated === "string" ? parsed.generated : undefined,
    lineMappings,
    root,
    source: typeof parsed.source === "string" ? parsed.source : undefined,
  };
}

function sourceMapPathForGeneratedPath(generatedPath) {
  return replaceExtension(generatedPath, ".lua.map.json");
}

function sourceMapRoot(generatedPath, generated) {
  if (typeof generated !== "string" || path.isAbsolute(generated)) {
    return undefined;
  }

  const normalizedGeneratedPath = normalizePath(generatedPath);
  const relativeGeneratedPath = normalizeRelativePath(generated);
  const suffix = `${path.sep}${relativeGeneratedPath}`;

  if (!normalizedGeneratedPath.endsWith(suffix)) {
    return undefined;
  }

  return (
    generatedPath.slice(0, generatedPath.length - suffix.length) ||
    path.parse(generatedPath).root
  );
}

function resolveSourceMapPath(root, mappedPath) {
  if (typeof mappedPath !== "string" || !mappedPath) {
    return undefined;
  }

  return path.resolve(root, mappedPath);
}

function nearestMapping(mappings, field, targetLine) {
  let best;
  let bestDistance = Number.MAX_SAFE_INTEGER;

  for (const mapping of mappings) {
    const line = mapping[field];
    const distance = Math.abs(line - targetLine);
    if (
      distance < bestDistance ||
      (distance === bestDistance &&
        (!best || mapping.generatedLine < best.generatedLine))
    ) {
      best = mapping;
      bestDistance = distance;
    }
  }

  return best;
}

async function openFileAtMappedLine(filePath, lineOneBased) {
  const position = new vscode.Position(Math.max(0, lineOneBased - 1), 0);
  const selection = new vscode.Range(position, position);
  const uri = vscode.Uri.file(filePath);
  const visibleEditor = vscode.window.visibleTextEditors.find(
    (editor) => normalizePath(editor.document.uri.fsPath) === normalizePath(filePath),
  );
  const document = visibleEditor?.document || (await vscode.workspace.openTextDocument(uri));

  await vscode.window.showTextDocument(document, {
    preview: true,
    selection,
    viewColumn: visibleEditor?.viewColumn || vscode.ViewColumn.Beside,
  });
}

function activeLineOneBased() {
  return (vscode.window.activeTextEditor?.selection?.active?.line || 0) + 1;
}

function integerLine(value) {
  return Number.isInteger(value) && value > 0 ? value : undefined;
}

module.exports = {
  registerGeneratedLuaCommand,
};
