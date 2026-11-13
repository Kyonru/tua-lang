"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { BrowserSession, waitFor } = require(path.resolve(__dirname, "../../../editors/vscode/test/webview/cdp"));

async function run() {
  const extension = vscode.extensions.getExtension("SolenodonteLabs.tua");
  assert(extension);
  const baseline = process.env.TUA_STARFALL_BASELINE === "1";
  assert.equal(process.env.TUA_COMPLETION_TRACE, "1", "trace environment should reach extension host");
  await extension.activate();
  if (!baseline) await waitFor(() => fs.existsSync(process.env.TUA_COMPLETION_TRACE_OUTPUT), "completion middleware trace", 10000);
  const port = Number(process.env.TUA_STARFALL_PORT);
  const target = await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${port}/json/list`);
    return (await response.json()).find(item => item.type === "page");
  }, "VS Code workbench", 15000);
  const browser = await BrowserSession.connect(target.webSocketDebuggerUrl);
  const report = { revision: process.env.TUA_TEST_REVISION || "current", vscode: vscode.version,
    project: "examples/starfall (temporary copy)", samples: [] };
  try {
    const [folder] = vscode.workspace.workspaceFolders;
    const document = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder.uri, "src/main.tua"));
    const editor = await vscode.window.showTextDocument(document, { preview: false });
    const occurrence = document.getText().indexOf("world.reset()");
    assert(occurrence >= 0);
    const start = document.positionAt(occurrence);
    const end = document.positionAt(occurrence + "world.reset()".length);
    await editor.edit(builder => builder.replace(new vscode.Range(start, end), "world"));
    const position = document.positionAt(occurrence + "world".length);
    assert.equal(document.lineAt(position.line).text.slice(position.character - 5, position.character), "world");
    for (let index = 0; index < 12; index++) {
      await browser.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
      await browser.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
      if (document.lineAt(position.line).text.charAt(position.character) === ".") {
        await editor.edit(builder => builder.delete(new vscode.Range(position, position.translate(0, 1))));
      }
      editor.selection = new vscode.Selection(position, position);
      await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
      await waitFor(() => browser.evaluate("Boolean(document.querySelector('.monaco-editor.focused .native-edit-context'))"), "focused editor", 10000);
      await browser.evaluate(`(() => {
        const e = document.querySelector('.monaco-editor.focused .native-edit-context');
        e.focus();
        window.__tuaStarfall = { trigger: performance.timeOrigin + performance.now(), firstVisible: null, paint: null, labels: [] };
        const check = () => {
          const w = document.querySelector('.suggest-widget');
          if (!w || !w.classList.contains('visible') || getComputedStyle(w).display === 'none') return;
          window.__tuaStarfall.firstVisible ??= performance.timeOrigin + performance.now();
          const labels = [...w.querySelectorAll('.label-name')].map(x => x.textContent.trim());
          const required = ${JSON.stringify(baseline ? ["reset", "update", "draw"] : ["width", "height", "reset", "update", "draw"])};
          if (!required.every(label => labels.includes(label))) return;
          window.__tuaStarfall.paint = performance.timeOrigin + performance.now();
          window.__tuaStarfall.labels = labels;
          window.__tuaStarfall.observer.disconnect();
        };
        window.__tuaStarfall.observer = new MutationObserver(check);
        window.__tuaStarfall.observer.observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
      })()`);
      await browser.send("Input.insertText", { text: "." });
      let widget;
      try {
        widget = await waitFor(() => browser.evaluate("window.__tuaStarfall?.paint ? {trigger:window.__tuaStarfall.trigger,firstVisible:window.__tuaStarfall.firstVisible,paint:window.__tuaStarfall.paint,labels:window.__tuaStarfall.labels} : null"), "Starfall member suggestions", 10000);
      } catch (error) {
        const state = await browser.evaluate(`(() => { const w=document.querySelector('.suggest-widget'); return {className:w?.className,labels:[...(w?.querySelectorAll('.label-name')||[])].map(x=>x.textContent.trim()).slice(0,40),focused:document.activeElement?.className}; })()`);
        throw new Error(`${error.message}: ${JSON.stringify({ state, trace: fs.readFileSync(process.env.TUA_COMPLETION_TRACE_OUTPUT, "utf8").split("\n").slice(-4), line: document.lineAt(position.line).text })}`);
      }
      const trace = baseline ? undefined : fs.readFileSync(process.env.TUA_COMPLETION_TRACE_OUTPUT, "utf8").trim().split("\n").map(line => JSON.parse(line))
        .filter(item => item.uri === document.uri.toString() && item.line === position.line &&
          item.started >= widget.trigger - 5 && item.finished <= widget.paint + 5).at(-1);
      if (!baseline) assert(trace, "completion middleware trace should be present");
      report.samples.push({
        ...(trace ? {
          triggerToProviderMs: trace.started - widget.trigger,
          providerToResponseMs: trace.finished - trace.started,
          responseToPaintMs: widget.paint - trace.finished,
        } : {}),
        totalMs: widget.paint - widget.trigger,
        firstVisibleMs: widget.firstVisible - widget.trigger,
        candidateCount: widget.labels.length,
      });
    }
    const completion = await vscode.commands.executeCommand("vscode.executeCompletionItemProvider",
      document.uri, position.translate(0, 1), ".");
    report.providerLabels = completion.items.map(item => item.label);
    for (const field of baseline ? ["reset", "update", "draw"] : ["width", "height", "reset", "update", "draw"]) {
      assert(report.providerLabels.some(item => item.label === field), `${field} should be in the provider result`);
    }
    const widthUse = document.getText().indexOf("world.width");
    const widthHover = await vscode.commands.executeCommand("vscode.executeHoverProvider",
      document.uri, document.positionAt(widthUse + "world.w".length));
    report.widthHover = widthHover.map(hover => hover.contents.map(content =>
      typeof content === "string" ? content : content.value).join("\n"));
    fs.writeFileSync(process.env.TUA_STARFALL_OUT, JSON.stringify(report, null, 2) + "\n");
    console.log("STARFALL_VISIBLE", JSON.stringify(report));
  } finally {
    browser.dispose();
  }
}

module.exports = { run };
