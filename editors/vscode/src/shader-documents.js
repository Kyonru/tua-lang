"use strict";

// Synchronize auxiliary buffers without registering language providers.
// Shader and workspace Lua adapters choose which documents they own.
function synchronizeDocuments(vscode, client, log, accepts) {
  const opened = new Set();
  const send = (method, params) => {
    void client.sendNotification(method, params).catch((error) => log(String(error)));
  };
  const open = (document) => {
    if (!accepts(document) || opened.has(document.uri.toString())) return;
    const uri = document.uri.toString();
    opened.add(uri);
    send("textDocument/didOpen", { textDocument: {
      uri, languageId: document.languageId, version: document.version, text: document.getText(),
    } });
  };
  const disposables = [
    vscode.workspace.onDidOpenTextDocument(open),
    vscode.workspace.onDidChangeTextDocument((event) => {
      if (!accepts(event.document)) return;
      if (!opened.has(event.document.uri.toString())) { open(event.document); return; }
      send("textDocument/didChange", {
        textDocument: { uri: event.document.uri.toString(), version: event.document.version },
        contentChanges: [{ text: event.document.getText() }],
      });
    }),
    vscode.workspace.onDidCloseTextDocument((document) => {
      if (!opened.delete(document.uri.toString())) return;
      send("textDocument/didClose", { textDocument: { uri: document.uri.toString() } });
    }),
  ];
  for (const document of vscode.workspace.textDocuments) open(document);
  return { dispose() { for (const disposable of disposables) disposable.dispose(); opened.clear(); } };
}

function synchronizeShaderDocuments(vscode, client, log) {
  return synchronizeDocuments(vscode, client, log, document => document.uri.scheme === "file"
    && /\.(glsl|vert|frag|vs|fs)$/i.test(document.uri.path));
}

module.exports = { synchronizeShaderDocuments, synchronizeDocuments };
