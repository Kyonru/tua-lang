"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");

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
  await vscode.commands.executeCommand("editor.action.formatDocument");
  const formatted = document.getText();
  assert.notEqual(formatted, source, "format document must apply the bundled server's edits");
  assert(formatted.includes('local label = "ready"'), "shared quote default");
  assert(formatted.includes("  read(): number {\n    return self.value\n  }"), "authored class method layout");
  await vscode.commands.executeCommand("editor.action.formatDocument");
  assert.equal(document.getText(), formatted, "second pass must be stable");

  const saveSource = "local label='saved'\n";
  await editor.edit(edit => edit.replace(new vscode.Range(0, 0, document.lineCount, 0), saveSource));
  assert(await document.save());
  assert.equal(document.getText(), 'local label = "saved"\n', "format on save");
  assert.equal(fs.readFileSync(file, "utf8"), document.getText(), "formatted text reaches disk");

  const rangeSource = 'local prefix="🐸"\nlocal selected={a=1,b=2}\nlocal untouched={x=3}\n';
  await editor.edit(edit => edit.replace(new vscode.Range(0, 0, document.lineCount, 0), rangeSource));
  editor.selection = new vscode.Selection(1, 0, 2, 0);
  await vscode.commands.executeCommand("editor.action.formatSelection");
  assert.equal(document.getText(), 'local prefix="🐸"\nlocal selected = { a = 1, b = 2 }\nlocal untouched={x=3}\n', "range formatting leaves surrounding source unchanged");
  await settings.update("defaultFormatter", previousFormatter, vscode.ConfigurationTarget.Workspace);
  await settings.update("formatOnSave", previousFormatOnSave, vscode.ConfigurationTarget.Workspace);
  console.log(JSON.stringify({ sourceFormatting: { document: true, stable: true, save: true, range: true, host: vscode.version } }));
}

module.exports = { verifySourceFormatting };
