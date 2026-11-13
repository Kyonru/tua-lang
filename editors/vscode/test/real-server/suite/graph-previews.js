"use strict";

const assert = require("assert/strict");
const vscode = require("vscode");
const { BrowserSession, waitFor } = require("../../webview/cdp");

// This exercises the real server response, extension view model, rendered
// webview, and source action in one isolated project. The synthetic webview
// suite separately covers empty/error states and the detailed UI controls.
async function verifyGraphPreviews(sourcePath) {
  const document = await vscode.workspace.openTextDocument(sourcePath);
  const sourceUri = document.uri;
  const port = process.env.TUA_REAL_SERVER_CDP_PORT;
  assert(port, "the real-server harness must provide a debugging port");

  for (const preview of [
    { command: "tua.showEcsExplorer", title: "ECS Explorer", label: "Position",
      detail: "x", action: '#detail .detail-header button', source: 'local Position' },
    { command: "tua.showObjectPoolExplorer", title: "Object Pool Explorer", label: "bullets",
      detail: "acquire", action: '#detail .detail-header button', source: 'local bullets' },
    { command: "tua.showSignalGraph", title: "Signal Graph", label: "enemy.hit",
      detail: "emit", action: '.call-row.role-emit', source: 'EnemyEvents:emit' },
  ]) {
    await vscode.window.showTextDocument(document, { preview: false });
    await vscode.commands.executeCommand(preview.command);
    const { browser, context } = await previewContext(port, preview.title);
    try {
      const rendered = await browser.evaluate(`({
        text: document.body.textContent,
        action: !!document.querySelector(${JSON.stringify(preview.action)})
      })`, context);
      assert(rendered.text.includes(preview.label), `${preview.title} should show ${preview.label} from the real server`);
      assert(rendered.text.includes(preview.detail), `${preview.title} should show ${preview.detail} from the real server`);
      assert(rendered.action, `${preview.title} should render its source action`);
      const sourceEditor = vscode.window.visibleTextEditors.find(editor => editor.document.uri.toString() === sourceUri.toString());
      assert(sourceEditor, `${preview.title} should retain the source editor`);
      sourceEditor.selection = new vscode.Selection(document.lineCount - 1, 0, document.lineCount - 1, 0);
      const expected = document.positionAt(document.getText().indexOf(preview.source));
      assert.notEqual(sourceEditor.selection.active.line, expected.line, "source action must move the cursor");
      await browser.evaluate(`document.querySelector(${JSON.stringify(preview.action)}).click()`, context);
      await waitFor(() => {
        const editor = vscode.window.activeTextEditor;
        return editor?.document.uri.toString() === sourceUri.toString() &&
          editor.selection.active.line === expected.line;
      }, `${preview.title} source action at ${preview.source}`);
    } finally {
      browser.dispose();
    }
  }
  console.log("REAL_SERVER_GRAPH_PREVIEWS ECS, pool, and Signal render and navigate to source");
}

async function previewContext(port, title) {
  return waitFor(async () => {
    const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    for (const target of targets.filter(item => item.type === "iframe" && item.url.startsWith("vscode-webview:"))) {
      const browser = await BrowserSession.connect(target.webSocketDebuggerUrl);
      for (const context of [...browser.contexts.values()].reverse()) {
        try {
          if (await browser.evaluate("document.title", context.id) === title) {
            return { browser, context: context.id };
          }
        } catch { /* Replaced webview context. */ }
      }
      browser.dispose();
    }
  }, `rendered ${title} webview`, 15000);
}

module.exports = { previewContext, verifyGraphPreviews };
