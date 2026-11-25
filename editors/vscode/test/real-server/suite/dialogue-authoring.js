"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { waitFor } = require("../../webview/cdp");

async function verifyDialogueAuthoring(mainPath) {
  const root = path.join(path.dirname(mainPath), "dialogue-authoring");
  fs.mkdirSync(root, { recursive: true });
  fs.writeFileSync(path.join(root, "tua.toml"), 'source = "."\n[i18n]\nenabled = true\n[[i18n.catalogs]]\nlocale = "en"\npath = "en.lua"\n');
  const write = (name, source) => fs.writeFileSync(path.join(root, name), source);
  write("en.lua", `return { en = { qualification = { hello = "Hello %{name}", go = "Go", bye = "Bye", taken = "Taken" } } }
`);
  write("use.tua", `return I18n.t("qualification.hello", { name = "Player" })
`);
  const cast = `return { Guide = { name = "Guide", expressions = { happy = { portrait = "happy.png" } } } }
`;
  write("characters.tua", cast);
  write("forward.tua", `return require("characters")
`);
  write("context.tua", `return Dialogue.context({
  facts = Save.shape(Save.record({ name = Save.string("") })),
  locals = Save.shape(Save.record({})),
})
`);
  const source = `@dialogue 1
@characters Cast "forward"
@context "context"
@bind name context.name
[start]
// @id qualification.hello
Cast.Guide: Hello %{name}
@expression happy
// @id qualification.go
-> Go [target:done]
[done]
// @id qualification.bye
: Bye (end)
`;
  write("scene.tdialogue", source);
  const document = await vscode.workspace.openTextDocument(path.join(root, "scene.tdialogue"));
  await vscode.window.showTextDocument(document, { preview: false });
  const position = (text, offset = text.length) => {
    const start = document.getText().indexOf(text);
    assert(start >= 0, `missing token ${text}`);
    return document.positionAt(start + offset);
  };
  async function completion(token, label) {
    return waitFor(async () => {
      const list = await vscode.commands.executeCommand("vscode.executeCompletionItemProvider", document.uri, position(token));
      return list?.items.some(item => (typeof item.label === "string" ? item.label : item.label.label) === label);
    }, `dialogue completion ${label}`, 20000);
  }
  async function replace(doc, text) {
    const edits = new vscode.WorkspaceEdit();
    edits.replace(doc.uri, new vscode.Range(doc.positionAt(0), doc.positionAt(doc.getText().length)), text);
    assert(await vscode.workspace.applyEdit(edits));
  }
  await completion("Cast.Gu", "Guide");
  await completion("context.na", "name");
  await completion("@expression ha", "happy");
  await completion("[target:do", "done");
  await completion("%{na", "name");
  await waitFor(async () => {
    const hover = await vscode.commands.executeCommand("vscode.executeHoverProvider", document.uri, position("Cast.Guide", 7));
    return hover?.some(item => item.contents.some(value => (value.value || String(value)).includes("name")));
  }, "dialogue character hover");
  const definitions = await vscode.commands.executeCommand("vscode.executeDefinitionProvider", document.uri, position("Cast.Guide", 7));
  assert(definitions.some(item => (item.targetUri || item.uri).fsPath.endsWith("characters.tua")));
  const references = await vscode.commands.executeCommand("vscode.executeReferenceProvider", document.uri, position("qualification.hello", 16));
  assert.equal(new Set(references.map(item => item.uri.fsPath)).size, 3);
  const rename = await vscode.commands.executeCommand("vscode.executeDocumentRenameProvider", document.uri, position("qualification.hello", 16), "welcome");
  assert.equal(rename.entries().length, 3);
  assert(await vscode.workspace.applyEdit(rename));
  assert(document.getText().includes("// @id qualification.welcome"));
  assert(document.getText().includes("[target:done]"));
  const catalog = await vscode.workspace.openTextDocument(path.join(root, "en.lua"));
  const use = await vscode.workspace.openTextDocument(path.join(root, "use.tua"));
  assert(catalog.getText().includes("welcome"));
  assert(use.getText().includes("qualification.welcome"));
  // An unsaved catalog conflict must reject a second refactor.
  await replace(catalog, catalog.getText().replace("taken", "conflict"));
  await waitFor(async () => {
    try {
      const conflict = await vscode.commands.executeCommand("vscode.executeDocumentRenameProvider", document.uri, position("qualification.welcome", 16), "conflict");
      return !conflict || conflict.size === 0;
    } catch { return true; }
  }, "unsaved catalog conflict blocks rename", 20000);
  const characters = await vscode.workspace.openTextDocument(path.join(root, "characters.tua"));
  await replace(characters, cast.replaceAll("Guide", "GuideChanged"));
  await completion("Cast.Gu", "GuideChanged");
  await replace(characters, cast);
  await characters.save();
  // Closed imported module deletion and watch-driven rebuild must refresh diagnostics.
  fs.unlinkSync(path.join(root, "forward.tua"));
  await waitFor(() => vscode.languages.getDiagnostics(document.uri).some(item => item.message.includes("cannot resolve character module")), "deleted dialogue import", 20000);
  write("forward.tua", `return require("characters")
`);
  await waitFor(() => !vscode.languages.getDiagnostics(document.uri).some(item => item.message.includes("cannot resolve character module")), "recreated dialogue import", 20000);
  await completion("Cast.Gu", "Guide");
  const branch = `@dialogue 1
@context "context"
[start]
@branch true [target:yes] [else:no]
[yes]
// @id qualification.yes
: Yes (end)
[no]
// @id qualification.no
: No (end)
`;
  await replace(document, branch);
  await waitFor(() => vscode.languages.getDiagnostics(document.uri).some(item => item.message.includes("`no` is unreachable")), "constant branch diagnostic", 20000);
  await replace(document, branch.replace("@dialogue 1", "@dialogue 1\n@entry no"));
  await waitFor(() => !vscode.languages.getDiagnostics(document.uri).some(item => item.message.includes("unreachable")), "external entry clears warning", 20000);
  const flowRename = await vscode.commands.executeCommand("vscode.executeDocumentRenameProvider", document.uri, position("@entry no", 8), "external");
  assert.equal(flowRename.entries()[0][1].length, 3);
  await replace(document, '@dialogue 1\n[start]\n@wait 1 [target:');
  await completion("[target:", "start");
  await replace(document, source.split("\n").map(line => line ? "  " + line : line).join("\n"));
  const formatting = await vscode.commands.executeCommand("vscode.executeFormatDocumentProvider", document.uri, { tabSize: 2, insertSpaces: true });
  assert(Array.isArray(formatting) && formatting.length > 0);
  const formatEdit = new vscode.WorkspaceEdit();
  formatEdit.set(document.uri, formatting);
  assert(await vscode.workspace.applyEdit(formatEdit));
  assert.equal(document.getText(), source);
  console.log("REAL_SERVER_DIALOGUE_AUTHORING completion, hover, definition, cross-file references/rename/conflicts, unsaved imports/catalogs, module deletion/recreation, branch diagnostics and incomplete edits passed");
}

module.exports = { verifyDialogueAuthoring };
