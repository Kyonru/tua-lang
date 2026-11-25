"use strict";

const { DOCUMENT_SELECTOR } = require("./constants");
const { synchronizeDocuments } = require("./shader-documents");

// Project Lua can supply catalogs and imported types without using Tua's providers.
// Keep these buffers authoritative while dirty, including delete/recreate cycles.
function synchronizeLuaDocuments(vscode, client, log) {
  return synchronizeDocuments(vscode, client, log, document => document.uri.scheme === "file"
    && /\.lua$/i.test(document.uri.path)
    && Boolean(vscode.workspace.getWorkspaceFolder(document.uri))
    && vscode.languages.match(DOCUMENT_SELECTOR, document) === 0);
}

module.exports = { synchronizeLuaDocuments };
