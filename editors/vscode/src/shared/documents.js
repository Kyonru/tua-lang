"use strict";

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

function isStaleDocumentResponse(response, document) {
  return Number.isInteger(document.version) && response?.documentVersion !== document.version;
}

module.exports = {
  isStaleDocumentResponse,
  isTuaDocument,
};
