"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { BrowserSession, waitFor } = require("../../webview/cdp");

async function verifyDialogueFormatting(mainPath) {
  const file = path.join(path.dirname(mainPath), "formatting.tdialogue");
  const source = "@dialogue 1\r\n@text rich\r\n[start]\r\n: 🐸 café %{name} (end)\r\n";
  fs.writeFileSync(file, source);
  const document = await vscode.workspace.openTextDocument(file);
  const editor = await vscode.window.showTextDocument(document, { preview: false });
  const start = document.positionAt(source.indexOf("café"));
  const end = start.translate(0, 4);
  editor.selection = new vscode.Selection(start, end);
  let actions;
  await waitFor(async () => {
    actions = await vscode.commands.executeCommand("vscode.executeCodeActionProvider", document.uri,
      editor.selection, "refactor.rewrite.dialogueText");
    return actions?.length === 6;
  }, "dialogue formatting actions from the real compiler", 15000);
  async function toolbar() {
    let content;
    await waitFor(async () => {
      const hovers = await vscode.commands.executeCommand("vscode.executeHoverProvider", document.uri, start);
      content = hovers.flatMap(hover => hover.contents).find(item => item.value?.includes("Format dialogue"));
      return content;
    }, "selected text formatting hover actions", 10000);
    assert.deepEqual(content.isTrusted, { enabledCommands: ["tua.dialogue.applyTextFormat"] });
    const command = content.value.match(/command:tua\.dialogue\.applyTextFormat\?([^)]*)/);
    assert(command);
    return JSON.parse(decodeURIComponent(command[1]));
  }
  await toolbar();
  // Inspect the actual native hover, rather than only provider metadata.
  const targets = await (await fetch(`http://127.0.0.1:${process.env.TUA_REAL_SERVER_CDP_PORT}/json/list`)).json();
  const page = targets.find(target => target.type === "page");
  assert(page, "native editor debugger target");
  const browser = await BrowserSession.connect(page.webSocketDebuggerUrl);
  try {
    await vscode.commands.executeCommand("editor.action.showHover", { focus: "noAutoFocus" });
    await waitFor(() => browser.evaluate(
      "[...document.querySelectorAll('.monaco-hover')].some(element => element.innerText.includes('Format dialogue'))",
    ), "visible native dialogue toolbar", 10000);
    assert(editor.selection.isEqual(new vscode.Selection(start, end)), "showing hover preserves selection");
    await new Promise(resolve => setTimeout(resolve, 300));
    if (process.env.TUA_FORMATTING_SCREENSHOT) {
      const image = await browser.send("Page.captureScreenshot", { format: "png" });
      fs.writeFileSync(process.env.TUA_FORMATTING_SCREENSHOT, Buffer.from(image.data, "base64"));
    }
    await browser.evaluate(
      "[...document.querySelectorAll('.monaco-hover a')].find(link => link.textContent === 'Emphasis').click()",
    );
    await waitFor(() => document.getText().includes("[em]café[/em]"), "clicking native formatting action");
  } finally {
    browser.dispose();
  }
  assert.equal(document.getText(), source.replace("café", "[em]café[/em]"));
  await vscode.commands.executeCommand("undo");
  assert.equal(document.getText(), source, "formatting must undo in one step");
  editor.selection = new vscode.Selection(end, start);
  const stale = await toolbar();
  await editor.edit(builder => builder.insert(new vscode.Position(0, 0), "// edit\r\n"));
  const changed = document.getText();
  assert.equal(await vscode.commands.executeCommand("tua.dialogue.applyTextFormat", ...stale), false);
  assert.equal(document.getText(), changed, "stale hover must never edit changed source");
  await vscode.commands.executeCommand("undo");
  editor.selections = [new vscode.Selection(start, end), new vscode.Selection(0, 0, 0, 3)];
  const hovers = await vscode.commands.executeCommand("vscode.executeHoverProvider", document.uri, start);
  assert(!hovers.flatMap(hover => hover.contents).some(item => item.value?.includes("Format dialogue")));
  await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
  console.log("REAL_SERVER_DIALOGUE_FORMATTING compiler actions, trusted hover, undo, stale edits and multiple selections passed");
}

module.exports = { verifyDialogueFormatting };
