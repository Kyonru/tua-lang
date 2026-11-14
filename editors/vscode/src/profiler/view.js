"use strict";
const { randomBytes } = require("node:crypto");
const vscode = require("vscode");

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>"']/gu, character => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[character]);
}

function millis(value) { return Number.isFinite(value) ? `${value.toFixed(2)} ms` : "—"; }

function renderProfileHtml(model, nonce) {
  const sampleRows = model.stacks.map((item, index) => {
    const location = item.location;
    const name = location ? `${location.sourcePath}:${location.sourceLine}` : item.stack.split("\n")[0];
    return `<tr><td><button type="button" data-source="${index}" ${location ? "" : "disabled"} aria-label="Open Tua source for sample ${index + 1}">${escapeHtml(name || "unknown frame")}</button>${location ? ` · <button type="button" data-generated="${index}" aria-label="Open generated Lua for sample ${index + 1}">Lua</button>` : ""}<small>${escapeHtml(item.stack.trim().split("\n").slice(0, 3).join(" ← "))}</small></td><td>${item.count}</td><td>${(item.share * 100).toFixed(1)}%</td></tr>`;
  }).join("");
  const frameRows = model.slowFrames.map(frame => `<tr><td>${frame.index}</td><td>${millis(frame.wallMs)}</td><td>${millis(frame.updateMs)}</td><td>${millis(frame.drawMs)}</td><td>${frame.wallMs > model.budgetMs ? "Over" : "Within"}</td><td>${Number.isFinite(frame.drawCalls) ? frame.drawCalls : "—"}</td><td>${Number.isFinite(frame.gcSamples) ? frame.gcSamples : "—"}</td><td>${Number.isFinite(frame.luaKb) ? (frame.luaKb / 1024).toFixed(1) : "—"}</td></tr>`).join("");
  const traceRows = model.traces.map((item, index) => `<tr><td>${escapeHtml(item.kind)}</td><td>${item.source ? `<button type="button" data-trace="${index}" aria-label="Open source for ${escapeHtml(item.name)}">${escapeHtml(item.name)}</button>` : escapeHtml(item.name)}</td><td>${item.calls}</td><td>${millis(item.totalMs)}</td><td>${millis(item.worstMs)}</td></tr>`).join("");
  const traceSection = model.traces.length || model.traceError ? `<section aria-labelledby="trace-title"><h2 id="trace-title">Selected game boundaries</h2><p class="note">Only categories selected at capture start are timed. These are exact wrapper durations, including nested work; totals can overlap.</p>${model.traceError ? `<p class="warning">Named tracing unavailable: ${escapeHtml(model.traceError)}</p>` : ""}${model.droppedTraces ? `<p class="warning">${model.droppedTraces} calls exceeded the 128-boundary limit.</p>` : ""}<div class="table-wrap"><table><thead><tr><th>Category</th><th>Boundary</th><th>Calls</th><th>Total</th><th>Worst</th></tr></thead><tbody>${traceRows || '<tr><td colspan="5">No selected boundaries ran.</td></tr>'}</tbody></table></div></section>` : "";
  const recentSection = model.errorMessage && model.recentTraces.length ? `<section aria-labelledby="recent-title"><h2 id="recent-title">Recent traced boundaries before failure</h2><p class="note">These are the last ${model.recentTraces.length} selected calls that completed; they are context, not a causal trace.</p><ol>${model.recentTraces.map(item => `<li>${escapeHtml(item.kind)} · ${escapeHtml(item.name)} · ${millis(item.elapsedMs)}</li>`).join("")}</ol></section>` : "";
  const failure = model.errorMessage ? `<section aria-labelledby="failure-title"><h2 id="failure-title">Game failure</h2><p>${escapeHtml(model.errorMessage)}</p>${model.failureLocation ? '<button type="button" id="failure-source">Open Tua source</button>' : ""}</section>` : "";
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}'"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Tua performance capture</title><style>
    body{font:var(--vscode-font-size) var(--vscode-font-family);color:var(--vscode-foreground);background:var(--vscode-editor-background);margin:0;padding:20px;line-height:1.5} main{max-width:1100px;margin:auto} h1{font-size:1.45rem;margin:0 0 8px} h2{font-size:1.1rem;margin:28px 0 8px} p{margin:8px 0} .summary{display:flex;flex-wrap:wrap;gap:18px;border-block:1px solid var(--vscode-panel-border);padding:14px 0;margin:16px 0}.summary div{min-width:115px}.summary strong{display:block;font-size:1.3rem;font-variant-numeric:tabular-nums}.summary span,small,.note{color:var(--vscode-descriptionForeground)} table{width:100%;border-collapse:collapse;font-variant-numeric:tabular-nums} th,td{text-align:left;padding:7px 8px;border-bottom:1px solid var(--vscode-panel-border);vertical-align:top} th{position:sticky;top:0;background:var(--vscode-editor-background)} tr:hover{background:var(--vscode-list-hoverBackground)} button{font:inherit;color:var(--vscode-textLink-foreground);background:none;border:0;padding:0;text-align:left;cursor:pointer}button:hover{text-decoration:underline}button:disabled{color:var(--vscode-descriptionForeground);cursor:default}button:focus-visible{outline:2px solid var(--vscode-focusBorder);outline-offset:2px}small{display:block;max-width:70ch;overflow-wrap:anywhere} .table-wrap{overflow:auto;max-height:42vh} .warning{color:var(--vscode-editorWarning-foreground)}
  </style></head><body><main><h1>LÖVE performance capture</h1><p class="note">${escapeHtml(model.reason)} · ${millis(model.durationMs)} captured. Frame wall time includes the game loop and any sleep; CPU rows and GC markers are statistical samples, not exact function durations.</p><div class="summary" aria-label="Capture summary"><div><strong>${millis(model.p50)}</strong><span>retained frame p50</span></div><div><strong>${millis(model.p95)}</strong><span>retained frame p95</span></div><div><strong>${millis(model.worst)}</strong><span>worst retained frame</span></div><div><strong>${model.sampleCount}</strong><span>CPU samples</span></div><div><strong>${model.missedBudget} / ${model.retainedFrames}</strong><span>over ${millis(model.budgetMs)} budget</span></div></div>${model.retainedFrames < model.frameCount ? `<p class="warning">Showing the latest ${model.retainedFrames} of ${model.frameCount} frames.</p>` : ""}${model.droppedSamples ? `<p class="warning">${model.droppedSamples} samples exceeded the 512-stack limit.</p>` : ""}${model.profileError ? `<p class="warning">CPU sampling unavailable: ${escapeHtml(model.profileError)}</p>` : ""}${failure}${recentSection}<section aria-labelledby="frames-title"><h2 id="frames-title">Slowest retained frames</h2><div class="table-wrap"><table><thead><tr><th>Frame</th><th>Wall</th><th>Update</th><th>Draw</th><th>Budget</th><th>Draw calls</th><th>GC samples</th><th>Lua MB</th></tr></thead><tbody>${frameRows || '<tr><td colspan="8">No frames were captured.</td></tr>'}</tbody></table></div></section><section aria-labelledby="cpu-title"><h2 id="cpu-title">Sampled CPU stacks</h2><p class="note">Samples are sorted by their top stack path. Click a mapped location to open Tua source; LuaJIT inlining can make line attribution approximate.</p><div class="table-wrap"><table><thead><tr><th>Stack and source</th><th>Samples</th><th>Share</th></tr></thead><tbody>${sampleRows || '<tr><td colspan="3">No CPU samples were captured.</td></tr>'}</tbody></table></div></section>${traceSection}</main><script nonce="${nonce}">const vscode=acquireVsCodeApi();document.addEventListener('click',event=>{const button=event.target.closest('button[data-source]');if(button)vscode.postMessage({type:'source',index:Number(button.dataset.source)});else if(event.target.closest('button[data-trace]'))vscode.postMessage({type:'trace',index:Number(event.target.closest('button[data-trace]').dataset.trace)});else if(event.target.closest('button[data-generated]'))vscode.postMessage({type:'generated',index:Number(event.target.closest('button[data-generated]').dataset.generated)});else if(event.target.id==='failure-source')vscode.postMessage({type:'failure'});});</script></body></html>`;
}

function showProfile(model) {
  const panel = vscode.window.createWebviewPanel("tuaProfile", "Tua Performance Capture", vscode.ViewColumn.Beside, { enableScripts: true });
  const nonce = randomBytes(16).toString("base64");
  panel.webview.html = renderProfileHtml(model, nonce);
  panel.webview.onDidReceiveMessage(async message => {
    let location;
    if (message?.type === "source" && Number.isSafeInteger(message.index)) location = model.stacks[message.index]?.location;
    if (message?.type === "failure") location = model.failureLocation;
    if (message?.type === "trace" && Number.isSafeInteger(message.index)) location = model.traces[message.index]?.source;
    if (message?.type === "generated" && Number.isSafeInteger(message.index)) {
      const sample = model.stacks[message.index]?.location;
      if (sample?.generatedPath && Number.isSafeInteger(sample.generatedLine)) {
        const document = await vscode.workspace.openTextDocument(vscode.Uri.file(sample.generatedPath));
        const line = Math.max(0, Math.min(document.lineCount - 1, sample.generatedLine - 1));
        await vscode.window.showTextDocument(document, { preview: true, selection: new vscode.Range(line, 0, line, 0) });
      }
      return;
    }
    if (!location?.sourcePath || !Number.isSafeInteger(location.sourceLine)) return;
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(location.sourcePath));
    const line = Math.max(0, Math.min(document.lineCount - 1, location.sourceLine - 1));
    await vscode.window.showTextDocument(document, { preview: true, selection: new vscode.Range(line, 0, line, 0) });
  });
  return panel;
}

module.exports = { renderProfileHtml, showProfile };
