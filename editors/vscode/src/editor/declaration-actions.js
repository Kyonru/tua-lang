"use strict";
const vscode = require("vscode");
const { STATUS_SELECTOR } = require("../constants");
const { declarationAt } = require("./declaration-target");

function actionsFor(target) {
  const actions = [];
  if (target.helper === "stateMachine") actions.push({ title: `Preview ${target.name} State Machine`, command: "tua.previewStateMachine", arguments: [target] });
  if (target.helper === "objectPool") actions.push({ title: `Explore ${target.name} Object Pool`, command: "tua.showObjectPoolExplorer", arguments: [target] });
  if (target.kind !== "input") actions.push({ title: `Inspect ${target.name} Runtime Instance…`, command: "tua.inspector.forDeclaration", arguments: [target] });
  actions.push({ title: `Open ${target.helper} API Reference`, command: "vscode.open", arguments: [vscode.Uri.parse(`tua:/builtins/${target.helper}.tua`)] });
  return actions;
}

function registerDeclarationActions({ getClient, isTuaDocument }) {
  async function targetAt(document, position, token) {
    if (!isTuaDocument(document) || document.getText().length > 2 * 1024 * 1024) return;
    const version = document.version;
    const declaration = declarationAt(document.getText(), document.offsetAt(position));
    const client = getClient();
    if (!declaration || !client || token?.isCancellationRequested) return;
    const helperPosition = document.positionAt(declaration.helperOffset);
    try {
      const result = await client.sendRequest("textDocument/definition", {
        textDocument: { uri: document.uri.toString() }, position: { line: helperPosition.line, character: helperPosition.character },
      });
      const locations = Array.isArray(result) ? result : [result];
      if (!locations.some(location => (location?.uri || location?.targetUri) === `tua:/builtins/${declaration.helper}.tua`)) return;
    } catch { return; }
    if (token?.isCancellationRequested || version !== document.version) return;
    const namePosition = document.positionAt(declaration.nameOffset);
    return { ...declaration, uri: document.uri.toString(), path: document.uri.scheme === "file" ? document.uri.fsPath : undefined,
      line: namePosition.line + 1, position: { line: namePosition.line, character: namePosition.character } };
  }
  return vscode.Disposable.from(
    vscode.languages.registerCodeActionsProvider(STATUS_SELECTOR, {
      async provideCodeActions(document, range, _context, token) {
        if (_context.only) return [];
        const target = await targetAt(document, range.start, token);
        if (!target) return [];
        return actionsFor(target).map(command => { const action = new vscode.CodeAction(command.title); action.command = command; return action; });
      },
    }),
    vscode.commands.registerCommand("tua.declaration.tools", async () => {
      const editor = vscode.window.activeTextEditor;
      const target = editor && await targetAt(editor.document, editor.selection.active);
      if (!target) return vscode.window.showInformationMessage("Place the cursor inside a local stateMachine, input, store, or objectPool declaration.");
      const picked = await vscode.window.showQuickPick(actionsFor(target).map(action => ({ label: action.title, action })), { placeHolder: `Tools for ${target.name}` });
      if (picked) await vscode.commands.executeCommand(picked.action.command, ...picked.action.arguments);
    }),
  );
}
module.exports = { registerDeclarationActions, actionsFor };
