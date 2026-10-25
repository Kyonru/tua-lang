"use strict";

// Synchronize shader overlays without registering Tua's completion/formatting
// providers for GLSL. The server publishes shader diagnostics separately.
function synchronizeShaderDocuments(vscode, client, log) {
  const opened = new Set();
  const isShader = (document) => document.uri.scheme === "file"
    && /\.(glsl|vert|frag|vs|fs)$/i.test(document.uri.path);
  const send = (method, params) => {
    void client.sendNotification(method, params).catch((error) => log(String(error)));
  };
  const open = (document) => {
    if (!isShader(document) || opened.has(document.uri.toString())) return;
    const uri = document.uri.toString();
    opened.add(uri);
    send("textDocument/didOpen", { textDocument: {
      uri, languageId: document.languageId, version: document.version, text: document.getText(),
    } });
  };
  const disposables = [
    vscode.workspace.onDidOpenTextDocument(open),
    vscode.workspace.onDidChangeTextDocument((event) => {
      if (!isShader(event.document)) return;
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

module.exports = { synchronizeShaderDocuments };
