"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { BrowserSession, waitFor } = require("../../webview/cdp");

async function verifyDialogueFormatting(mainPath) {
  const file = path.join(path.dirname(mainPath), "formatting.tdialogue");
  const source = "@dialogue 1\r\n@text rich\r\n[start]\r\n// @id formatting.greeting\r\n: 🐸 café (end)\r\n";
  fs.writeFileSync(file, source);
  const document = await vscode.workspace.openTextDocument(file);
  const editor = await vscode.window.showTextDocument(document, { preview: false });
  const start = document.positionAt(source.indexOf("café"));
  const end = start.translate(0, 4);
  const targets = await (await fetch(`http://127.0.0.1:${process.env.TUA_REAL_SERVER_CDP_PORT}/json/list`)).json();
  const page = targets.find(target => target.type === "page");
  assert(page, "native editor debugger target");
  const browser = await BrowserSession.connect(page.webSocketDebuggerUrl);
  const buttons = `[...document.querySelectorAll('.editor-actions .action-label')]
    .filter(element => /Emphasis|Italic|Underline|Strikethrough|Highlight|Whisper/.test(element.getAttribute('aria-label') || ''))`;
  async function state() {
    return browser.evaluate(`${buttons}.map(element => ({
      label: element.getAttribute('aria-label'),
      disabled: element.getAttribute('aria-disabled') === 'true' || element.closest('.action-item').classList.contains('disabled'),
      active: (element.getAttribute('aria-label') || '').includes('(active)')
    }))`);
  }
  async function ready(active = []) {
    await waitFor(async () => {
      const items = await state();
      return items.length === 6 && items.every(item => !item.disabled &&
        item.active === active.some(name => item.label.includes(name)));
    }, `enabled icon toggles, pressed: ${active.join(', ')}`, 15000);
  }
  async function disabled() {
    await waitFor(async () => {
      const items = await state();
      return items.length === 6 && items.every(item => item.disabled && !item.active);
    }, "persistent disabled formatting icons", 10000);
  }
  async function click(name, expected) {
    await browser.evaluate(`${buttons}.find(element => element.getAttribute('aria-label').includes(${JSON.stringify(name)})).click()`);
    await waitFor(() => document.getText() === source.replace("café", expected), `toggle ${name}`, 10000);
  }
  try {
    await disabled();
    editor.selection = new vscode.Selection(end, start);
    await ready();
    await click("Emphasis", "[em]café[/em]");
    await ready(["Emphasis"]);
    await click("Italic", "[style=thought][em]café[/em][/style]");
    await ready(["Emphasis", "Italic"]);
    if (process.env.TUA_FORMATTING_SCREENSHOT) {
      const image = await browser.send("Page.captureScreenshot", { format: "png" });
      fs.writeFileSync(process.env.TUA_FORMATTING_SCREENSHOT, Buffer.from(image.data, "base64"));
    }
    await click("Emphasis", "[style=thought]café[/style]");
    await ready(["Italic"]);
    await vscode.commands.executeCommand("undo");
    assert.equal(document.getText(), source.replace("café", "[style=thought][em]café[/em][/style]"));
    await vscode.commands.executeCommand("redo");
    assert.equal(document.getText(), source.replace("café", "[style=thought]café[/style]"));
    const beforeWrongUri = document.getText();
    assert.equal(await vscode.commands.executeCommand("tua.dialogue.format.emphasis", vscode.Uri.file(mainPath)), false);
    assert.equal(document.getText(), beforeWrongUri, "another editor's title action cannot edit this selection");
    editor.selections = [new vscode.Selection(start, end), new vscode.Selection(0, 0, 0, 3)];
    await disabled();
    assert.equal(await vscode.commands.executeCommand("tua.dialogue.format.emphasis"), false);
    editor.selection = new vscode.Selection(0, 0, 0, 5);
    await disabled();
    const hovers = await vscode.commands.executeCommand("vscode.executeHoverProvider", document.uri, start);
    assert(!hovers.flatMap(hover => hover.contents).some(item => item.value?.includes("Format dialogue")));
    await vscode.workspace.getConfiguration("tua.dialogue").update("formattingToolbar", false, vscode.ConfigurationTarget.Workspace);
    await waitFor(async () => (await state()).length === 0, "toolbar setting hides title actions");
    await vscode.workspace.getConfiguration("tua.dialogue").update("formattingToolbar", true, vscode.ConfigurationTarget.Workspace);
    await disabled();
    const currentStart = document.positionAt(document.getText().indexOf("café"));
    editor.selection = new vscode.Selection(currentStart, currentStart.translate(0, 4));
    await ready(["Italic"]);
    await editor.edit(builder => builder.delete(new vscode.Range(1, 0, 2, 0)));
    const plainSource = document.getText();
    assert.equal(await vscode.commands.executeCommand("tua.dialogue.format.emphasis"), false);
    assert.equal(document.getText(), plainSource, "cached actions cannot apply after removing rich mode");
    await disabled();
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    console.log("REAL_SERVER_DIALOGUE_FORMATTING persistent icons, disabled/pressed states, independent toggles, undo/redo, wrong URI, multiple selections and toolbar setting passed");
  } catch (error) {
    error.message += `; document: ${document.getText()}; selection: ${JSON.stringify(editor.selection)}; toolbar state: ${JSON.stringify(await state())}`;
    throw error;
  } finally {
    browser.dispose();
  }
}

module.exports = { verifyDialogueFormatting };
