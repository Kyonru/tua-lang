"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");
const { performance } = require("node:perf_hooks");
const { BrowserSession, waitFor } = require(path.resolve(__dirname, "../../../editors/vscode/test/webview/cdp"));
const { signalGraphHtml, signalGraphViewModel } = require(path.resolve(__dirname, "../../../editors/vscode/src/signals/signal-graph"));
const { moduleGraphHtml } = require(path.resolve(__dirname, "../../../editors/vscode/src/modules/template"));

const range = { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } };
function signalFixture(folder) {
  const uri = vscode.Uri.joinPath(folder.uri, "src/main.tua").toString();
  return signalGraphViewModel({ events: Array.from({ length: 500 }, (_, index) => ({
    name: `event_${String(index).padStart(3, "0")}`, bus: "GameEvents", busUri: uri,
    calls: ["declaration", "listener", "emit"].map((role, callIndex) => ({
      role, uri, range: { start: { line: index + callIndex, character: 0 }, end: { line: index + callIndex, character: 1 } }
    }))
  })) });
}
function moduleFixture(folder) {
  const uri = index => vscode.Uri.joinPath(folder.uri, `src/module_${index}.tua`).toString();
  const modules = Array.from({ length: 500 }, (_, index) => ({
    uri: uri(index), file: `module_${index}.tua`, sourceKind: "tua", root: index === 0,
    inCycle: false, unresolvedDependencyCount: 0, dependencyCount: index < 499 ? 1 : 0,
    reverseDependencyCount: index > 0 ? 1 : 0, exports: [], inclusionPath: { steps: index === 0 ? [] : [{}] }
  }));
  const edges = Array.from({ length: 499 }, (_, index) => ({
    id: `edge_${index}`, sourceUri: uri(index), sourceFile: `module_${index}.tua`,
    specifier: `module_${index + 1}`, kind: "require", range,
    resolution: { status: "resolved", targetUri: uri(index + 1), targetFile: `module_${index + 1}.tua` }
  }));
  return { analysisRevision: 1, modules, edges, cycles: [], focusUri: uri(0) };
}
async function run() {
  const extension = vscode.extensions.getExtension("SolenodonteLabs.tua");
  assert(extension);
  await extension.activate();
  const [folder] = vscode.workspace.workspaceFolders;
  const document = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(folder.uri, "src/main.tua"));
  await vscode.window.showTextDocument(document);
  const panel = vscode.window.createWebviewPanel("tuaStress", "Tua graph stress", vscode.ViewColumn.One, { enableScripts: true });
  const messages = [];
  panel.webview.onDidReceiveMessage(message => messages.push(message));
  const port = Number(process.env.TUA_PREVIEW_STRESS_PORT);
  const target = await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${port}/json/list`);
    return (await response.json()).find(item => item.type === "iframe" && item.url.startsWith("vscode-webview:"));
  }, "preview target", 15000);
  const browser = await BrowserSession.connect(target.webSocketDebuggerUrl);
  const report = { vscode: vscode.version, signal: {}, module: {}, realModule: {}, nativeAsset: {}, nativeAssets: {}, rendererErrors: [] };
  let context;
  try {
    let started = performance.now();
    panel.webview.html = signalGraphHtml(panel.webview, signalFixture(folder));
    context = await browser.documentContext("Signal Graph");
    await waitFor(() => browser.evaluate("document.querySelectorAll('.node.event').length === 500", context), "500 signal nodes", 30000);
    report.signal.renderHostMs = performance.now() - started;
    report.signal.summary = await browser.evaluate("document.getElementById('summary').textContent", context);
    report.signal.filterMs = await browser.evaluate(`(() => { const input=document.getElementById('event-filter'); const start=performance.now(); input.value='event_499'; input.dispatchEvent(new Event('input',{bubbles:true})); return performance.now()-start; })()`, context);
    report.signal.filteredNodes = await browser.evaluate("document.querySelectorAll('.node.event').length", context);
    assert.equal(report.signal.filteredNodes, 1);
    await browser.evaluate("document.querySelector('.call-row').click()", context);
    await waitFor(() => messages.some(message => message.command === "openLocation"), "signal source action", 5000);
    report.signal.sourceAction = messages.at(-1).command;

    started = performance.now();
    panel.webview.html = moduleGraphHtml(panel.webview, moduleFixture(folder));
    context = await browser.documentContext("Module Graph");
    await waitFor(() => browser.evaluate("document.querySelectorAll('.module-node').length === 500", context), "500 module nodes", 30000);
    report.module.renderHostMs = performance.now() - started;
    report.module.summary = await browser.evaluate("document.getElementById('summary').textContent", context);
    report.module.filterMs = await browser.evaluate(`(() => { const input=document.getElementById('filter'); const start=performance.now(); input.value='module_499'; input.dispatchEvent(new Event('input',{bubbles:true})); return performance.now()-start; })()`, context);
    report.module.filteredNodes = await browser.evaluate("document.querySelectorAll('.module-node').length", context);
    assert.equal(report.module.filteredNodes, 2, "filter matches module and its importing neighbor");
    await browser.evaluate("document.querySelector('[data-module-uri$=\"module_499.tua\"]').dispatchEvent(new MouseEvent('dblclick',{bubbles:true}))", context);
    await waitFor(() => messages.filter(message => message.command === "openLocation").length > 1, "module source action", 5000);
    report.module.sourceAction = messages.at(-1).command;

    const real = JSON.parse(fs.readFileSync(process.env.TUA_REAL_GRAPH_JSON, "utf8"));
    assert.equal(real.graph.modules.length, 121);
    assert.equal(real.graph.edges.length, 120);
    const world = real.graph.modules.find(module => module.uri.endsWith("module_119.tua"));
    assert(world?.exports.some(field => field.name === "value" && field.type === "number"));
    started = performance.now();
    panel.webview.html = moduleGraphHtml(panel.webview, { ...real.graph, analysisRevision: real.analysisRevision });
    await waitFor(async () => {
      try {
        const current = await browser.documentContext("Module Graph");
        if (await browser.evaluate(`document.querySelectorAll('.module-node').length === ${real.graph.modules.length}`, current)) {
          context = current;
          return true;
        }
      } catch { /* The previous webview context can be destroyed during replacement. */ }
      return false;
    }, "compiler-produced module graph nodes", 30000);
    report.realModule = {
      compilerNodes: real.graph.modules.length,
      compilerEdges: real.graph.edges.length,
      renderedNodes: await browser.evaluate("document.querySelectorAll('.module-node').length", context),
      renderedLinks: await browser.evaluate("document.querySelectorAll('.connection').length", context),
      renderHostMs: performance.now() - started,
    };
    assert.equal(report.realModule.renderedNodes, report.realModule.compilerNodes);
    assert.equal(report.realModule.renderedLinks, report.realModule.compilerEdges);

    const assetUri = vscode.Uri.joinPath(folder.uri, "src/assets/player.png");
    const previewResult = await vscode.commands.executeCommand("tua.assets.preview", assetUri.toString());
    await waitFor(() => vscode.window.tabGroups.activeTabGroup.activeTab?.input?.uri?.toString() === assetUri.toString(), "native asset tab", 10000);
    const tab = vscode.window.tabGroups.activeTabGroup.activeTab;
    report.nativeAsset = { result: previewResult, label: tab.label, inputType: tab.input.constructor.name, uri: tab.input.uri.toString() };
    const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    const workbench = await BrowserSession.connect(targets.find(item => item.type === "page").webSocketDebuggerUrl);
    try {
      await workbench.send("Page.bringToFront");
      await new Promise(resolve => setTimeout(resolve, 750));
      const shot = await workbench.send("Page.captureScreenshot");
      fs.writeFileSync(path.join(__dirname, "native-asset.png"), Buffer.from(shot.data, "base64"));
      report.nativeAsset.screenshot = "native-asset.png";
      report.nativeAsset.workbenchErrors = workbench.errors.length;
      for (const name of ["sfx_move.wav", "sfx_bump.ogg", "sniglet.otf", "wave.glsl"]) {
        const uri = vscode.Uri.joinPath(folder.uri, "src/assets", name);
        const result = await vscode.commands.executeCommand("tua.assets.preview", uri.toString());
        await waitFor(() => vscode.window.tabGroups.activeTabGroup.activeTab?.input?.uri?.toString() === uri.toString(), `${name} asset tab`, 10000);
        const active = vscode.window.tabGroups.activeTabGroup.activeTab;
        report.nativeAssets[name] = { result, label: active.label, inputType: active.input.constructor.name, uri: active.input.uri.toString() };
        await new Promise(resolve => setTimeout(resolve, 750));
        const assetShot = await workbench.send("Page.captureScreenshot");
        const screenshot = name === "sfx_move.wav" ? "native-audio-wav.png" : `native-${name.replace('.', '-')}.png`;
        fs.writeFileSync(path.join(__dirname, screenshot), Buffer.from(assetShot.data, "base64"));
        report.nativeAssets[name].screenshot = screenshot;
      }
    } finally {
      workbench.dispose();
    }
    report.rendererErrors = browser.errors;
    assert.deepEqual(browser.errors, []);
    fs.writeFileSync(process.env.TUA_PREVIEW_STRESS_OUT, JSON.stringify(report, null, 2) + "\n");
    console.log("PREVIEW_STRESS", JSON.stringify(report));
  } finally {
    browser.dispose();
    panel.dispose();
  }
}
module.exports = { run };
