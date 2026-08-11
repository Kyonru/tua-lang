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
        column: animation.column == null ? undefined : integer(animation.column, 0),
        durations: animation.durations == null ? undefined : String(animation.durations),
        durationValues: (Array.isArray(animation.durationValues)
          ? animation.durationValues
          : []).map((duration) => finiteNumber(duration, 0)),
        frame: frameViewModel(animation.frame),
        frames: (Array.isArray(animation.frames) ? animation.frames : []).map(
          (range) => ({
            from: integer(range.from, 0),
            row: integer(range.row, 0),
            to: integer(range.to, 0),
          }),
        ),
        loop: animation.loop !== false,
        markers: (Array.isArray(animation.markers) ? animation.markers : []).map(
          (marker) => ({
            frame: integer(marker.frame, 0),
            name: String(marker.name || ""),
          }),
        ),
        metadata: (Array.isArray(animation.metadata) ? animation.metadata : []).map(
          (metadata) => ({
            origin: originViewModel(metadata.origin),
            sourceHeight: integer(metadata.sourceHeight, 0),
            sourceWidth: integer(metadata.sourceWidth, 0),
            trimX: integer(metadata.trimX, 0),
            trimY: integer(metadata.trimY, 0),
          }),
        ),
        name: String(animation.name || ""),
        onLoop: animation.onLoop == null ? undefined : String(animation.onLoop),
        origin: originViewModel(animation.origin),
        row: integer(animation.row, 0),
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
    frame: frameViewModel(item.frame),
    origin: originViewModel(item.origin),
    imagePath: String(item.image?.path || ""),
    imageStatus: String(item.image?.status || "unchecked"),
    imageUri: imageUri ? webview.asWebviewUri(imageUri).toString() : undefined,
    name: item.name,
  };
}

function originViewModel(origin) {
  return {
    x: integer(origin?.x, 0),
    y: integer(origin?.y, 0),
  };
}

function frameViewModel(frame) {
  return frame && finiteNumber(frame.width) > 0 && finiteNumber(frame.height) > 0
    ? {
        border: integer(frame.border, 0),
        height: integer(frame.height, 0),
        left: integer(frame.left, 0),
        top: integer(frame.top, 0),
        width: integer(frame.width, 0),
      }
    : undefined;
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
