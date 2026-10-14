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
  const sourceMapPath = path.join(fixtures.workspaceOne, "build", "open-generated.lua.map.json");

  fs.writeFileSync(sourcePath, "local answer = 42\nreturn answer\n");
  fs.mkdirSync(path.dirname(generatedPath), { recursive: true });
  fs.writeFileSync(generatedPath, "return 'stale generated output'\n");
  fs.writeFileSync(
    sourceMapPath,
    `${JSON.stringify(
      {
        version: 1,
        generated: "build/open-generated.lua",
        source: "src/open-generated.tua",
        lineMappings: [
          { generatedLine: 1, sourceLine: 1 },
          { generatedLine: 2, sourceLine: 2 },
        ],
      },
      null,
      2,
    )}\n`,
  );

  const sourceDocument = await vscode.workspace.openTextDocument(sourcePath);
  const sourceEditor = await vscode.window.showTextDocument(sourceDocument, {
    preview: false,
    viewColumn: vscode.ViewColumn.One,
  });
  await sourceEditor.edit((edit) => {
    const start = new vscode.Position(0, 0);
    const end = sourceDocument.positionAt(sourceDocument.getText().length);

    edit.replace(new vscode.Range(start, end), "local answer = 99\nreturn answer\n");
  });
  assert.equal(sourceDocument.isDirty, true);
  sourceEditor.selection = new vscode.Selection(1, 0, 1, 0);

  const openedPath = await vscode.commands.executeCommand("tua.openGeneratedLua");
  assert.equal(openedPath, generatedPath);
  assert.equal(sourceDocument.isDirty, false);
  assert.equal(fs.readFileSync(generatedPath, "utf8"), "local answer = 99\nreturn answer\n");
  assert.equal(vscode.window.activeTextEditor?.document.uri.fsPath, generatedPath);
  assert.equal(
    vscode.window.activeTextEditor?.document.getText(),
    "local answer = 99\nreturn answer\n",
  );
  assert.equal(vscode.window.activeTextEditor?.viewColumn, vscode.ViewColumn.Two);
  await closeActiveEditor();

  let reopenedSourceEditor = await vscode.window.showTextDocument(sourceDocument, {
    preview: false,
    viewColumn: vscode.ViewColumn.One,
  });
  await vscode.commands.executeCommand("workbench.action.focusFirstEditorGroup");
  await waitUntil(
    () => vscode.window.activeTextEditor?.document.uri.fsPath === sourcePath,
    "the source editor to regain focus before rebuilding generated Lua",
  );
  await reopenedSourceEditor.edit((edit) => {
    const answer = sourceDocument.getText().indexOf("99");
    edit.replace(
      new vscode.Range(
        sourceDocument.positionAt(answer),
        sourceDocument.positionAt(answer + 2),
      ),
      "100",
    );
  });

  assert.equal(await vscode.commands.executeCommand("tua.openGeneratedLua"), generatedPath);
  assert.equal(
    vscode.window.activeTextEditor?.document.getText(),
    "local answer = 100\nreturn answer\n",
  );
  await closeActiveEditor();

  reopenedSourceEditor = await vscode.window.showTextDocument(sourceDocument, {
    preview: false,
    viewColumn: vscode.ViewColumn.One,
  });
  await vscode.commands.executeCommand("workbench.action.focusFirstEditorGroup");
  await waitUntil(
    () => vscode.window.activeTextEditor?.document.uri.fsPath === sourcePath,
    "the source editor to regain focus before jumping to generated Lua",
  );
  reopenedSourceEditor.selection = new vscode.Selection(1, 0, 1, 0);
  const openedGeneratedPath = await vscode.commands.executeCommand(
    "tua.jumpBetweenSourceAndGenerated",
  );
  assert.equal(openedGeneratedPath, generatedPath);
  assert.equal(vscode.window.activeTextEditor?.document.uri.fsPath, generatedPath);
  assert.equal(vscode.window.activeTextEditor?.selection.active.line, 1);

  const openedSourcePath = await vscode.commands.executeCommand(
    "tua.jumpBetweenSourceAndGenerated",
  );
  assert.equal(openedSourcePath, sourcePath);
  assert.equal(vscode.window.activeTextEditor?.document.uri.fsPath, sourcePath);
  assert.equal(vscode.window.activeTextEditor?.selection.active.line, 1);

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
