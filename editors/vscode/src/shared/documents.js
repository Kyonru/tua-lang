"use strict";

const vscode = require("vscode");

function isTuaDocument(document) {
  if (document.languageId === "tua") {
    return true;
  }

  const scheme = document.uri?.scheme;
  if (scheme && scheme !== "file" && scheme !== "untitled") {
    return false;
  }

  const filename = document.fileName || document.uri?.fsPath || document.uri?.path || "";

  return filename.endsWith(".tua");
}

function isActiveDocument(document) {
  const active = vscode.window.activeTextEditor?.document;

  if (!active) {
    return false;
  }

  return active === document || active.uri?.toString() === document.uri?.toString();
}

function documentDebugLabel(document) {
  const uri = document?.uri?.toString?.() || document?.fileName || "<unknown>";
  const language = document?.languageId || "<unknown>";
  const version = document?.version ?? "<unknown>";

  return `uri=${uri} language=${language} version=${version}`;
}

function contentChangeDebugLabel(change) {
  const text = change?.text || "";
  const range = change?.range
    ? `${change.range.start.line}:${change.range.start.character}-${change.range.end.line}:${change.range.end.character}`
    : "full";

  return `range=${range} text=${textPreview(text)} length=${text.length}`;
}

function textPreview(text) {
  if (text.length <= 24) {
    return JSON.stringify(text);
  }

  return `${JSON.stringify(text.slice(0, 24))}...`;
}

module.exports = {
  contentChangeDebugLabel,
  documentDebugLabel,
  isActiveDocument,
  isTuaDocument,
};
