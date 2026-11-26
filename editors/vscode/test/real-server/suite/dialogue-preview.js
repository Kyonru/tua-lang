"use strict";

const assert = require("assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { previewContext } = require("./graph-previews");
const { waitFor } = require("../../webview/cdp");

async function verifyDialoguePreview(mainPath) {
  const file = path.join(path.dirname(mainPath), "preview.tdialogue");
  const source = text => `@dialogue 1\n[start]\n// @id greeting\n: ${text}\n// @id proceed\n-> Continue [target:done]\n[done]\n// @id bye\n: Goodbye (end)\n`;
  fs.writeFileSync(file, source("Initial native dialogue"));
  await vscode.workspace.getConfiguration("tua").update("preview.enabled", true, vscode.ConfigurationTarget.Workspace);
  let document = await vscode.workspace.openTextDocument(file);
  const editor = await vscode.window.showTextDocument(document, { viewColumn: vscode.ViewColumn.One, preview: false });
  assert.equal(document.languageId, "tdialogue");
  await vscode.commands.executeCommand("tua.previewDialogue");
  const { browser } = await previewContext(process.env.TUA_REAL_SERVER_CDP_PORT, "Dialogue Preview");
  let context;
  async function rendered(text) {
    let lastText;
    return waitFor(async () => {
      try {
        context = await browser.documentContext("Dialogue Preview");
        lastText = await browser.evaluate("document.body.innerText", context);
        return lastText.includes(text);
      } catch {
        return false;
      }
    }, `dialogue preview: ${text}`, 15000).catch(error => {
      throw new Error(`${error.message}; document v${document.version}, closed=${document.isClosed}, dirty=${document.isDirty}; last view: ${lastText?.slice(0, 1800)}`);
    });
  }
  async function replace(text) {
    const edit = new vscode.WorkspaceEdit();
    edit.replace(document.uri, new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length)), text);
    assert(await vscode.workspace.applyEdit(edit));
  }
  try {
    await rendered("Initial native dialogue");
    await browser.evaluate("document.querySelector('.dialogue-map').open = true", context);
    assert.equal(await browser.evaluate("document.querySelectorAll('.dialogue-node').length", context), 2);
    editor.selection = new vscode.Selection(0, 0, 0, 0);
    await browser.evaluate("document.querySelector('main .line-source .source').click()", context);
    await waitFor(() => {
      const active = vscode.window.activeTextEditor;
      return active?.document.uri.toString() === document.uri.toString() && active.selection.active.line === 3;
    }, "real dialogue source navigation");
    const sourceTabs = () => vscode.window.tabGroups.all.flatMap(group => group.tabs)
      .filter(tab => tab.input instanceof vscode.TabInputText && tab.input.uri.toString() === document.uri.toString());
    assert.equal(sourceTabs().length, 1, "navigation must reuse the existing source tab");
    const previewGroupNow = () => vscode.window.tabGroups.all.find(group => group.tabs.some(tab =>
      tab.input instanceof vscode.TabInputWebview && tab.input.viewType.endsWith("tuaDialoguePreview")));
    assert.notEqual(vscode.window.activeTextEditor.viewColumn, previewGroupNow().viewColumn);
    // Hide the existing source behind another file; it must still be reused.
    const otherSource = await vscode.workspace.openTextDocument(mainPath);
    await vscode.window.showTextDocument(otherSource, { viewColumn: editor.viewColumn, preview: false });
    await browser.evaluate("document.querySelector('main .line-source .source').click()", context);
    await waitFor(() => vscode.window.activeTextEditor?.document.uri.toString() === document.uri.toString(), "reuse hidden source tab");
    assert.equal(sourceTabs().length, 1);
    await vscode.commands.executeCommand("tua.previewDialogue");
    await rendered("Initial native dialogue");
    // Put preview and source in the same group. Navigation should move the
    // preview aside instead of duplicating the source into a fresh group.
    await vscode.commands.executeCommand("workbench.action.moveEditorToPreviousGroup");
    await waitFor(() => previewGroupNow()?.viewColumn === editor.viewColumn, "preview moved into source group");
    context = await browser.documentContext("Dialogue Preview");
    await browser.evaluate("document.querySelector('main .line-source .source').click()", context);
    await waitFor(() => vscode.window.activeTextEditor?.document.uri.toString() === document.uri.toString()
      && vscode.window.activeTextEditor.viewColumn !== previewGroupNow()?.viewColumn, "restore side-by-side source navigation");
    assert.equal(sourceTabs().length, 1);
    await vscode.commands.executeCommand("tua.previewDialogue");
    await rendered("Initial native dialogue");

    await replace(source("Unsaved native dialogue"));
    await rendered("Unsaved native dialogue");
    assert(!fs.readFileSync(file, "utf8").includes("Unsaved"));
    await replace("@dialogue 1\n[broken");
    await rendered("Check Problems");
    assert.equal(await browser.evaluate("document.querySelectorAll('[data-source]').length", context), 0);
    await replace(source("Recovered native dialogue"));
    await document.save();
    await rendered("Recovered native dialogue");

    fs.writeFileSync(file, source("External native dialogue"));
    await rendered("External native dialogue");
    fs.unlinkSync(file);
    await rendered("Dialogue file was deleted");
    assert.equal(await browser.evaluate("document.querySelectorAll('[data-source]').length", context), 0);
    fs.writeFileSync(file, source("Recreated native dialogue"));
    await rendered("Recreated native dialogue");

    document = await vscode.workspace.openTextDocument(file);
    await replace(source("Dirty native dialogue"));
    await rendered("Dirty native dialogue");
    fs.unlinkSync(file);
    // Observe the native filesystem event before asserting dirty-buffer authority.
    await new Promise(resolve => setTimeout(resolve, 500));
    await rendered("Dirty native dialogue");
    await vscode.window.showTextDocument(document, { viewColumn: vscode.ViewColumn.One, preview: false });
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    await rendered("Dialogue file was deleted");
    fs.writeFileSync(file, source("Reopened native dialogue"));
    await rendered("Reopened native dialogue");

    const previewGroup = vscode.window.tabGroups.all.find(group => group.tabs.some(tab =>
      tab.input instanceof vscode.TabInputWebview && tab.input.viewType.endsWith("tuaDialoguePreview")));
    assert(previewGroup, "dialogue preview group should exist");
    const isPreviewActive = () => previewGroup.activeTab?.input instanceof vscode.TabInputWebview;
    const other = await vscode.workspace.openTextDocument(mainPath);
    await vscode.window.showTextDocument(other, { viewColumn: previewGroup.viewColumn, preview: false });
    assert.equal(isPreviewActive(), false, "the preview must be hidden during the disk edit");
    fs.writeFileSync(file, source("Hidden native dialogue"));
    await new Promise(resolve => setTimeout(resolve, 500));
    await vscode.commands.executeCommand("workbench.action.previousEditor");
    await waitFor(isPreviewActive, "hidden dialogue preview reveal");
    await rendered("Hidden native dialogue");
    console.log("REAL_SERVER_DIALOGUE_PREVIEW source navigation, overlays, invalid edits, external changes, deletion/recreation, dirty discard and hidden refresh passed");
  } finally {
    browser.dispose();
  }
}

module.exports = { verifyDialoguePreview };
