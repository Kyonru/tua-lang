"use strict";

const assert = require("assert/strict");
const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const {
  closeActiveEditor,
  eventSequence,
  fixtures,
  hoverText,
  openFixture,
  positionAfter,
  waitForEvent,
  waitUntil,
} = require("./support");

async function runWorkspaceTests() {
  await testSecondWorkspaceFolder();
  await testOpenGeneratedLuaBesideSource();
  await testFileRenameEdits();
  await testAssetRenameEdits();
}

async function testOpenGeneratedLuaBesideSource() {
  const sourcePath = path.join(fixtures.workspaceOne, "src", "open-generated.tua");
  const generatedPath = path.join(fixtures.workspaceOne, "build", "open-generated.lua");

  fs.writeFileSync(sourcePath, "local answer = 42\nreturn answer\n");
  fs.mkdirSync(path.dirname(generatedPath), { recursive: true });
  fs.writeFileSync(generatedPath, "local answer = 42\nreturn answer\n");

  const sourceDocument = await vscode.workspace.openTextDocument(sourcePath);
  await vscode.window.showTextDocument(sourceDocument, {
    preview: false,
    viewColumn: vscode.ViewColumn.One,
  });

  const openedPath = await vscode.commands.executeCommand("tua.openGeneratedLua");
  assert.equal(openedPath, generatedPath);
  assert.equal(vscode.window.activeTextEditor?.document.uri.fsPath, generatedPath);
  assert.equal(vscode.window.activeTextEditor?.viewColumn, vscode.ViewColumn.Two);

  await closeActiveEditor();
  await closeActiveEditor();
}

async function testAssetRenameEdits() {
  const { document } = await openFixture("sprite");
  const oldUri = vscode.Uri.file(fixtures.playerAsset);
  const newPath = path.join(path.dirname(fixtures.playerAsset), "hero.png");
  const newUri = vscode.Uri.file(newPath);
  const beforeRename = eventSequence();
  const edit = new vscode.WorkspaceEdit();

  edit.renameFile(oldUri, newUri);
  assert.equal(await vscode.workspace.applyEdit(edit), true);

  const request = await waitForEvent(
    (event) =>
      event.type === "workspace.willRenameFiles" &&
      event.files.some(
        (file) => file.oldUri === oldUri.toString() && file.newUri === newUri.toString(),
      ),
    "workspace/willRenameFiles for a LOVE asset",
    beforeRename,
  );
  assert(request.editedUris.includes(document.uri.toString()));

  await waitUntil(
    () => document.getText().includes('image = "assets/hero.png"'),
    "the asset file-rename workspace edit to update static paths",
  );
  assert.equal(fs.existsSync(fixtures.playerAsset), false);
  assert.equal(fs.existsSync(newPath), true);
  assert.equal(await document.save(), true);

  await waitForEvent(
    (event) =>
      event.type === "workspace.didRenameFiles" &&
      event.files.some((file) => file.newUri === newUri.toString()),
    "workspace/didRenameFiles after the asset rename",
    beforeRename,
  );
  await closeActiveEditor();
}

async function testSecondWorkspaceFolder() {
  assert.equal(vscode.workspace.workspaceFolders.length, 2);
  const { document } = await openFixture("rootTwo");
  const position = positionAfter(document, "return root_two_valu");
  const beforeHover = eventSequence();
  const hovers = await vscode.commands.executeCommand(
    "vscode.executeHoverProvider",
    document.uri,
    position,
  );

  assert.match(hoverText(hovers), /root_two_value/);
  await waitForEvent(
    (event) =>
      event.type === "hover.request" &&
      event.uri === document.uri.toString() &&
      event.docVersion === document.version,
    "language intelligence in the second workspace folder",
    beforeHover,
  );
  await closeActiveEditor();
}

async function testFileRenameEdits() {
  const { document } = await openFixture("renameSource");
  await vscode.workspace.openTextDocument(fixtures.oldModule);
  const oldUri = vscode.Uri.file(fixtures.oldModule);
  const newPath = path.join(path.dirname(fixtures.oldModule), "new_module.tua");
  const newUri = vscode.Uri.file(newPath);
  const beforeRename = eventSequence();
  const edit = new vscode.WorkspaceEdit();

  edit.renameFile(oldUri, newUri);
  assert.equal(await vscode.workspace.applyEdit(edit), true);

  const request = await waitForEvent(
    (event) =>
      event.type === "workspace.willRenameFiles" &&
      event.files.some(
        (file) => file.oldUri === oldUri.toString() && file.newUri === newUri.toString(),
      ),
    "workspace/willRenameFiles for a Tua module",
    beforeRename,
  );
  assert(request.editedUris.includes(document.uri.toString()));

  await waitUntil(
    () => document.getText().includes('require("new_module")'),
    "the file-rename workspace edit to update require paths",
  );
  assert.equal(fs.existsSync(fixtures.oldModule), false);
  assert.equal(fs.existsSync(newPath), true);
  assert.equal(await document.save(), true);

  await waitForEvent(
    (event) =>
      event.type === "workspace.didRenameFiles" &&
      event.files.some((file) => file.newUri === newUri.toString()),
    "workspace/didRenameFiles after the rename",
    beforeRename,
  );
  await closeActiveEditor();
}

module.exports = {
  runWorkspaceTests,
};
