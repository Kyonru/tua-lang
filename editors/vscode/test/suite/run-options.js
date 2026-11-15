"use strict";

const assert = require("node:assert/strict");
const vscode = require("vscode");
const { BrowserSession, waitFor } = require("../webview/cdp");

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
}

module.exports = { runRunOptionsTests };
