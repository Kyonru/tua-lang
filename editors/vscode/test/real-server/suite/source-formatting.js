"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { waitFor } = require("../../webview/cdp");

async function verifySourceFormatting(mainPath) {
  const file = path.join(path.dirname(mainPath), "source-formatting.tua");
  const source = "protocol Reader {\nread(): number\n}\nclass Counter implements Reader {\nvalue:number=1\nread():number { return self.value }\n}\nlocal label='ready'\n";
  fs.writeFileSync(file, source);
  const document = await vscode.workspace.openTextDocument(file);
  const editor = await vscode.window.showTextDocument(document, { preview: false });
  const settings = vscode.workspace.getConfiguration("editor", document.uri);
  const previousFormatter = settings.inspect("defaultFormatter").workspaceValue;
  const previousFormatOnSave = settings.inspect("formatOnSave").workspaceValue;
  await settings.update("defaultFormatter", "SolenodonteLabs.tua", vscode.ConfigurationTarget.Workspace);
  await settings.update("formatOnSave", true, vscode.ConfigurationTarget.Workspace);
  await waitFor(() => {
    const current = vscode.workspace.getConfiguration("editor", document.uri);
    return current.get("defaultFormatter") === "SolenodonteLabs.tua" && current.get("formatOnSave") === true;
  }, "formatter settings to reach the editor", 10000);
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  await vscode.commands.executeCommand("editor.action.formatDocument");
  await waitFor(() => document.getText() !== source, "format document edits", 10000);
  const formatted = document.getText();
  assert.notEqual(formatted, source, "format document must apply the bundled server's edits");
  assert(formatted.includes('local label = "ready"'), "shared quote default");
  assert(formatted.includes("  read(): number {\n    return self.value\n  }"), "authored class method layout");
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  await vscode.commands.executeCommand("editor.action.formatDocument");
  assert.equal(document.getText(), formatted, "second pass must be stable");

  const nestedSource = "package.loaded['demo.state'] = { state = { update = function(_, callback) callback(value) end } }\n";
  await editor.edit(edit => edit.replace(new vscode.Range(0, 0, document.lineCount, 0), nestedSource));
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  await vscode.commands.executeCommand("editor.action.formatDocument");
  await waitFor(() => document.getText() !== nestedSource, "nested table format edits", 10000);
  const nestedFormatted = document.getText();
  assert(nestedFormatted.includes('package.loaded["demo.state"] = {\n  state = {\n'), "nested callback expands its parent table");
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  await vscode.commands.executeCommand("editor.action.formatDocument");
  assert.equal(document.getText(), nestedFormatted, "nested table stabilizes in one pass");

  const safeSource = "local safe=call(1,2)\n";
  await editor.edit(edit => edit.replace(new vscode.Range(0, 0, document.lineCount, 0), safeSource));
  const safeEdits = await vscode.commands.executeCommand(
    "vscode.executeFormatOnTypeProvider", document.uri, new vscode.Position(0, 20), ")",
    { tabSize: 2, insertSpaces: true },
  );
  assert(safeEdits?.length > 0, "on-type provider returns edits for a safe change");

  const partialSource = "local first=1; local second=2\n\n";
  await editor.edit(edit => edit.replace(new vscode.Range(0, 0, document.lineCount, 0), partialSource));
  const partialEdits = await vscode.commands.executeCommand(
    "vscode.executeFormatOnTypeProvider",
    document.uri,
    new vscode.Position(1, 0),
    "\n",
    { tabSize: 2, insertSpaces: true },
  );
  assert.deepEqual(partialEdits ?? [], [], "on-type formatting declines edits that would drop a moved statement");
  assert.equal(document.getText(), partialSource, "on-type safety probe leaves both statements intact");

  const saveSource = "local label='saved'\n";
  const savedFormatted = 'local label = "saved"\n';
  await editor.edit(edit => edit.replace(new vscode.Range(0, 0, document.lineCount, 0), saveSource));
  assert(document.isDirty, "format-on-save probe must start with an unsaved edit");
  await vscode.window.showTextDocument(document, { preview: false });
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  assert.equal(
    vscode.window.activeTextEditor?.document.uri.toString(),
    document.uri.toString(),
    "format-on-save probe must save the active source editor",
  );
  assert.equal(await document.save(), true, "format-on-save probe must save the source document");
  await waitFor(() => document.getText() === savedFormatted, "format-on-save edits", 30000);
  await waitFor(
    () => fs.readFileSync(file, "utf8") === savedFormatted,
    "formatted text to reach disk",
    30000,
  );

  const rangeSource = 'local prefix="🐸"\nlocal selected={a=1,b=2}\nlocal untouched={x=3}\n';
  await editor.edit(edit => edit.replace(new vscode.Range(0, 0, document.lineCount, 0), rangeSource));
  editor.selection = new vscode.Selection(1, 0, 2, 0);
  await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
  await vscode.commands.executeCommand("editor.action.formatSelection");
  await waitFor(() => document.getText() !== rangeSource, "range format edits", 10000);
  assert.equal(document.getText(), 'local prefix="🐸"\nlocal selected = { a = 1, b = 2 }\nlocal untouched={x=3}\n', "range formatting leaves surrounding source unchanged");
  await settings.update("defaultFormatter", previousFormatter, vscode.ConfigurationTarget.Workspace);
  await settings.update("formatOnSave", previousFormatOnSave, vscode.ConfigurationTarget.Workspace);
  console.log(JSON.stringify({ sourceFormatting: { document: true, stable: true, nestedTables: true, onTypeSafety: true, save: true, range: true, host: vscode.version } }));
}

module.exports = { verifySourceFormatting };
