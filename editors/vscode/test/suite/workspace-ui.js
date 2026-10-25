"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { BrowserSession, waitFor } = require("../webview/cdp");
const { fixtures, openFixture } = require("./support");

async function runWorkspaceUiTests() {
  await vscode.workspace.getConfiguration("tua").update("love.command", fixtures.mockLove, vscode.ConfigurationTarget.Workspace);
  await vscode.workspace.getConfiguration("workbench").update("editor.editorActionsLocation", "titleBar", vscode.ConfigurationTarget.Global);
  await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Indigo Carbon HC", vscode.ConfigurationTarget.Global);
  const { document, editor } = await openFixture("intelligence");
  const targets = await (await fetch(`http://127.0.0.1:${process.env.TUA_TEST_CDP_PORT}/json/list`)).json();
  const browser = await BrowserSession.connect(targets.find(item => item.type === "page").webSocketDebuggerUrl);
  const text = () => browser.evaluate("document.body.innerText");
  const clickToolbar = async label => {
    const clicked = await browser.evaluate(`(() => {
      const button = [...document.querySelectorAll('.part.titlebar .action-label, .part.editor .editor-actions .action-label')]
        .find(item => (item.getAttribute('aria-label') || item.title || '').includes(${JSON.stringify(label)}));
      button?.click(); return Boolean(button);
    })()`);
    assert(clicked, `Toolbar action must be present: ${label}`);
  };
  const runningMarker = path.join(fixtures.workspaceOne, ".tua-test-keep-running");
  const processMarker = path.join(fixtures.workspaceOne, ".tua-test-process.pid");
  const screenshot = async name => {
    if (!process.env.TUA_WORKSPACE_SCREENSHOTS) return;
    fs.mkdirSync(process.env.TUA_WORKSPACE_SCREENSHOTS, { recursive: true });
    const result = await browser.send("Page.captureScreenshot");
    fs.writeFileSync(path.join(process.env.TUA_WORKSPACE_SCREENSHOTS, `${name}.png`), Buffer.from(result.data, "base64"));
  };
  try {
    await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
    await vscode.commands.executeCommand("notifications.clearAll");
    await vscode.commands.executeCommand("tua.workspace.open");
    await clickToolbar("Select Run Configuration");
    await waitFor(() => browser.evaluate("[...document.querySelectorAll('.quick-input-widget')].some(item => item.getBoundingClientRect().height > 0 && item.innerText.includes('Hot reload'))"), "toolbar run picker");
    await screenshot("configuration-picker");
    await browser.send("Input.insertText", { text: "Hot reload" });
    await browser.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
    await browser.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
    await waitFor(async () => (await text()).includes("Hot reload"), "selected run configuration");
    await vscode.commands.executeCommand("tua.workspace.focus");
    await waitFor(() => browser.evaluate("[...document.querySelectorAll('.monaco-list-row')].some(row => row.innerText.includes('Modules'))"), "workspace project groups");
    await browser.evaluate(`(() => {
      const row = [...document.querySelectorAll('.monaco-list-row')].find(row => row.innerText.trim() === 'Modules');
      row?.querySelector('.monaco-tl-twistie')?.click();
    })()`);
    await waitFor(async () => (await text()).includes("module_graph_player.tua"), "compiler-backed module entries");

    const checks = await vscode.commands.executeCommand("tua.checks.run");
    assert.equal(checks.find(row => row.id === "compiler").ok, true);
    assert.equal(checks.find(row => row.id === "love").ok, true);
    assert.equal(checks.find(row => row.id === "maps").ok, true);

    const range = new vscode.Range(0, 0, 6, document.lineAt(6).text.length);
    await vscode.commands.executeCommand("tua.inlayHints.selectMode", "full");
    const full = await vscode.commands.executeCommand("vscode.executeInlayHintProvider", document.uri, range);
    assert(full.length > 0, "fixture has compiler hints");
    await vscode.commands.executeCommand("tua.inlayHints.selectMode", "contextual");
    const quiet = await vscode.commands.executeCommand("vscode.executeInlayHintProvider", document.uri, range);
    assert.equal(quiet.length, 0);
    const line = full[0].position.line;
    editor.selection = new vscode.Selection(line, 0, line, document.lineAt(line).text.length);
    const inspected = await vscode.commands.executeCommand("vscode.executeInlayHintProvider", document.uri, range);
    assert(inspected.length > 0 && inspected.every(hint => hint.position.line === line));

    await vscode.commands.executeCommand("tua.run.selectConfiguration", { name: "Development" });
    await vscode.commands.executeCommand("tua.run.selected");
    await waitFor(async () => (await text()).includes("Exited with errors"), "runtime exit status");
    await waitFor(async () => (await text()).includes("low health"), "captured game output");
    await waitFor(async () => (await text()).includes("×2"), "collapsed consecutive messages");
    await waitFor(async () => (await text()).includes("Exited with errors"), "runtime exit status");
    await screenshot("workspace-dark");
    await browser.evaluate(`(() => {
      const row = [...document.querySelectorAll('.monaco-list-row')].find(row => row.innerText.includes('fixture failure'));
      row?.querySelector('.monaco-tl-contents')?.click();
    })()`);
    await waitFor(() => vscode.window.activeTextEditor?.selection.active.line === 6, "runtime error source navigation");
    await vscode.window.showTextDocument(document);
    fs.writeFileSync(runningMarker, "keep fixture alive until Stop\n");
    await clickToolbar("Run Selected Configuration");
    await waitFor(() => fs.existsSync(processMarker), "running fixture process");
    const processId = Number(fs.readFileSync(processMarker, "utf8"));
    await waitFor(async () => (await text()).includes("Tua: Running"), "active run status");
    await screenshot("toolbar-running-dark");
    for (const [theme, name] of [["Light Modern", "workspace-light"], ["Default High Contrast", "workspace-hc"]]) {
      await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
      await new Promise(resolve => setTimeout(resolve, 200));
      await screenshot(name);
    }
    await clickToolbar("Stop Session");
    await waitFor(async () => (await text()).includes("Tua: Stopped"), "toolbar Stop status");
    await waitFor(() => {
      try { process.kill(processId, 0); return false; } catch (error) { return error.code === "ESRCH"; }
    }, "Stop terminates the game process");
    assert.deepEqual(browser.errors, [], "workspace UI must not throw renderer exceptions");
    console.log("Tua workspace UI passed: sidebar modules, toolbar picker, process termination through Stop, tool checks, contextual hints, console repeat counts, status, and source navigation.");
  } catch (error) {
    await screenshot("workspace-failure");
    console.log((await text()).slice(-7000));
    throw error;
  } finally {
    fs.rmSync(runningMarker, { force: true });
    await vscode.commands.executeCommand("tua.activity.stop");
    browser.dispose();
  }
}

module.exports = { runWorkspaceUiTests };
