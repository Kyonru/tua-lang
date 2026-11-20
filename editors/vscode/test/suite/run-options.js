"use strict";

const assert = require("node:assert/strict");
const path = require("node:path");
const vscode = require("vscode");
const { BrowserSession, waitFor } = require("../webview/cdp");
const { fixtures } = require("./support");

async function runRunOptionsTests() {
  const targets = await (await fetch(`http://127.0.0.1:${process.env.TUA_TEST_CDP_PORT}/json/list`)).json();
  const browser = await BrowserSession.connect(targets.find((target) => target.type === "page").webSocketDebuggerUrl);
  const visibleOptions = () => browser.evaluate(`(() => {
    const text = document.body.innerText;
    return text.toLowerCase().includes("tua love game") && text.includes("Development") && text.includes("Hot reload");
  })()`);
  try {
    await vscode.commands.executeCommand("tua.showRunOptions");
    await waitFor(visibleOptions, "Tua configurations to appear in Run and Debug");
    assert.equal(await visibleOptions(), true);
    const collapsed = await browser.evaluate(`(() => {
      const header = [...document.querySelectorAll(".pane-header")]
        .find((element) => element.textContent.toLowerCase().includes("tua love game"));
      if (!header) return false;
      header.click();
      return true;
    })()`);
    assert.equal(collapsed, true, "the Tua Run and Debug section should have a collapsible header");
    await waitFor(async () => !(await visibleOptions()), "Tua run options to collapse");
    await vscode.commands.executeCommand("tua.showRunOptions");
    await waitFor(visibleOptions, "Tua run options to reappear after focusing a collapsed view");
  } finally {
    browser.dispose();
  }
  await runTerminalReuseTests();
}

async function runTerminalReuseTests() {
  const command = vscode.workspace.getConfiguration("tua");
  const previousLoveCommand = command.inspect("love.command").globalValue;
  const opened = [];
  const listener = vscode.window.onDidOpenTerminal(terminal => opened.push(terminal));
  try {
    await command.update("love.command", path.join(fixtures.workspaceOne, "missing-love"), vscode.ConfigurationTarget.Global);
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(fixtures.suggestions));
    const run = async presentation => {
      let dispose;
      let timer;
      const ended = new Promise((resolve, reject) => {
        timer = setTimeout(() => { dispose?.dispose(); reject(new Error("Tua run task did not finish")); }, 30000);
        dispose = vscode.tasks.onDidEndTask(event => {
          if (event.execution.task.definition?.command !== "runLoveProject") return;
          clearTimeout(timer);
          dispose.dispose();
          resolve();
        });
      });
      try {
        await vscode.commands.executeCommand("tua.runLoveProject", presentation);
        await ended;
      } finally {
        clearTimeout(timer);
        dispose?.dispose();
      }
    };
    await run();
    assert.equal(opened.length, 1, "the first run should open one task terminal");
    await run();
    assert.equal(opened.length, 1, "a finished run should reuse its dedicated terminal");
    await run({ configurationName: "Another configuration" });
    assert.equal(opened.length, 2, "a changed run configuration may open a new terminal");
  } finally {
    listener.dispose();
    for (const terminal of opened) terminal.dispose();
    await command.update("love.command", previousLoveCommand, vscode.ConfigurationTarget.Global);
  }
}

module.exports = { runRunOptionsTests };
