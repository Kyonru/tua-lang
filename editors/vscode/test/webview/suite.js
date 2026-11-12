"use strict";

const assert = require("assert/strict");
const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const { BrowserSession, waitFor } = require("./cdp");
const { spritePreviewHtml, spritePreviewErrorHtml } = require("../../src/love/sprite-preview/template");
const { stateMachinePreviewHtml } = require("../../src/state-machines/template");
const { behaviorTreePreviewHtml } = require("../../src/behavior-trees/preview");
const { audioExplorerHtml, audioExplorerErrorHtml } = require("../../src/audio");
const { ecsExplorerHtml } = require("../../src/ecs/template");
const { moduleGraphHtml } = require("../../src/modules/template");
const { objectPoolExplorerHtml } = require("../../src/object-pools/template");
const { signalGraphHtml } = require("../../src/signals/signal-graph");
const { signalGraphViewModel } = require("../../src/signals/signal-graph");
const { ecsExplorerViewModel } = require("../../src/ecs/view-model");
const { objectPoolExplorerViewModel } = require("../../src/object-pools/view-model");
const { serverHealthHtml } = require("../../src/health/template");
const { serverHealthViewModel } = require("../../src/health/view-model");

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
    await waitFor(() => workbench.evaluate("window.innerWidth <= 640"), "narrow animation viewport");
    assert.equal(await evaluate("document.querySelector('.stage').getBoundingClientRect().bottom <= document.querySelector('.inspector').getBoundingClientRect().top + 1"), true, "narrow preview must not cover playback controls");
    assert.equal(await evaluate("document.documentElement.scrollWidth <= window.innerWidth + 1"), true, "narrow animation layout must not overflow the page");
    await screenshot("animation-narrow");
    await workbench.send("Emulation.clearDeviceMetricsOverride");
    await load(spritePreviewHtml(panel.webview, { ...model, source: "other.tua" }), "Tua Sprite Preview");
    await waitFor(() => evaluate("document.querySelector('.animation-tabs .active')?.textContent === 'idle'"), "independent source selection");
    const bordered = { ...model, source: "bordered.tua", sprites: [{ ...model.sprites[0],
      frame: { width: 16, height: 16, border: 1, left: 0, top: 0 },
      animations: [animation("spaced", 0, 2, true)],
    }] };
    await load(spritePreviewHtml(panel.webview, bordered), "Tua Sprite Preview");
    await waitFor(() => evaluate("document.querySelectorAll('.frame-chip').length === 3"), "bordered sprite frames");
    await evaluate("document.querySelectorAll('.frame-chip')[1].click()");
    assert.equal(await evaluate("document.querySelector('.rect-value').textContent"), "17,0 16x16",
      "sprite preview treats border as spacing between cells");
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
    const pathBeforeZoom = await evaluate("document.querySelector('path.connection')?.getAttribute('d')");
    await evaluate("document.getElementById('zoom-in').click(); drawConnections()");
    assert.equal(await evaluate("document.getElementById('zoom-level').textContent"), "120%");
    assert.equal(await evaluate("document.querySelector('path.connection')?.getAttribute('d')"), pathBeforeZoom, "zoom keeps graph routes aligned with states");
    await evaluate("document.getElementById('graph-scroll').dispatchEvent(new WheelEvent('wheel', {deltaY:-40, ctrlKey:true, bubbles:true, cancelable:true, clientX:200, clientY:200}))");
    assert.equal(await evaluate("viewport.scale() > 1.2"), true, "Control-wheel gesture zooms the graph");
    await evaluate("document.getElementById('zoom-fit').click()");
    assert.equal(await evaluate("viewport.scale() <= 1"), true, "Fit keeps the whole graph visible");
    await screenshot("state-machine");
    await workbench.send("Emulation.setDeviceMetricsOverride", { width: 640, height: 900, deviceScaleFactor: 1, mobile: false });
    await waitFor(() => workbench.evaluate("window.innerWidth <= 640"), "narrow graph viewport");
    assert.equal(await evaluate("document.documentElement.scrollWidth <= window.innerWidth + 1"), true, "narrow graph layout must not overflow the page");
    await screenshot("state-machine-narrow");
    const behaviorNode = (kind, line, children = []) => ({ kind, range: { start: { line, character: 0 }, end: { line, character: 8 } }, children });
    const behavior = [{ name: "patrol", root: behaviorNode("selector", 1, [
      behaviorNode("sequence", 2, [behaviorNode("condition", 3), behaviorNode("action", 4)]),
      behaviorNode("wait", 5),
      behaviorNode("dynamic", 6),
    ]) }];
    await load(behaviorTreePreviewHtml(panel.webview, "file:///actor.tua", behavior), "Behavior Tree Preview");
    await waitFor(() => evaluate("document.querySelectorAll('.node').length === 6"), "behavior tree nodes");
    assert.equal(await evaluate("document.querySelector('.tree > li > .node-frame .node').getBoundingClientRect().bottom < document.querySelector('.tree ul .node').getBoundingClientRect().top"), true, "children sit below their parent");
    assert.equal(await evaluate("document.documentElement.scrollWidth <= window.innerWidth + 1"), true, "wide tree stays inside the scroll region");
    assert.equal(await evaluate("(() => { const region = document.querySelector('.tree-scroll').getBoundingClientRect(); const root = document.querySelector('.tree > li > .node-frame .node').getBoundingClientRect(); return root.left >= region.left && root.right <= region.right; })()"), true, "root is visible when the tree is wider than the editor");
    assert.deepEqual(await evaluate("[...document.querySelectorAll('.tree > li > ul > li > .edge-order')].map(node => node.textContent)"), ["1", "2", "3"], "child order is visible on the branches");
    await evaluate("document.querySelector('.tree-zoom [data-zoom=in]').click()");
    assert.equal(await evaluate("document.querySelector('.tree-zoom output').textContent"), "120%", "behavior tree zooms");
    await evaluate("document.querySelector('.branch-toggle').click()");
    assert.equal(await evaluate("document.querySelector('.tree > li > ul').hidden && document.querySelector('.branch-toggle').getAttribute('aria-expanded') === 'false'"), true, "branch collapse hides descendants accessibly");
    await load(behaviorTreePreviewHtml(panel.webview, "file:///actor.tua", behavior), "Behavior Tree Preview");
    assert.equal(await evaluate("document.querySelector('.tree > li > ul').hidden"), true, "collapsed branch survives preview refresh");
    assert.equal(await evaluate("document.querySelector('.tree-zoom output').textContent"), "120%", "behavior tree zoom survives refresh");
    await evaluate("document.querySelector('.tree-zoom [data-zoom=fit]').click()");
    await evaluate("document.querySelector('.branch-toggle').click()");
    assert.equal(await evaluate("document.querySelector('.tree > li > ul').hidden"), false, "branch can expand again");
    const expandedBounds = await evaluate("(() => { const element = document.querySelector('.tree-scroll'); const region = element.getBoundingClientRect(); const root = document.querySelector('.tree > li > .node-frame').getBoundingClientRect(); return {left:root.left,right:root.right,regionLeft:region.left,regionRight:region.right,scrollLeft:element.scrollLeft,scrollWidth:element.scrollWidth}; })()");
    assert(expandedBounds.left >= expandedBounds.regionLeft && expandedBounds.right <= expandedBounds.regionRight, "expanded branch remains in view: " + JSON.stringify(expandedBounds));
    await evaluate("document.querySelector('.node--leaf').click()");
    await waitFor(() => messages.some((message) => message.command === "openLocation" && message.uri === "file:///actor.tua"), "behavior node source navigation");
    await screenshot("behavior-tree-narrow");
    await workbench.send("Emulation.clearDeviceMetricsOverride");
    await screenshot("behavior-tree");
    const emptyExplorers = [
      ["Module Graph", moduleGraphHtml(panel.webview, { analysisRevision: 1, modules: [], edges: [], cycles: [] }), "#filter"],
      ["ECS Explorer", ecsExplorerHtml(panel.webview, { analysisRevision: 1, components: [], systems: [], queries: [], insights: [] }), "#filter"],
      ["Object Pool Explorer", objectPoolExplorerHtml(panel.webview, { analysisRevision: 1, pools: [], operations: [], insights: [] }), "#filter"],
      ["Signal Graph", signalGraphHtml(panel.webview, { events: [] }), "#event-filter"],
      ["Server Health", serverHealthHtml(panel.webview, serverHealthViewModel({ revision: 1 })), "#reload"],
    ];
    for (const [title, html, focusSelector] of emptyExplorers) {
      await load(html, title);
      await waitFor(() => evaluate(`Boolean(document.querySelector(${JSON.stringify(focusSelector)}))`), `${title} controls`);
      await workbench.send("Emulation.setDeviceMetricsOverride", { width: 640, height: 900, deviceScaleFactor: 1, mobile: false });
      await waitFor(() => workbench.evaluate("window.innerWidth <= 640"), `${title} narrow viewport`);
      const widths = await evaluate("({ viewport: window.innerWidth, document: document.documentElement.scrollWidth, toolbar: document.querySelector('.toolbar')?.scrollWidth, graph: document.querySelector('.graph')?.scrollWidth })");
      assert(widths.document <= widths.viewport + 1, `${title} should fit a narrow editor: ${JSON.stringify(widths)}`);
      if (title === "Signal Graph") {
        const emptyBounds = await evaluate("(() => { const empty = document.querySelector('.graph .empty')?.getBoundingClientRect(); const scroll = document.getElementById('graph-scroll')?.getBoundingClientRect(); return empty && {left:empty.left,width:empty.width,viewport:window.innerWidth,scrollLeft:scroll?.left,scrollWidth:scroll?.width}; })()");
        assert(emptyBounds && emptyBounds.left >= 0 && emptyBounds.left < emptyBounds.viewport && emptyBounds.width <= emptyBounds.viewport,
          "the signal empty state must be visible without horizontal scrolling: " + JSON.stringify(emptyBounds));
      }
      if (title === "Module Graph") {
        assert.equal(await evaluate("(() => { const segments = document.querySelector('.segments'); return window.innerWidth >= 650 || segments.scrollWidth <= segments.clientWidth + 1; })()"), true,
          "module filters must wrap instead of scrolling when the webview is narrow");
      }
      if (title === "Server Health") {
        assert.equal(await evaluate("[...document.querySelectorAll('.metric')].find(item => item.textContent.includes('Recent p95'))?.querySelector('strong')?.textContent"),
          "No samples", "unmeasured latency must not appear as zero milliseconds");
      }
      await screenshot(title.toLowerCase().replaceAll(" ", "-") + "-narrow");
      await workbench.send("Emulation.clearDeviceMetricsOverride");
    }
    for (const [title, createHtml, filterSelector, modeSelector] of [
      ["ECS Explorer", revision => ecsExplorerHtml(panel.webview, { analysisRevision: revision, components: [], systems: [], queries: [], insights: [] }), "#filter", '[data-mode="systems"]'],
      ["Object Pool Explorer", revision => objectPoolExplorerHtml(panel.webview, { analysisRevision: revision, pools: [], operations: [], insights: [] }), "#filter", '[data-mode="operations"]'],
      ["Module Graph", revision => moduleGraphHtml(panel.webview, { analysisRevision: revision, modules: [], edges: [], cycles: [] }), "#filter", '[data-mode="roots"]'],
      ["Signal Graph", revision => signalGraphHtml(panel.webview, { analysisRevision: revision, events: [] }), "#event-filter", '[data-mode="listeners"]'],
    ]) {
      await load(createHtml(2), title);
      await evaluate(`(() => { const filter = document.querySelector(${JSON.stringify(filterSelector)}); filter.value = "player"; filter.dispatchEvent(new Event("input")); document.querySelector(${JSON.stringify(modeSelector)}).click(); })()`);
      if (title === "Module Graph" || title === "Signal Graph") {
        await evaluate("document.getElementById('zoom-in').click()");
        assert.equal(await evaluate("document.getElementById('zoom-level').textContent"), "120%");
      }
      await load(createHtml(3), title);
      assert.equal(await evaluate(`document.querySelector(${JSON.stringify(filterSelector)}).value`), "player", `${title} preserves filtering after refresh`);
      assert.equal(await evaluate(`document.querySelector(${JSON.stringify(modeSelector)}).classList.contains("active")`), true, `${title} preserves mode after refresh`);
      if (title === "Module Graph") assert.equal(await evaluate("document.getElementById('zoom-level').textContent"), "120%", "module graph preserves zoom after refresh");
      if (title === "Signal Graph") assert.equal(await evaluate("document.getElementById('zoom-level').textContent"), "120%", "signal graph preserves zoom after refresh");
    }
    const projectSignalView = (workspaceKey, name) => ({
      ...signalGraphViewModel({ events: [{
        id: name, name, bus: "Events", busUri: `file:///${workspaceKey}/events.tua`,
        calls: [{ uri: `file:///${workspaceKey}/events.tua`, role: "emit", method: "emit", range }],
      }] }),
      workspaceKey,
    });
    await load(signalGraphHtml(panel.webview, projectSignalView("one", "enemy.hit")), "Signal Graph");
    await evaluate('(() => { document.querySelector("[data-mode=all]").click(); const filter = document.querySelector("#event-filter"); filter.value = "enemy"; filter.dispatchEvent(new Event("input")); })()');
    await load(signalGraphHtml(panel.webview, projectSignalView("two", "player.hit")), "Signal Graph");
    assert.equal(await evaluate('document.querySelector("#event-filter").value'), "", "Signal Graph clears a filter from another project");
    assert.equal(await evaluate('document.querySelector(".node.event strong")?.textContent'), "player.hit", "Signal Graph shows the new project's event");
    await load(ecsExplorerHtml(panel.webview, { analysisRevision: 4, focusUri: "file:///one.tua", components: [], systems: [], queries: [], insights: [] }), "ECS Explorer");
    await evaluate('document.querySelector("#filter").value = "old file"; document.querySelector("#filter").dispatchEvent(new Event("input"))');
    await load(ecsExplorerHtml(panel.webview, { analysisRevision: 5, focusUri: "file:///two.tua", components: [], systems: [], queries: [], insights: [] }), "ECS Explorer");
    assert.equal(await evaluate('document.querySelector("#filter").value'), "", "ECS filtering resets when focus moves to another file");
    const ecsView = ecsExplorerViewModel({ analysisRevision: 6, graph: {
      components: [{ id: "position", uri: "file:///game/components.tua", range, localName: "Position", runtimeName: "Position", fields: [{ name: "x", type: "number", range }] }],
      systems: [{ id: "movement", uri: "file:///game/movement.tua", range, name: "Movement", components: [{ reference: "Position", componentId: "position", access: "write", range }] }],
      queries: [], insights: [],
    } }, "file:///game/components.tua");
    await load(ecsExplorerHtml(panel.webview, ecsView), "ECS Explorer");
    assert.equal(await evaluate('document.querySelector("#detail").textContent.includes("Movement")'), true, "component detail shows the related system");
    await evaluate('document.querySelector("#detail .detail-header button").click()');
    await waitFor(() => messages.some(message => message.command === "openLocation" && message.uri === "file:///game/components.tua"), "populated ECS source action");
    await screenshot("ecs-explorer-populated");
    const poolView = objectPoolExplorerViewModel({ analysisRevision: 7, graph: {
      pools: [{ id: "bullets", uri: "file:///game/pools.tua", range, name: "bullets", itemType: "Bullet" }],
      operations: [{ id: "take", uri: "file:///game/pools.tua", range, poolId: "bullets", poolName: "bullets", kind: "acquire", itemName: "bullet" }],
      insights: [],
    } }, "file:///game/pools.tua");
    await load(objectPoolExplorerHtml(panel.webview, poolView), "Object Pool Explorer");
    assert.equal(await evaluate('document.querySelector("#detail").textContent.includes("Bullet") && document.querySelector("#detail").textContent.includes("acquire")'), true, "pool detail shows item type and lifecycle");
    await evaluate('document.querySelector("#detail .detail-header button").click()');
    await waitFor(() => messages.some(message => message.command === "openLocation" && message.uri === "file:///game/pools.tua"), "populated pool source action");
    await screenshot("object-pool-explorer-populated");
    const signalView = signalGraphViewModel({ events: [{
      id: "hit", name: "enemy.hit", bus: "EnemyEvents", busUri: "file:///game/events.tua",
      resolvedPayload: [{ name: "damage", type: "number" }], schemaConflict: false,
      calls: [
        { uri: "file:///game/events.tua", receiver: "EnemyEvents", method: "declare", role: "declaration", range, payload: [{ name: "damage", type: "number" }] },
        { uri: "file:///game/combat.tua", receiver: "EnemyEvents", method: "on", role: "listener", range, payload: [{ name: "damage", type: "number" }] },
        { uri: "file:///game/enemy.tua", receiver: "EnemyEvents", method: "emit", role: "emit", range, inferredPayload: [{ name: "value1", type: "number" }] },
      ],
    }] });
    await load(signalGraphHtml(panel.webview, signalView), "Signal Graph");
    await evaluate('(() => { const filter = document.querySelector("#event-filter"); filter.value = ""; filter.dispatchEvent(new Event("input")); document.querySelector("[data-mode=all]").click(); })()');
    assert.equal(await evaluate('document.querySelectorAll(".call-row").length'), 3, "signal detail lists declaration, listener, and emit call");
    assert.equal(await evaluate('(() => { const label = document.querySelector(".node.event strong"); return label.textContent === "enemy.hit" && label.getBoundingClientRect().width >= 50; })()'), true, "signal graph keeps the event name visible beside long bus metadata");
    await evaluate('document.querySelector(".call-row.role-emit").click()');
    await waitFor(() => messages.some(message => message.command === "openLocation" && message.uri === "file:///game/enemy.tua"), "populated signal source action");
    await screenshot("signal-graph-populated");
    const audioGraph = { systems: [{ uri: "file:///game/audio.tua", displayPath: "src/audio.tua", range: { start: { line: 4 } },
      buses: ["master"], cues: ["hit"], musicStates: ["combat"], layers: ["drums"], effects: [],
      cueItems: [{ id: "cue:hit", name: "hit", range: { start: { line: 6 } }, source: { kind: "asset", loader: "sound", assetPath: "assets/hit.ogg" }, assetUri: "file:///game/assets/hit.ogg", assetState: "present", properties: { bus: { kind: "string", value: "sfx" }, maxVoices: { kind: "number", value: "4" } } }],
      sceneItems: [{ id: "scene:combat", name: "combat", range: { start: { line: 10 } }, properties: { bpm: { kind: "number", value: "120" } }, layers: [{ id: "layer:drums", name: "drums", range: { start: { line: 11 } }, source: { kind: "asset", loader: "music", assetPath: "assets/drums.ogg" }, assetState: "present", assetUri: "file:///game/assets/drums.ogg", properties: {} }] },
        { id: "scene:quiet", name: "quiet", range: { start: { line: 14 } }, properties: { bpm: { kind: "number", value: "120" } }, layers: [{ id: "layer:pad", name: "pad", range: { start: { line: 15 } }, source: { kind: "asset", loader: "music", assetPath: "assets/pad.ogg" }, assetState: "present", assetUri: "file:///game/assets/pad.ogg", properties: {} }] }],
      busItems: [{ name: "sfx", properties: {} }], effectItems: [] }] };
    await load(audioExplorerHtml(panel.webview, audioGraph, [{ path: "assets/hit.ogg", assetUri: "file:///game/assets/hit.ogg", bpm: 120, confidence: 0.9,
      duration: 0.18, channels: 1, sampleRate: 44100, peak: 0.82, energy: [0.1, 0.8, 0.3, 0.05] }]), "Audio Workbench");
    await waitFor(() => evaluate("document.querySelectorAll('[data-uri]').length >= 4"), "audio source and asset actions");
    assert.equal(await evaluate('document.querySelector(".detail").textContent.includes("Cue voice limit") && document.querySelector(".detail").textContent.includes("assets/hit.ogg")'), true, "cue detail exposes authored source and voice policy");
    assert.equal(await evaluate('document.querySelector(".detail").textContent.includes("0.18 s") && document.querySelector(".energy[aria-label]") !== null'), true, "cue detail ties bounded analysis to its asset");
    assert.equal(await evaluate('document.querySelector("[data-play]").disabled'), false, "static asset cue can be auditioned");
    await evaluate("document.querySelector('[data-uri]').click()");
    await waitFor(() => messages.some(message => message.command === "openLocation" && message.uri === "file:///game/audio.tua" && message.range?.start?.line === 4), "audio declaration navigation");
    await screenshot("audio-explorer");
    await load(audioExplorerHtml(panel.webview, audioGraph, [], { mode: "scenes", selectedId: "scene:combat", revision: 42 }), "Audio Workbench");
    assert.equal(await evaluate('document.querySelector(".detail").textContent.includes("Authored BPM") && document.querySelector(".detail").textContent.includes("drums")'), true, "music scene shows authored timing and layer ownership");
    assert.equal(await evaluate('document.querySelector("[data-play-scene]").disabled'), false, "static scene is auditionable");
    await evaluate('document.querySelector("#transition-quantize").value="bar"; document.querySelector("[data-transition]").click()');
    await waitFor(() => messages.some(message => message.command === "transition" && message.targetId === "scene:quiet" && message.quantize === "bar"), "scene transition action");
    await evaluate('document.querySelector(".layers button").click()');
    await waitFor(() => messages.some(message => message.command === "openLocation" && message.range?.start?.line === 11), "audio layer source navigation");
    await screenshot("audio-scenes");
    await workbench.send("Emulation.setDeviceMetricsOverride", { width: 640, height: 900, deviceScaleFactor: 1, mobile: false });
    await waitFor(() => workbench.evaluate("window.innerWidth <= 640"), "narrow audio viewport");
    assert.equal(await evaluate("document.documentElement.scrollWidth <= window.innerWidth + 1"), true, "audio explorer should fit a narrow editor");
    await screenshot("audio-explorer-narrow");
    await workbench.send("Emulation.clearDeviceMetricsOverride");
    const audioReloadCount = messages.filter(message => message.command === "reload").length;
    await load(audioExplorerErrorHtml(panel.webview, new Error("Server unavailable")), "Audio Workbench");
    assert.equal(await evaluate("document.querySelector('[role=alert]').textContent"), "Server unavailable");
    await evaluate("document.getElementById('reload').click()");
    await waitFor(() => messages.filter(message => message.command === "reload").length > audioReloadCount, "audio retry action");
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
