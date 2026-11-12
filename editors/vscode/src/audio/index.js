"use strict";

const path = require("node:path");
const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { isPathInside, projectLayoutForUri } = require("../editor/project-layout");
const { openLocationMessage } = require("../shared/locations");
const { createRenderGate } = require("../shared/render-gate");
const { escapeHtml, webviewDocument } = require("../shared/webview");
const { createAudition } = require("./audition");
const { canAudition, canAuditionScene } = require("./audition-source");

const styles = `
  body{margin:0;color:var(--vscode-foreground);background:var(--vscode-editor-background);font:var(--vscode-font-size) var(--vscode-font-family)}
  main{max-width:1100px;margin:auto;padding:18px}button,input{font:inherit}button{cursor:pointer}button:focus-visible,input:focus-visible{outline:2px solid var(--vscode-focusBorder);outline-offset:2px}
  .toolbar{display:flex;align-items:center;gap:8px;flex-wrap:wrap;padding-bottom:14px;border-bottom:1px solid var(--vscode-panel-border)}
  h1{font-size:18px;margin:0 auto 0 0}.muted,.eyeline{color:var(--vscode-descriptionForeground)}.eyeline{font-size:12px}.toolbar button,.actions button{border:1px solid var(--vscode-button-border,var(--vscode-panel-border));border-radius:4px;background:var(--vscode-button-background);color:var(--vscode-button-foreground);padding:6px 10px}.toolbar button:hover,.actions button:hover{background:var(--vscode-button-hoverBackground)}
  .toolbar .quiet,.actions .quiet{background:var(--vscode-editor-background);color:var(--vscode-foreground)}.toolbar .quiet:hover,.actions .quiet:hover{background:var(--vscode-list-hoverBackground)}
  .modes{display:flex;gap:12px;margin:14px 0 0;border-bottom:1px solid var(--vscode-panel-border)}.modes button{border:0;border-bottom:2px solid transparent;padding:8px 2px;background:transparent;color:var(--vscode-descriptionForeground)}.modes button[aria-selected=true]{border-color:var(--vscode-focusBorder);color:var(--vscode-foreground);font-weight:600}
  .workspace{display:grid;grid-template-columns:minmax(185px,28%) minmax(0,1fr);min-height:420px}.index{border-right:1px solid var(--vscode-panel-border);padding:14px 12px 14px 0}.index label{display:block;font-size:12px;color:var(--vscode-descriptionForeground);margin-bottom:6px}.index input{box-sizing:border-box;width:100%;padding:6px 8px;color:var(--vscode-input-foreground);background:var(--vscode-input-background);border:1px solid var(--vscode-input-border,var(--vscode-panel-border));border-radius:3px}
  .item-list{list-style:none;padding:0;margin:12px 0}.item-list li{margin:2px 0}.item-list button{display:block;text-align:left;width:100%;border:0;border-radius:4px;background:transparent;color:var(--vscode-foreground);padding:8px}.item-list button:hover{background:var(--vscode-list-hoverBackground)}.item-list button[aria-current=true]{background:var(--vscode-list-activeSelectionBackground);color:var(--vscode-list-activeSelectionForeground)}.item-list small{display:block;margin-top:3px;opacity:.75;overflow-wrap:anywhere}
  .detail{min-width:0;padding:18px 0 20px 20px}.detail h2{font-size:21px;line-height:1.2;margin:4px 0 7px;overflow-wrap:anywhere}.detail h3{font-size:13px;margin:22px 0 8px}.detail p{margin:8px 0;line-height:1.45}.detail .actions{display:flex;gap:8px;flex-wrap:wrap;margin:16px 0 18px}.detail button[disabled]{opacity:.55;cursor:not-allowed}.facts{display:grid;grid-template-columns:minmax(90px,150px) minmax(0,1fr);border-top:1px solid var(--vscode-panel-border)}.facts dt,.facts dd{margin:0;padding:8px 0;border-bottom:1px solid var(--vscode-panel-border);overflow-wrap:anywhere}.facts dt{color:var(--vscode-descriptionForeground)}.notice{padding:10px 12px;border:1px solid var(--vscode-panel-border);border-radius:4px;color:var(--vscode-descriptionForeground)}.notice[role=alert]{border-color:var(--vscode-inputValidation-errorBorder,var(--vscode-errorForeground));color:var(--vscode-errorForeground)}
  .layers{list-style:none;padding:0;margin:0}.layers li{display:flex;justify-content:space-between;gap:8px;padding:9px 0;border-bottom:1px solid var(--vscode-panel-border)}.layers button{border:0;background:transparent;color:var(--vscode-textLink-foreground);padding:0;text-align:left}.analysis{border-top:1px solid var(--vscode-panel-border);margin-top:24px;padding-top:12px}.analysis h2{font-size:14px}.analysis-list{list-style:none;padding:0}.analysis-list li{padding:5px 0}.analysis-list button{border:0;background:transparent;color:var(--vscode-textLink-foreground);padding:0}
  .transition{display:flex;align-items:end;gap:8px;flex-wrap:wrap;padding:10px 0 16px;border-bottom:1px solid var(--vscode-panel-border)}.transition label{display:grid;gap:4px;font-size:12px;color:var(--vscode-descriptionForeground)}.transition select{min-width:110px;padding:5px;background:var(--vscode-dropdown-background);color:var(--vscode-dropdown-foreground);border:1px solid var(--vscode-dropdown-border,var(--vscode-panel-border))}.transition button{padding:6px 10px;background:var(--vscode-button-background);color:var(--vscode-button-foreground);border:0;border-radius:3px}
  .energy{width:100%;height:64px;border:1px solid var(--vscode-panel-border);border-radius:3px;color:var(--vscode-charts-blue,var(--vscode-textLink-foreground))}.energy polyline{fill:none;stroke:currentColor;stroke-width:1.5;vector-effect:non-scaling-stroke}
  @media(max-width:620px){main{padding:12px}.workspace{display:block}.index{border-right:0;border-bottom:1px solid var(--vscode-panel-border);padding-right:0}.detail{padding:16px 0}.item-list{max-height:180px;overflow:auto}}
  .vscode-high-contrast .item-list button[aria-current=true],.vscode-high-contrast-light .item-list button[aria-current=true]{outline:1px solid var(--vscode-contrastBorder)}
`;

function documentHtml(webview, body, script = "") {
  return webviewDocument(webview, { title: "Audio Workbench", body: `<main>${body}</main>`, styles, script });
}
function loadingHtml(webview) { return documentHtml(webview, '<p role="status">Loading audio declarations…</p>'); }
function errorHtml(webview, error) {
  return documentHtml(webview,
    `<h1>Audio Workbench unavailable</h1><p role="alert">${escapeHtml(error instanceof Error ? error.message : String(error))}</p><button id="reload" type="button">Retry</button>`,
    'document.getElementById("reload").addEventListener("click",()=>vscode.postMessage({command:"reload"}));');
}
function property(item, key) {
  const value = item?.properties?.[key];
  return value?.kind === "dynamic" ? "computed at runtime" : value?.value ?? "not set";
}
function fact(name, value) { return `<dt>${escapeHtml(name)}</dt><dd>${escapeHtml(String(value))}</dd>`; }
function sourceLocation(uri, range, label, extra = "") {
  return `<button type="button" class="quiet" data-uri="${escapeHtml(uri)}" data-line="${Number(range?.start?.line || 0)}" data-character="${Number(range?.start?.character || 0)}" ${extra}>${escapeHtml(label)}</button>`;
}
function itemRows(items, selectedId) {
  return items.map(({ system, item }) => `<li><button type="button" data-select="${escapeHtml(item.id)}" aria-current="${item.id === selectedId}"><strong>${escapeHtml(item.name)}</strong><small>${escapeHtml(system.displayPath || system.uri)}</small></button></li>`).join("");
}
function selectedDetail(entry, mode, analyses) {
  if (!entry) return '<section class="detail"><p class="muted">Select a cue or music scene to inspect its authored settings.</p></section>';
  const { item, system } = entry;
  const isCue = mode === "cues";
  const source = item.source;
  const asset = source?.kind === "asset" ? source.assetPath : undefined;
  const canPlay = isCue && canAudition(system, item);
  const sourceLabel = asset || (source?.kind === "computed" ? "computed Source" : "unknown or dynamic");
  const analysis = asset && analyses.find(candidate => candidate.assetUri === item.assetUri);
  const energy = analysis?.energy;
  const duration = analysis?.duration, peak = analysis?.peak;
  const energyPlot = energy?.length > 1 && Number.isFinite(duration) && Number.isFinite(peak) ? `<h3>Analyzed energy envelope</h3><svg class="energy" viewBox="0 0 100 60" role="img" aria-label="Analyzed energy envelope across ${duration.toFixed(2)} seconds; peak ${peak.toFixed(2)}"><polyline points="${energy.map((value, index) => `${(index * 100 / (energy.length - 1)).toFixed(2)},${(58 - Math.min(1, Math.max(0, value)) * 56).toFixed(2)}`).join(" ")}"/></svg>` : "";
  const assetFacts = analysis ? `${Number.isFinite(duration) ? fact("Duration", `${duration.toFixed(2)} s`) : ""}${Number.isFinite(analysis.channels) ? fact("Channels", analysis.channels) : ""}${Number.isFinite(analysis.sampleRate) ? fact("Sample rate", `${analysis.sampleRate} Hz`) : ""}${Number.isFinite(peak) ? fact("Peak", peak.toFixed(2)) : ""}` : "";
  const actions = [sourceLocation(system.uri, item.range, "Open source")];
  if (asset && item.assetUri) actions.push(sourceLocation(item.assetUri, null, "Open asset"));
  if (isCue) actions.unshift(`<button type="button" data-play="${escapeHtml(item.id)}" ${canPlay ? "" : "disabled"} title="${canPlay ? "Audition through Tua audioSystem" : "Audition requires an existing static audio asset"}">Play cue</button>`);
  else actions.unshift(`<button type="button" data-play-scene="${escapeHtml(item.id)}" ${canAuditionScene(system, item) ? "" : "disabled"} title="${canAuditionScene(system, item) ? "Audition authored static layers" : "Scene audition requires static streamed layers and authored timing"}">Play scene</button>`);
  const routing = isCue ? `<dl class="facts">${fact("Source", sourceLabel)}${fact("Asset", asset ? item.assetState || "not analyzed" : "not statically known")}${assetFacts}${fact("Bus", property(item, "bus"))}${fact("Effect", property(item, "effect"))}${fact("Gain", property(item, "volume"))}${fact("Pitch", property(item, "pitch"))}${fact("Cue voice limit", property(item, "maxVoices"))}${fact("Overflow", property(item, "overflow"))}${fact("Fade in", property(item, "fadeIn"))}</dl>${energyPlot}` : "";
  const layerRows = (item.layers || []).map(layer => `<li>${sourceLocation(system.uri, layer.range, layer.name)}<span class="muted">${escapeHtml(layer.source?.assetPath || layer.source?.kind || "unknown source")}</span></li>`).join("");
  const destinations = (system.sceneItems || []).filter(scene => scene.id !== item.id);
  const transition = destinations.length ? `<div class="transition"><label>Transition to<select id="transition-target">${destinations.map(scene => `<option value="${escapeHtml(scene.id)}">${escapeHtml(scene.name)}</option>`).join("")}</select></label><label>At<select id="transition-quantize"><option value="immediate">Immediately</option><option value="beat">Next beat</option><option value="bar">Next bar</option></select></label><button type="button" data-transition="${escapeHtml(item.id)}">Test transition</button></div>` : "";
  const scene = !isCue ? `<dl class="facts">${fact("Authored BPM", property(item, "bpm"))}${fact("Time signature", property(item, "signature"))}${fact("Loop region", property(item, "loopRegion"))}${fact("Analysis asset", property(item, "analysis"))}</dl><h3>Layers in this scene</h3><ul class="layers">${layerRows || '<li class="muted">No static layers</li>'}</ul>${transition}<p class="muted">Layer rows describe the authored scene. Audition uses static streamed layers and authored tempo; there is no live game clock in this view.</p>` : "";
  return `<section class="detail" aria-label="${escapeHtml(isCue ? "Selected cue" : "Selected music scene")}"><div class="eyeline">${escapeHtml(system.displayPath || system.uri)} · ${isCue ? "Cue" : "Music scene"}</div><h2>${escapeHtml(item.name)}</h2><div class="actions">${actions.join("")}</div>${routing}${scene}</section>`;
}
function analysisSection(analyses) {
  if (!analyses.length) return '<section class="analysis"><h2>Generated tempo analysis</h2><p class="muted">Run tua check or tua build to generate asset analysis. Analysis is a suggestion, not the authored score.</p></section>';
  return `<section class="analysis"><h2>Generated tempo analysis</h2><ul class="analysis-list">${analyses.map(item => `<li>${sourceLocation(item.assetUri, null, item.path)} · ${item.bpm == null ? "BPM unresolved" : `BPM ${Number(item.bpm).toFixed(2)}`} · confidence ${Number(item.confidence || 0).toFixed(2)}</li>`).join("")}</ul></section>`;
}
function html(webview, graph, analyses = [], state = {}) {
  const systems = graph?.systems || [];
  const mode = state.mode === "scenes" ? "scenes" : "cues";
  const items = systems.flatMap(system => ((mode === "cues" ? system.cueItems : system.sceneItems) || []).map(item => ({ system, item })));
  const selected = items.find(entry => entry.item.id === state.selectedId) || items[0];
  const declaration = systems[0] ? sourceLocation(systems[0].uri, systems[0].range, "Open declaration") : "";
  const body = `<header class="toolbar"><h1>Audio Workbench</h1><span class="eyeline">Static configuration${state.revision != null ? ` · revision ${escapeHtml(String(state.revision))}` : ""}</span>${declaration}<button id="reload" type="button" class="quiet">Refresh</button><button id="stop" type="button" class="quiet">Stop audition</button></header>
    <div class="modes" role="tablist" aria-label="Audio view"><button type="button" role="tab" data-mode="cues" aria-selected="${mode === "cues"}">Cues</button><button type="button" role="tab" data-mode="scenes" aria-selected="${mode === "scenes"}">Music scenes</button></div>
    ${systems.length ? `<div class="workspace"><section class="index" aria-label="${mode === "cues" ? "Cue list" : "Music scene list"}"><label for="audio-search">Find ${mode === "cues" ? "cue" : "scene"}</label><input id="audio-search" type="search" autocomplete="off"><ul class="item-list">${itemRows(items, selected?.item.id)}</ul>${items.length ? "" : '<p class="muted">No static declarations in this mode.</p>'}</section>${selectedDetail(selected, mode, analyses)}</div>` : '<p class="notice">No static audioSystem declarations found in this project.</p>'}
    <p id="audition-status" role="status" class="muted">Audition is stopped. Play uses an isolated LÖVE process; editing the declaration stops playback and refreshes this view.</p>${analysisSection(analyses)}`;
  const script = `document.getElementById("reload").addEventListener("click",()=>vscode.postMessage({command:"reload"}));
    document.getElementById("stop").addEventListener("click",()=>vscode.postMessage({command:"stop"}));
    document.querySelectorAll("[data-mode]").forEach(button=>button.addEventListener("click",()=>vscode.postMessage({command:"mode",mode:button.dataset.mode})));
    document.querySelectorAll("[data-select]").forEach(button=>button.addEventListener("click",()=>vscode.postMessage({command:"select",id:button.dataset.select})));
    document.querySelectorAll("[data-play]").forEach(button=>button.addEventListener("click",()=>vscode.postMessage({command:"play",id:button.dataset.play})));
    document.querySelectorAll("[data-play-scene]").forEach(button=>button.addEventListener("click",()=>vscode.postMessage({command:"playScene",id:button.dataset.playScene})));
    document.querySelectorAll("[data-transition]").forEach(button=>button.addEventListener("click",()=>vscode.postMessage({command:"transition",id:button.dataset.transition,targetId:document.getElementById("transition-target").value,quantize:document.getElementById("transition-quantize").value})));
    document.querySelectorAll("[data-uri]").forEach(button=>button.addEventListener("click",()=>vscode.postMessage({command:"openLocation",uri:button.dataset.uri,range:{start:{line:Number(button.dataset.line),character:Number(button.dataset.character)},end:{line:Number(button.dataset.line),character:Number(button.dataset.character)}}})));
    document.getElementById("audio-search")?.addEventListener("input",event=>{const query=event.target.value.toLocaleLowerCase();document.querySelectorAll(".item-list li").forEach(row=>{row.hidden=!row.textContent.toLocaleLowerCase().includes(query)})});
    window.addEventListener("message",event=>{if(event.data?.command==="auditionStatus") document.getElementById("audition-status").textContent=event.data.message});`;
  return documentHtml(webview, body, script);
}

function registerAudioDevex({ getClient, getServerCommand }) {
  let panel, currentGraph, currentAnalyses = [];
  let refreshTimer;
  const state = { mode: "cues", selectedId: undefined, revision: undefined };
  const renderGate = createRenderGate();
  const audition = createAudition({ getServerCommand, onStatus: message => panel?.webview.postMessage({ command: "auditionStatus", message }) });
  const loadAnalysis = async () => {
    if (!vscode.workspace?.findFiles) return [];
    const files = await vscode.workspace.findFiles("**/audio-index.json", "**/{.git,.tua,node_modules,build}/**");
    const analyses = [];
    for (const uri of files) {
      try {
        const data = JSON.parse(Buffer.from(await vscode.workspace.fs.readFile(uri)).toString("utf8"));
        const folder = vscode.workspace.getWorkspaceFolder(uri);
        for (const item of Array.isArray(data) ? data : []) {
          if (typeof item.path !== "string") continue;
          const energy = Array.isArray(item.energy) ? item.energy : [];
          const bins = Math.min(80, energy.length);
          const sampledEnergy = [];
          for (let index = 0; index < bins; index++) {
            let maximum = 0;
            for (let at = Math.floor(index * energy.length / bins); at < Math.floor((index + 1) * energy.length / bins); at++) {
              if (Number.isFinite(energy[at])) maximum = Math.max(maximum, energy[at]);
            }
            sampledEnergy.push(maximum);
          }
          const assetPath = folder ? path.resolve(folder.uri.fsPath, item.path) : undefined;
          if (assetPath && !isPathInside(folder.uri.fsPath, assetPath)) continue;
          analyses.push({ path: item.path, bpm: item.bpm, confidence: item.confidence,
            duration: item.duration, channels: item.channels, sampleRate: item.sampleRate, peak: item.peak,
            energy: sampledEnergy,
            assetUri: assetPath ? vscode.Uri.file(assetPath).toString() : uri.toString() });
        }
      } catch (_) { /* A partial index is retried on refresh. */ }
    }
    return analyses;
  };
  const hydrate = async graph => {
    for (const system of graph?.systems || []) {
      system.displayPath = vscode.workspace.asRelativePath(vscode.Uri.parse(system.uri), false);
      const assetItems = [...(system.cueItems || []), ...(system.sceneItems || []).flatMap(scene => scene.layers || [])]
        .filter(item => item.source?.kind === "asset");
      if (!assetItems.length) continue;
      const layout = await projectLayoutForUri(vscode.Uri.parse(system.uri), getClient);
      for (const item of assetItems) {
        const asset = item.source?.kind === "asset" ? item.source.assetPath : undefined;
        if (!asset || !layout?.sourceDirectory) continue;
        const absolute = path.resolve(layout.sourceDirectory, asset);
        if (!isPathInside(layout.sourceDirectory, absolute)) { item.assetState = "outside source root"; continue; }
        const uri = vscode.Uri.file(absolute);
        item.assetUri = uri.toString();
        try { await vscode.workspace.fs.stat(uri); item.assetState = "present"; }
        catch { item.assetState = "missing"; }
      }
    }
    return graph;
  };
  const paint = () => { if (panel) panel.webview.html = html(panel.webview, currentGraph, currentAnalyses, state); };
  const render = async () => {
    const client = getClient();
    if (!client || !panel) return;
    const currentPanel = panel, request = renderGate.begin();
    currentPanel.webview.html = loadingHtml(currentPanel.webview);
    try {
      const response = await client.sendRequest("tua/audioGraph", null);
      const graph = await hydrate(response?.graph);
      const analyses = await loadAnalysis();
      if (!renderGate.isCurrent(request) || panel !== currentPanel) return;
      state.revision = response?.analysisRevision;
      currentGraph = graph;
      currentAnalyses = analyses;
      paint();
    } catch (error) {
      if (renderGate.isCurrent(request) && panel === currentPanel) currentPanel.webview.html = errorHtml(currentPanel.webview, error);
    }
  };
  const command = vscode.commands.registerCommand(COMMANDS.showAudioExplorer, async () => {
    if (!getClient()) return vscode.window.showWarningMessage("The Tua language server is not running.");
    if (!panel) {
      panel = vscode.window.createWebviewPanel("tuaAudioExplorer", "Tua Audio Workbench", vscode.ViewColumn.Beside, { enableScripts: true, retainContextWhenHidden: true });
      panel.onDidDispose(() => { renderGate.invalidate(); audition.stop(); panel = undefined; });
      panel.webview.onDidReceiveMessage(async message => {
        if (message?.command === "reload") return render();
        if (message?.command === "stop") return audition.stop();
        if (message?.command === "mode" && ["cues", "scenes"].includes(message.mode)) { state.mode = message.mode; state.selectedId = undefined; return paint(); }
        if (message?.command === "select" && typeof message.id === "string") { state.selectedId = message.id; return paint(); }
        if (message?.command === "play" && typeof message.id === "string") {
          const system = currentGraph?.systems?.find(system => system.cueItems?.some(item => item.id === message.id));
          const cue = system?.cueItems?.find(item => item.id === message.id);
          if (!cue || !canAudition(system, cue)) return;
          try { await audition.play({ system, cue }); }
          catch (error) { vscode.window.showErrorMessage(`Audio audition failed: ${error.message}`); }
          return;
        }
        if (["playScene", "transition"].includes(message?.command) && typeof message.id === "string") {
          const system = currentGraph?.systems?.find(system => system.sceneItems?.some(item => item.id === message.id));
          const scene = system?.sceneItems?.find(item => item.id === message.id);
          const target = system?.sceneItems?.find(item => item.id === message.targetId);
          if (!scene || (message.command === "transition" && !target)) return;
          try { await audition.playScene({ system, scene, target, quantize: message.quantize || "immediate" }); }
          catch (error) { vscode.window.showErrorMessage(`Audio audition failed: ${error.message}`); }
          return;
        }
        return openLocationMessage(message);
      });
    } else panel.reveal(vscode.ViewColumn.Beside, false);
    await render();
  });
  const sourceChanges = vscode.workspace.onDidChangeTextDocument(event => {
    if (!panel || !currentGraph?.systems?.some(system => system.uri === event.document.uri.toString())) return;
    audition.stop();
    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(render, 250);
  });
  const assetChanges = vscode.workspace.createFileSystemWatcher("**/*.{wav,ogg,mp3,flac}");
  const scheduleAssetRefresh = () => {
    if (!panel) return;
    audition.stop();
    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(render, 250);
  };
  const assetSubscriptions = [assetChanges.onDidChange(scheduleAssetRefresh), assetChanges.onDidCreate(scheduleAssetRefresh), assetChanges.onDidDelete(scheduleAssetRefresh)];
  return vscode.Disposable.from(command, sourceChanges, assetChanges, ...assetSubscriptions,
    { dispose() { clearTimeout(refreshTimer); renderGate.invalidate(); audition.stop(); panel?.dispose(); } });
}
module.exports = { registerAudioDevex, audioExplorerHtml: html, audioExplorerErrorHtml: errorHtml };
