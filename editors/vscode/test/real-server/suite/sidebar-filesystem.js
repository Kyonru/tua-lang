"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { BrowserSession, waitFor } = require("../../webview/cdp");

async function run() {
  const extension = vscode.extensions.getExtension("SolenodonteLabs.tua");
  assert(extension);
  await extension.activate();
  const [folder] = vscode.workspace.workspaceFolders;
  assert(folder);
  const source = path.join(folder.uri.fsPath, "src");
  const mainPath = path.join(source, "main.tua");
  const helperPath = path.join(source, "helper.lua");
  const extraPath = path.join(source, "extra.tua");
  const addedPath = path.join(source, "added.lua");
  const evidence = { vscode: vscode.version, steps: [] };
  const record = (step, rows) => evidence.steps.push({ step, rows });
  const port = Number(process.env.TUA_SIDEBAR_CDP_PORT);
  const page = await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${port}/json/list`);
    return (await response.json()).find(item => item.type === "page");
  }, "VS Code workbench", 15000);
  const browser = await BrowserSession.connect(page.webSocketDebuggerUrl);
  const rows = () => browser.evaluate("[...document.querySelectorAll('.monaco-list-row')].map(row => row.innerText.trim())");
  const rowExists = async predicate => (await rows()).some(predicate);
  const expand = async label => {
    const expanded = await browser.evaluate(`(() => {
      const row=[...document.querySelectorAll('.monaco-list-row')]
        .find(item => item.innerText.trim() === ${JSON.stringify(label)});
      if (!row) return false;
      if (row.getAttribute('aria-expanded') !== 'true') row.querySelector('.monaco-tl-twistie')?.click();
      return true;
    })()`);
    assert(expanded, `sidebar row missing: ${label}; rows: ${JSON.stringify(await rows())}`);
  };
  try {
    await waitFor(async () => {
      await vscode.commands.executeCommand("tua.copyDebugInfo");
      return (await vscode.env.clipboard.readText()).includes("Server state: running");
    }, "real Tua server", 15000);
    await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
    await vscode.commands.executeCommand("tua.workspace.focus");
    await waitFor(() => rowExists(row => row === "Project files"), "Project files group", 10000);
    await expand("Project files");
    await waitFor(() => rowExists(row => row === folder.name), "workspace folder");
    await expand(folder.name);
    await waitFor(() => rowExists(row => row === "src"), "source directory");
    await expand("src");
    await waitFor(() => rowExists(row => row === "helper.lua"), "Lua source in Project files");
    record("initial-project-files", await rows());

    fs.writeFileSync(addedPath, "return { added = true }\n");
    await waitFor(() => rowExists(row => row === "added.lua"), "external Lua file create in Project files", 10000);
    record("created-lua", await rows());
    fs.unlinkSync(addedPath);
    await waitFor(async () => !(await rowExists(row => row === "added.lua")), "external Lua file delete in Project files", 10000);
    record("deleted-lua", await rows());

    await expand("Modules");
    await waitFor(() => rowExists(row => row.includes("main.tua") && row.includes("1 dependencies")),
      "initial module dependency count", 10000);
    record("initial-modules", await rows());

    const main = await vscode.workspace.openTextDocument(mainPath);
    const editor = await vscode.window.showTextDocument(main, { preview: false });
    await editor.edit(builder => builder.insert(new vscode.Position(1, 0), 'local extra = require "extra"\n'));
    assert(main.isDirty, "the changed module must remain unsaved");
    await waitFor(() => rowExists(row => row.includes("main.tua") && row.includes("2 dependencies")),
      "unsaved module dependency update", 10000);
    record("unsaved-module-edit", await rows());

    await editor.edit(builder => builder.delete(main.lineAt(1).rangeIncludingLineBreak));
    await waitFor(() => rowExists(row => row.includes("main.tua") && row.includes("1 dependencies")),
      "unsaved dependency removal", 10000);
    record("unsaved-module-recovery", await rows());

    fs.writeFileSync(helperPath, "return { fresh = 2 }\n");
    const freshExportVisible = async () => {
      const visible = await rowExists(row => row.includes("helper.lua") && row.includes("dependencies"));
      if (!visible) return false;
      const opened = await browser.evaluate(`(() => {
        const row=[...document.querySelectorAll('.monaco-list-row')]
          .find(item => item.innerText.includes('helper.lua') && item.innerText.includes('dependencies'));
        if (!row) return false;
        if (row.getAttribute('aria-expanded') !== 'true') row.querySelector('.monaco-tl-twistie')?.click();
        return true;
      })()`);
      return opened && await rowExists(row => row === "fresh");
    };
    try {
      await waitFor(freshExportVisible, "external Lua export change in Modules", 10000);
    } catch (error) {
      evidence.autoChangeError = error.message;
      await vscode.commands.executeCommand("tua.workspace.refresh");
      evidence.manualRefreshReachedFreshExport = await waitFor(freshExportVisible,
        "external Lua export after manual refresh", 5000).then(() => true, () => false);
      throw error;
    }
    record("changed-lua-export", await rows());

    fs.unlinkSync(extraPath);
    await waitFor(async () => !(await rowExists(row => row.includes("extra.tua") && row.includes("dependencies"))),
      "deleted Tua module removed from Modules", 10000);
    record("deleted-module", await rows());
    assert.equal(browser.errors.length, 0, "workbench renderer exceptions");
    console.log("REAL_SERVER_SIDEBAR", JSON.stringify(evidence.steps.map(step => step.step)));
  } catch (error) {
    evidence.error = error.message;
    evidence.finalRows = await rows().catch(() => []);
    throw error;
  } finally {
    fs.writeFileSync(process.env.TUA_SIDEBAR_EVIDENCE, JSON.stringify(evidence, null, 2) + "\n");
    browser.dispose();
  }
}

module.exports = { run };
