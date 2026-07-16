"use strict";

const path = require("path");
const vscode = require("vscode");

function spritePreviewViewModel(webview, document, inspection) {
  const sprites = inspection.items.map((item) => spriteViewModel(webview, item));
  const localResourceRoots = uniqueUriRoots(
    inspection.items
      .map((item) => fileUri(item.image?.uri))
      .filter(Boolean)
      .map((uri) => vscode.Uri.file(path.dirname(uri.fsPath))),
  );

  return {
    analysisRevision: inspection.analysisRevision,
    documentVersion: inspection.documentVersion,
    localResourceRoots,
    source: document.uri.fsPath,
    sprites,
    summary: {
      animationCount: sprites.reduce(
        (count, sprite) => count + sprite.animations.length,
        0,
      ),
      missingAssetCount: sprites.filter(
        (sprite) => sprite.imageStatus !== "valid" || !sprite.imageUri,
      ).length,
      spriteCount: sprites.length,
    },
  };
}

function spriteViewModel(webview, item) {
  const imageUri = fileUri(item.image?.uri);

  return {
    animations: (Array.isArray(item.animations) ? item.animations : []).map(
      (animation) => ({
        fps: finiteNumber(animation.fps, 0),
        from: integer(animation.from, 0),
        loop: animation.loop !== false,
        name: String(animation.name || ""),
        to: integer(animation.to, 0),
      }),
    ),
    diagnostics: (Array.isArray(item.diagnostics) ? item.diagnostics : []).map(
      (diagnostic) => ({
        code: diagnostic.code ? String(diagnostic.code) : "",
        message: String(diagnostic.message || ""),
        severity: integer(diagnostic.severity, 1),
      }),
    ),
    frame:
      item.frame && finiteNumber(item.frame.width) > 0 && finiteNumber(item.frame.height) > 0
        ? {
            height: integer(item.frame.height, 0),
            width: integer(item.frame.width, 0),
          }
        : undefined,
    imagePath: String(item.image?.path || ""),
    imageStatus: String(item.image?.status || "unchecked"),
    imageUri: imageUri ? webview.asWebviewUri(imageUri).toString() : undefined,
    name: item.name,
  };
}

function spritePreviewWebviewOptions(viewModel) {
  return {
    enableScripts: true,
    localResourceRoots: viewModel.localResourceRoots || [],
    retainContextWhenHidden: true,
  };
}

function fileUri(value) {
  if (typeof value !== "string") {
    return undefined;
  }

  const uri = vscode.Uri.parse(value);

  return uri.scheme === "file" && uri.fsPath ? uri : undefined;
}

function uniqueUriRoots(roots) {
  const unique = [];
  const seen = new Set();

  for (const root of roots) {
    const key = root.toString();

    if (!seen.has(key)) {
      seen.add(key);
      unique.push(root);
    }
  }

  return unique;
}

function finiteNumber(value, fallback) {
  const number = Number(value);

  return Number.isFinite(number) ? number : fallback;
}

function integer(value, fallback) {
  return Math.trunc(finiteNumber(value, fallback));
}

module.exports = {
  spritePreviewViewModel,
  spritePreviewWebviewOptions,
};
