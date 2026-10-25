"use strict";

const assert = require("assert/strict");
const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const { BrowserSession, waitFor } = require("./cdp");
const { spritePreviewHtml, spritePreviewErrorHtml } = require("../../src/love/sprite-preview/template");
const { stateMachinePreviewHtml } = require("../../src/state-machines/template");

async function run() {
  const panel = vscode.window.createWebviewPanel("tuaPreviewTest", "Tua Preview Test", vscode.ViewColumn.One, { enableScripts: true });
  const messages = [];
  panel.webview.onDidReceiveMessage((message) => messages.push(message));
  const png = fs.readFileSync(path.join(__dirname, "../fixtures/player.png"));
  const animation = (name, from, to, loop) => ({ name, from, to, row: 0, fps: 12, frames: [], durationValues: [], metadata: [], loop });
  const model = {
    source: "player.tua",
    sprites: [{ name: "Player", imagePath: "player.png", imageUri: `data:image/png;base64,${png.toString("base64")}`,
      frame: { width: 16, height: 16, border: 0, left: 0, top: 0 }, diagnostics: [],
      animations: [animation("idle", 0, 3, true), animation("attack", 4, 7, false)],
    }],
  };
  panel.webview.html = spritePreviewHtml(panel.webview, model);
  const target = await waitFor(async () => {
    const targets = await (await fetch(`http://127.0.0.1:${process.env.TUA_PREVIEW_CDP_PORT}/json/list`)).json();
    return targets.find((item) => item.type === "iframe" && item.url.startsWith("vscode-webview:"));
  }, "preview debugging target");
  const browser = await BrowserSession.connect(target.webSocketDebuggerUrl);
  const targets = await (await fetch(`http://127.0.0.1:${process.env.TUA_PREVIEW_CDP_PORT}/json/list`)).json();
  const workbench = await BrowserSession.connect(targets.find(item => item.type === "page").webSocketDebuggerUrl);
  await workbench.send("Page.bringToFront");
  const screenshot = async (name) => {
    if (!process.env.TUA_PREVIEW_SCREENSHOTS) return;
    fs.mkdirSync(process.env.TUA_PREVIEW_SCREENSHOTS, { recursive: true });
    const image = await workbench.send("Page.captureScreenshot");
    fs.writeFileSync(path.join(process.env.TUA_PREVIEW_SCREENSHOTS, name + ".png"), Buffer.from(image.data, "base64"));
  };
  let context;
  let reloadSerial = 0;
  const evaluate = (expression) => browser.evaluate(expression, context);
  const load = async (html, title) => {
    const previous = context;
    // Match a user-initiated reload: let the host receive queued setState calls
    // before replacing the document from outside the webview.
    const serial = ++reloadSerial;
    await evaluate(`vscode.postMessage({ command: "test:flush", serial: ${serial} })`);
    await waitFor(() => messages.some(message => message.command === "test:flush" && message.serial === serial), "persisted preview state");
    panel.webview.html = html;
    context = await waitFor(async () => {
      const next = await browser.documentContext(title);
      return next !== previous ? next : undefined;
    }, "replacement preview document");
  };
  try {
    context = await browser.documentContext("Tua Sprite Preview");
    await waitFor(() => evaluate("document.querySelectorAll('.frame-chip').length === 4 && !document.querySelector('.play').disabled"), "decoded sprite image");
    // A controlled browser clock avoids desktop occlusion throttling rAF on CI.
    await evaluate(`(() => {
      if (document.querySelector('.play').textContent === 'Pause') document.querySelector('.play').click();
      globalThis.previewHidden = false;
      Object.defineProperty(document, 'hidden', { get: () => previewHidden });
      globalThis.animationFramesForTest = new Map();
      globalThis.previewFrameId = 0;
      globalThis.previewTime = performance.now();
      globalThis.requestAnimationFrame = callback => { const id = ++previewFrameId; animationFramesForTest.set(id, callback); return id; };
      globalThis.cancelAnimationFrame = id => animationFramesForTest.delete(id);
      globalThis.advancePreview = count => {
        for (let i = 0; i < count; i++) {
          previewTime += 17;
          const callbacks = [...animationFramesForTest.values()];
          animationFramesForTest.clear();
          callbacks.forEach(callback => callback(previewTime));
        }
      };
      document.querySelector('.play').click();
      globalThis.chip = document.querySelector('.frame-chip');
      chip.focus();
      advancePreview(30);
    })()`);
    assert.equal(await evaluate("chip.isConnected && document.activeElement === chip"), true, "playback preserves frame-button identity and keyboard focus");
    assert.equal(await evaluate("document.querySelector('.sheet-canvas').width <= 320"), true);
    assert.equal(await evaluate("previewHidden = true; document.dispatchEvent(new Event('visibilitychange')); animationFramesForTest.size"), 0, "hidden previews stop requesting frames");
    await evaluate("previewHidden = false; document.dispatchEvent(new Event('visibilitychange')); document.querySelectorAll('.animation-tabs button')[1].click(); advancePreview(40)");
    assert.equal(await evaluate("document.querySelector('.play').textContent"), "Play", "non-looping animations stop at their end");
    assert.equal(await evaluate("animationFramesForTest.size"), 0, "paused previews stop requesting frames");
    assert.equal(await evaluate("Object.values(vscode.getState().sprites)[0].playing"), false, "completed playback is persisted immediately");
    assert.equal(await evaluate("document.querySelector('.play').click(); document.querySelector('.frame-value').textContent.includes('(1/4)')"), true, "Play restarts a completed animation");
    await evaluate("document.querySelectorAll('.frame-chip')[1].click(); const speed = document.querySelector('.speed'); speed.value = '1.5'; speed.dispatchEvent(new Event('input')); const zoom = document.querySelector('.zoom'); zoom.value = '2'; zoom.dispatchEvent(new Event('input'));");
    await load(spritePreviewHtml(panel.webview, model), "Tua Sprite Preview");
    await waitFor(() => evaluate("document.querySelector('.frame-value')?.textContent.includes('(2/4)')"), "restored paused frame");
    assert.deepEqual(await evaluate("[document.querySelector('.animation-tabs .active').textContent, document.querySelector('.speed').value, document.querySelector('.zoom').value, document.querySelector('.play').textContent]"), ["attack", "1.5", "2", "Play"]);

    assert.equal(await evaluate("document.getElementById('reload-preview').click(); document.getElementById('reload-preview').disabled"), true);
    assert.equal(await evaluate("window.dispatchEvent(new MessageEvent('message', { data:{command:'reloadComplete'} })); document.getElementById('reload-preview').disabled"), false, "reload remains available if the server cannot refresh");
    await screenshot("animation");
    await workbench.send("Emulation.setDeviceMetricsOverride", { width: 640, height: 900, deviceScaleFactor: 1, mobile: false });
    await waitFor(() => evaluate("window.innerWidth <= 640"), "narrow animation viewport");
    assert.equal(await evaluate("document.querySelector('.stage').getBoundingClientRect().bottom <= document.querySelector('.inspector').getBoundingClientRect().top + 1"), true, "narrow preview must not cover playback controls");
    assert.equal(await evaluate("document.documentElement.scrollWidth <= window.innerWidth + 1"), true, "narrow animation layout must not overflow the page");
    await screenshot("animation-narrow");
    await workbench.send("Emulation.clearDeviceMetricsOverride");
    await load(spritePreviewHtml(panel.webview, { ...model, source: "other.tua" }), "Tua Sprite Preview");
    await waitFor(() => evaluate("document.querySelector('.animation-tabs .active')?.textContent === 'idle'"), "independent source selection");
    const broken = { ...model, sprites: [{ ...model.sprites[0], imageUri: "data:image/png;base64,bm90LWFuLWltYWdl" }] };
    await load(spritePreviewHtml(panel.webview, broken), "Tua Sprite Preview");
    await waitFor(() => evaluate("document.querySelector('.stage-status')?.textContent.includes('Could not decode')"), "corrupt image feedback");
    assert.equal(await evaluate("document.querySelector('.play').disabled"), true);
    const reloadCount = messages.filter(message => message.command === "reload").length;
    await load(spritePreviewErrorHtml(panel.webview, "player.tua", "Inspection failed"), "Tua Sprite Preview");
    await evaluate("document.getElementById('retry').click()");
    await waitFor(() => messages.filter(message => message.command === "reload").length > reloadCount, "preview retry action");

    const range = { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } };
    const machine = (name) => ({ name, initial: "idle", range, selectionRange: range,
      states: ["idle", "running", "paused"].map((name) => ({ name, range })),
      transitions: [
        { id: `${name}:start:0`, event: "start", from: ["idle"], to: "running", range, selectionRange: range },
        { id: `${name}:pause:1`, event: "pause", from: ["running"], to: "paused", range, selectionRange: range },
      ],
    });
    const graph = { source: "file:///actor.tua", sourceLabel: "actor.tua", machines: [machine("menu"), machine("player")] };
    await load(stateMachinePreviewHtml(panel.webview, graph), "Tua State Machine Preview");
    await evaluate("(() => { const picker = document.getElementById('machine-picker'); picker.value = '1'; picker.dispatchEvent(new Event('change')); })()");
    // Run the normal layout callback explicitly when macOS occlusion suspends rAF.
    await evaluate("drawConnections()");
    await waitFor(() => evaluate("document.querySelectorAll('.edge-label').length === 2"), "state graph routes");
    assert.equal(await evaluate("globalThis.node = document.querySelector('[data-state=running]'); node.focus(); node.click(); node.isConnected && document.activeElement === node"), true, "graph selection preserves focus");
    await evaluate("node.dispatchEvent(new MouseEvent('dblclick', { bubbles:true }))");
    await waitFor(() => messages.some((message) => message.command === "openLocation"), "double-click source navigation");
    assert.equal(await evaluate("globalThis.edge = document.querySelector('.edge-label'); edge.focus(); edge.click(); edge.isConnected && document.activeElement === edge"), true, "transition selection preserves focus");
    const reordered = { ...graph, machines: [machine("player"), machine("menu")] };
    reordered.machines[0].transitions[0].id = "player:start:5";
    await load(stateMachinePreviewHtml(panel.webview, reordered), "Tua State Machine Preview");
    await evaluate("drawConnections()");
    await waitFor(() => evaluate("document.querySelector('.edge-label.selected')?.textContent === 'start'"), "restored transition selection");
    assert.equal(await evaluate("document.querySelector('#machine-picker').selectedOptions[0].textContent"), "player");
    await screenshot("state-machine");
    await workbench.send("Emulation.setDeviceMetricsOverride", { width: 640, height: 900, deviceScaleFactor: 1, mobile: false });
    await waitFor(() => evaluate("window.innerWidth <= 640"), "narrow graph viewport");
    assert.equal(await evaluate("document.documentElement.scrollWidth <= window.innerWidth + 1"), true, "narrow graph layout must not overflow the page");
    await screenshot("state-machine-narrow");
    for (const [theme, className] of [["Light Modern", "vscode-light"], ["Default High Contrast", "vscode-high-contrast"], ["Default High Contrast Light", "vscode-high-contrast-light"]]) {
      await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
      await waitFor(() => evaluate(`document.body.classList.contains(${JSON.stringify(className)})`), theme + " theme");
      await screenshot(className);
    }
    assert.deepEqual(browser.errors, [], "preview scripts must not throw");
    console.log("Preview webviews passed: decoded images, playback focus, replay, restore, error recovery, graph focus, double-click navigation, and selection persistence.");
  } finally {
    workbench.dispose();
    browser.dispose();
    panel.dispose();
  }
}

module.exports = { run };
