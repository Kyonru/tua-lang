"use strict";
// Capture actual extension commands in a disposable profile and example workspace.
const fs = require("node:fs"), path = require("node:path"), os = require("node:os");
const extension = path.resolve(__dirname, "..");
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
exports.run = async () => {
  const vscode = require("vscode");
  const { BrowserSession, waitFor } = require("../test/webview/cdp");
  const root = vscode.workspace.workspaceFolders[0].uri.fsPath;
  await vscode.extensions.getExtension("SolenodonteLabs.tua").activate();
  const targets = await (await fetch(`http://127.0.0.1:${process.env.TUA_CAPTURE_PORT}/json/list`)).json();
  const browser = await BrowserSession.connect(targets.find(t => t.type === "page").webSocketDebuggerUrl);
  const output = process.env.TUA_CAPTURE_OUTPUT;
  fs.mkdirSync(output, { recursive: true });
  const expandRow = async label => {
    await browser.evaluate(`(() => {
      const row = [...document.querySelectorAll('.monaco-list-row[aria-expanded="false"]')].find(node => node.textContent.trim().startsWith(${JSON.stringify(label)}));
      row?.querySelector('.monaco-tl-twistie')?.click();
    })()`);
    await sleep(350);
  };
  const command = (name, ...args) => vscode.commands.executeCommand(name, ...args);
  const open = async name => {
    const doc = await vscode.workspace.openTextDocument(path.join(root, "src", name));
    return vscode.window.showTextDocument(doc, { preview: false, viewColumn: vscode.ViewColumn.One });
  };
  const capture = async name => {
    await command("notifications.clearAll");
    await browser.send("Page.bringToFront"); await sleep(1600);
    const shot = await browser.send("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(path.join(output, name + ".png"), Buffer.from(shot.data, "base64"));
    console.log("Captured " + name);
  };
  const webviewReady = async title => {
    const target = await waitFor(async () => {
      const list = await (await fetch(`http://127.0.0.1:${process.env.TUA_CAPTURE_PORT}/json/list`)).json();
      return list.find(t => t.type === "iframe" && t.url.startsWith("vscode-webview:"));
    }, title);
    const web = await BrowserSession.connect(target.webSocketDebuggerUrl);
    try {
      const context = await web.documentContext(title);
      const selector = title === "Tua State Machine Preview" ? ".state-node" : title === "Tua Sprite Preview" ? ".frame-chip" : ".module-node";
      await waitFor(() => web.evaluate(`document.querySelectorAll(${JSON.stringify(selector)}).length > 0`, context), title + " populated");
      if (web.errors.length) throw new Error(JSON.stringify(web.errors));
    } finally { web.dispose(); }
  };
  try {
    await browser.send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 960, deviceScaleFactor: 1, mobile: false });
    await command("workbench.action.closeAllEditors");
    await command("workbench.action.closePanel");
    await command("workbench.action.closeAuxiliaryBar");
    const editor = await open("main.tua");
    await waitFor(async () => (await command("vscode.executeDocumentSymbolProvider", editor.document.uri))?.length > 0, "compiler symbols", 30000);
    await command("tua.workspace.open"); await sleep(1500);
    for (const label of ["Project files", "Starfall", "src", "Images", "Used"]) await expandRow(label);
    await capture("workspace");
    await command("workbench.action.toggleSidebarVisibility");
    await command("tua.previewStateMachine");
    await webviewReady("Tua State Machine Preview");
    await command("workbench.action.joinAllGroups");
    await capture("state-machine");
    await command("workbench.action.closeAllEditors");
    await open("sprites.tua");
    await command("tua.previewSprites");
    await webviewReady("Tua Sprite Preview");
    await command("workbench.action.joinAllGroups");
    await capture("sprite-animation");
    await command("workbench.action.closeAllEditors");
    await open("main.tua");
    await command("tua.showModuleGraph");
    await webviewReady("Module Graph");
    await command("workbench.action.joinAllGroups");
    await capture("module-graph");
    await command("workbench.action.closeAllEditors");
    await open("main.tua");
    await command("tua.workspace.open");
    await command("tua.runLoveProject");
    await command("tua.inspector.focus");
    await waitFor(() => browser.evaluate("document.body.innerText.includes('Live') && document.body.innerText.includes('State machines')"), "live runtime", 45000);
    await command("workbench.action.closePanel");
    await browser.evaluate(`(() => {
      for (const header of document.querySelectorAll('.pane-header')) {
        if (!header.textContent.includes('Project Checks') && ['Run Configuration', 'Project', 'Tua Assets'].some(label => header.textContent.trim().startsWith(label))) header.click();
      }
    })()`);
    await sleep(500);
    for (const label of ["scene", "pools", "playerState"]) await expandRow(label);
    await capture("runtime-inspector");
  } finally {
    await command("tua.stopLoveProject");
    browser.dispose();
  }
};
async function main() {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-readme-"));
  const project = path.join(temp, "Starfall");
  fs.mkdirSync(project);
  fs.cpSync(path.resolve(extension, "../../examples/starfall/src"), path.join(project, "src"), { recursive: true });
  fs.copyFileSync(path.resolve(extension, "../../examples/starfall/tua.toml"), path.join(project, "tua.toml"));
  fs.mkdirSync(path.join(project, "src/assets"));
  fs.copyFileSync(path.resolve(extension, "../../examples/features/src/assets/player.png"), path.join(project, "src/assets/player.png"));
  fs.writeFileSync(path.join(project, "src/sprites.tua"), `-- Demo sprite sheet: preview playback without starting the game.\nlocal Player = spriteSheet({\n  image = "assets/player.png",\n  frame = { width = 128, height = 128 },\n  animations = {\n    idle = { from = 0, to = 2, row = 0, fps = 8 },\n    walk = { from = 3, to = 6, row = 0, fps = 12 },\n  },\n})\nreturn Player\n`);
  fs.writeFileSync(path.join(project, "src/inspector_demo.tua"), 'local playerState = store({ health = 100, energy = 75, level = 3, weapon = "Pulse blaster" })\nreturn playerState\n');
  const mainFile = path.join(project, "src/main.tua");
  fs.writeFileSync(mainFile, 'require("inspector_demo")\n' + fs.readFileSync(mainFile, "utf8"));
  // Only the disposable game's configuration is adjusted, including save identity.
  fs.appendFileSync(path.join(project, "src/conf.tua"), '\nlocal configure = love.conf\nfunction love.conf(t: any)\n  configure(t)\n  t.identity = "tua-readme-capture"\n  t.window.visible = false\nend\n');
  const user = path.join(temp, "user"); fs.mkdirSync(path.join(user, "User"), { recursive: true });
  fs.writeFileSync(path.join(user, "User/settings.json"), JSON.stringify({
    "workbench.colorTheme": "Indigo Carbon HC", "workbench.iconTheme": "tua-icons",
    "workbench.startupEditor": "none", "window.commandCenter": true,
    "editor.fontSize": 15, "editor.minimap.enabled": false, "editor.stickyScroll.enabled": false,
    "workbench.editor.editorActionsLocation": "titleBar", "workbench.layoutControl.enabled": true,
    "telemetry.telemetryLevel": "off", "tua.preview.enabled": true,
    "tua.runtimeInspector.enabled": true, "tua.server.path": path.join(extension, "bin/tua"),
  }, null, 2));
  const net = require("node:net"), server = net.createServer();
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve)); const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  try {
    await require("@vscode/test-electron").runTests({
      extensionDevelopmentPath: extension, extensionTestsPath: __filename,
      vscodeExecutablePath: require("../test/vscode-executable").localVSCodeExecutable(),
      extensionTestsEnv: { TUA_CAPTURE_PORT: String(port), TUA_CAPTURE_OUTPUT: process.env.TUA_CAPTURE_OUTPUT || path.join(extension, "docs/screenshots") },
      launchArgs: [project, "--disable-extensions", "--disable-workspace-trust", "--skip-welcome", "--skip-release-notes", `--user-data-dir=${user}`, `--extensions-dir=${temp}/extensions`, `--remote-debugging-port=${port}`],
    });
  } finally { fs.rmSync(temp, { recursive: true, force: true }); }
}
if (require.main === module) main().catch(error => { console.error(error); process.exitCode = 1; });
