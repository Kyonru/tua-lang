"use strict";

const childProcess = require("child_process");
const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { formatError } = require("../shared/errors");
const {
  loadSourceMapForGeneratedPath,
  nearestMapping,
  resolveSourceMapPath,
  sourceMapPathForGeneratedPath,
} = require("./source-maps");
const {
  generatedLuaPathForUri,
  normalizePath,
} = require("./project-layout");

function registerGeneratedLuaCommand({ appendOutputLine, getServerCommand, isTuaDocument }) {
  return vscode.Disposable.from(
    vscode.commands.registerCommand(COMMANDS.openGeneratedLua, () =>
      openGeneratedLuaBesideActiveDocument({
        appendOutputLine,
        getServerCommand,
        isTuaDocument,
      }),
    ),
    vscode.commands.registerCommand(COMMANDS.jumpBetweenSourceAndGenerated, () =>
      jumpBetweenSourceAndGenerated({ isTuaDocument }),
    ),
  );
}

async function openGeneratedLuaBesideActiveDocument({
  appendOutputLine,
  getServerCommand,
  isTuaDocument,
}) {
  const document = vscode.window.activeTextEditor?.document;
  if (!document || !isTuaDocument(document)) {
    vscode.window.showWarningMessage("Open a Tua source file to open generated Lua.");
    return undefined;
  }
  if (document.uri.scheme !== "file") {
    vscode.window.showWarningMessage("Generated Lua can only be opened for saved Tua files.");
    return undefined;
  }
  if (!vscode.workspace.isTrusted) {
    vscode.window.showWarningMessage(
      "Trust this workspace before rebuilding and opening generated Lua.",
    );
    return undefined;
  }

  try {
    const generated = resolveGeneratedLua(document.uri);
    if (!generated) {
      return undefined;
    }
    const openGeneratedDocument = vscode.workspace.textDocuments.find(
      (candidate) =>
        candidate.uri.scheme === "file" &&
        normalizePath(candidate.uri.fsPath) === normalizePath(generated.path),
    );
    if (openGeneratedDocument?.isDirty) {
      vscode.window.showWarningMessage(
        "Save or discard edits in the generated Lua file before rebuilding it.",
      );
      return undefined;
    }
    if (document.isDirty && !(await document.save())) {
      vscode.window.showErrorMessage("Could not save the active Tua source before rebuilding.");
      return undefined;
    }

    const command = getServerCommand?.() || "tua";
    appendOutputLine?.(`[tua] Rebuilding generated Lua for ${document.uri.fsPath}`);
    appendOutputLine?.(`$ ${command} build`);
    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: "Tua: Rebuilding generated Lua",
      },
      () => runBuild(command, generated.layout.configDirectory, appendOutputLine),
    );

    if (!fs.existsSync(generated.path)) {
      vscode.window.showWarningMessage(
        `The build completed but did not emit generated Lua at ${generated.path}.`,
      );
      return undefined;
    }

    const luaDocument = await vscode.workspace.openTextDocument(vscode.Uri.file(generated.path));
    await vscode.window.showTextDocument(luaDocument, {
      preview: true,
      viewColumn: vscode.ViewColumn.Beside,
    });
    await vscode.commands.executeCommand("workbench.action.files.revert");

    return generated.path;
  } catch (error) {
    appendOutputLine?.(`[tua] Generated Lua rebuild failed: ${formatError(error)}`);
    vscode.window.showErrorMessage(
      `Could not rebuild and open generated Lua: ${formatError(error)}`,
    );
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

  const sourceMap = loadGeneratedLuaSourceMap(generatedPath);
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
  const sourceMap = loadGeneratedLuaSourceMap(generatedPath);
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
  return resolveGeneratedLua(uri)?.path;
}

function resolveGeneratedLua(uri) {
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

  return resolved;
}

function runBuild(command, cwd, appendOutputLine) {
  return new Promise((resolve, reject) => {
    let settled = false;
    let stdout = "";
    let stderr = "";
    let child;

    const rejectOnce = (error) => {
      if (settled) {
        return;
      }
      settled = true;
      reject(error);
    };

    try {
      child = childProcess.spawn(command, ["build"], {
        cwd,
        env: process.env,
        stdio: ["ignore", "pipe", "pipe"],
        windowsHide: true,
      });
    } catch (error) {
      rejectOnce(error);
      return;
    }

    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });
    child.once("error", rejectOnce);
    child.once("exit", (code) => {
      if (settled) {
        return;
      }
      settled = true;

      appendCommandOutput(stdout, appendOutputLine);
      appendCommandOutput(stderr, appendOutputLine);
      if (code === 0) {
        resolve();
        return;
      }

      const detail = [stderr.trim(), stdout.trim()].find(Boolean);
      reject(
        new Error(
          `\`${command} build\` exited with status ${code}${detail ? `: ${detail}` : ""}`,
        ),
      );
    });
  });
}

function appendCommandOutput(output, appendOutputLine) {
  if (!output || !appendOutputLine) {
    return;
  }

  for (const line of output.split(/\r?\n/u)) {
    if (line) {
      appendOutputLine(line);
    }
  }
}

function loadGeneratedLuaSourceMap(generatedPath) {
  const mapPath = sourceMapPathForGeneratedPath(generatedPath);
  if (!fs.existsSync(mapPath)) {
    vscode.window.showWarningMessage(
      `Generated Lua source map was not found at ${mapPath}. Enable emit_maps and run tua build.`,
    );
    return undefined;
  }

  return loadSourceMapForGeneratedPath(generatedPath);
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

module.exports = {
  registerGeneratedLuaCommand,
};
