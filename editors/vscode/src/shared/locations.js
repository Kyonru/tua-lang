"use strict";

const path = require("path");
const vscode = require("vscode");

function activeFileUri() {
  const uri = vscode.window.activeTextEditor?.document?.uri;
  return uri?.scheme === "file" ? uri.toString() : undefined;
}

function displayUriPath(uriValue) {
  const uri = vscode.Uri.parse(uriValue);

  if (typeof vscode.workspace.asRelativePath === "function") {
    const relative = vscode.workspace.asRelativePath(uri, false);
    if (relative) {
      return relative;
    }
  }

  return uri.fsPath ? path.basename(uri.fsPath) : uriValue;
}

function lspRangeToVscode(range) {
  if (!range?.start || !range?.end) {
    return undefined;
  }

  const start = new vscode.Position(
    protocolCoordinate(range.start.line, 0),
    protocolCoordinate(range.start.character, 0),
  );
  const end = new vscode.Position(
    protocolCoordinate(range.end.line, start.line),
    protocolCoordinate(range.end.character, start.character),
  );

  return new vscode.Range(start, end);
}

function protocolCoordinate(value, fallback) {
  const number = Number(value);
  return Number.isInteger(number) && number >= 0 ? number : fallback;
}

async function openLocationMessage(message) {
  if (message?.command !== "openLocation" || typeof message.uri !== "string") {
    return false;
  }

  const selection = lspRangeToVscode(message.range) || new vscode.Range(0, 0, 0, 0);

  await vscode.commands.executeCommand("vscode.open", vscode.Uri.parse(message.uri), {
    preview: true,
    selection,
  });

  return true;
}

module.exports = {
  activeFileUri,
  displayUriPath,
  lspRangeToVscode,
  openLocationMessage,
};
