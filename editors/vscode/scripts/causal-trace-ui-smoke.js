"use strict";
// Real editor tree + real LÖVE transport in a disposable user profile/workspace.
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const assert = require("node:assert/strict");
const extension = path.resolve(__dirname, "..");
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

exports.run = async () => {
  const vscode = require("vscode");
  const { BrowserSession, waitFor } = require("../test/webview/cdp");
  await vscode.extensions.getExtension("SolenodonteLabs.tua").activate();
  const targets = await (await fetch(`http://127.0.0.1:${process.env.TUA_CAUSAL_PORT}/json/list`)).json();
  const browser = await BrowserSession.connect(targets.find(target => target.type === "page").webSocketDebuggerUrl);
  const command = (name, ...args) => vscode.commands.executeCommand(name, ...args);
  const has = text => browser.evaluate(`document.body.innerText.includes(${JSON.stringify(text)})`);
  const expand = async label => {
    await waitFor(() => has(label), label);
    await browser.evaluate(`(() => {
      const row = [...document.querySelectorAll('.monaco-list-row[aria-expanded="false"]')]
        .find(node => node.textContent.trim().startsWith(${JSON.stringify(label)}));
      row?.querySelector('.monaco-tl-twistie')?.click();
    })()`);
    await sleep(250);
  };
  try {
    await browser.send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 960, deviceScaleFactor: 1, mobile: false });
    const root = vscode.workspace.workspaceFolders[0].uri.fsPath;
    const document = await vscode.workspace.openTextDocument(path.join(root, "src/main.tua"));
    await vscode.window.showTextDocument(document);
    await waitFor(async () => (await command("vscode.executeDocumentSymbolProvider", document.uri))?.length > 0, "compiler ready", 30000);
    await command("tua.runLoveProject");
    await command("tua.inspector.focus");
    await waitFor(() => has("Live"), "runtime connected", 45000);
    await command("workbench.action.closePanel");
    await command("workbench.action.closeAuxiliaryBar");
    await browser.evaluate(`(() => {
      for (const header of document.querySelectorAll('.pane-header')) {
        if (['Run Configuration', 'Project', 'Tua Assets', 'Project Checks'].some(label => header.textContent.trim().startsWith(label))
          && header.getAttribute('aria-expanded') !== 'false') header.click();
      }
    })()`);
    await sleep(300);
    for (const label of ["causal", "Causal trace", "Root #10", "Attack #10", "Attack #11"]) await expand(label);
    await waitFor(() => has("suppressed: root_limit"), "suppression evidence");
    assert(await has("Stat provenance"));
    await command("tua.inspector.pause");
    await waitFor(() => has("Updates paused"), "paused status");
    await command("tua.inspector.refresh");
    await command("tua.inspector.resume");
    await waitFor(() => has("Live"), "resumed status");
    await command("notifications.clearAll");
    await browser.send("Page.bringToFront");
    await sleep(500);
    const shot = await browser.send("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(process.env.TUA_CAUSAL_SCREENSHOT, Buffer.from(shot.data, "base64"));
    await command("tua.stopLoveProject");
    await waitFor(() => has("Ended"), "retained ended snapshot");
    assert(await has("suppressed: root_limit"));
    console.log("Causal trace native editor view passed: live ancestry, suppression, provenance, pause/refresh/resume and ended snapshot.");
  } catch (error) {
    console.log(await browser.evaluate("document.body.innerText"));
    const shot = await browser.send("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(process.env.TUA_CAUSAL_SCREENSHOT, Buffer.from(shot.data, "base64"));
    throw error;
  } finally {
    await command("tua.stopLoveProject");
    browser.dispose();
  }
};

async function main() {
  const executable = require("../test/vscode-executable").localVSCodeExecutable();
  if (!executable || !fs.existsSync(executable)) throw new Error("A local graphical VS Code host is required");
  const compiler = process.env.TUA_BIN || path.join(extension, "bin/tua");
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-causal-ui-"));
  try {
    const project = path.join(temp, "project");
    fs.mkdirSync(path.join(project, "src"), {recursive: true});
    fs.writeFileSync(path.join(project, "tua.toml"), 'source = "src"\nout = "build"\ntarget = "love11-luajit"\n');
    fs.writeFileSync(path.join(project, "src/conf.tua"), 'function love.conf(t: any)\n t.window.visible = false\n t.modules.audio = false\nend\n');
    fs.writeFileSync(path.join(project, "src/main.tua"), `local causal = store({
  inspectionKind = "causal-trace/v1", epoch = 1, enabled = true, overwritten = 0,
  statRootId = 10, weaponId = "example", stats = { "damage: base 2 -> 3" },
  events = {
    { sequence = 1, attackId = 10, parentId = 0, rootId = 10, phase = "processing", reason = "primary" },
    { sequence = 2, attackId = 11, parentId = 10, rootId = 10, phase = "suppressed", reason = "root_limit" }
  }
})
function love.update(dt: number)
  assert(causal:get().enabled)
end
`);
    const user = path.join(temp, "user");
    fs.mkdirSync(path.join(user, "User"), {recursive: true});
    fs.writeFileSync(path.join(user, "User/settings.json"), JSON.stringify({
      "workbench.colorTheme": "Indigo Carbon HC", "workbench.startupEditor": "none",
      "telemetry.telemetryLevel": "off", "tua.runtimeInspector.enabled": true,
      "tua.server.path": compiler,
    }));
    const server = require("node:net").createServer();
    await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
    const port = server.address().port;
    await new Promise(resolve => server.close(resolve));
    await require("@vscode/test-electron").runTests({
      extensionDevelopmentPath: extension, extensionTestsPath: __filename, vscodeExecutablePath: executable,
      extensionTestsEnv: { TUA_CAUSAL_PORT: String(port), TUA_CAUSAL_SCREENSHOT: process.env.TUA_CAUSAL_SCREENSHOT || path.join(os.tmpdir(), "tua-causal-view.png") },
      launchArgs: [project, "--disable-extensions", "--disable-workspace-trust", "--skip-welcome", "--skip-release-notes",
        `--user-data-dir=${user}`, `--extensions-dir=${temp}/extensions`, `--remote-debugging-port=${port}`],
    });
  } finally { fs.rmSync(temp, {recursive: true, force: true}); }
}
if (require.main === module) main().catch(error => { console.error(error); process.exitCode = 1; });
