"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { configuredLoveCommand } = require("../love/run-project");
const { ensureExperimentalPreviewEnabled } = require("../shared/preview-gate");
const { escapeHtml, webviewDocument } = require("../shared/webview");
const { DEFAULTS, createProceduralRunner, validateOptions } = require("./runner");

function proceduralPreviewHtml(webview, options, result, status) {
  const selected = (name) => options.algorithm === name ? " selected" : "";
  const controls = `<form id="controls"><label>Algorithm<select name="algorithm"><option value="sidewinder"${selected("sidewinder")}>Sidewinder</option><option value="binaryTree"${selected("binaryTree")}>Binary tree</option><option value="arena"${selected("arena")}>Walled arena</option></select></label><label>Seed<input name="seed" type="number" min="1" max="1000000000" step="1" value="${options.seed}"></label><label>Width<input name="width" type="number" min="11" max="41" step="2" value="${options.width}"></label><label>Height<input name="height" type="number" min="11" max="41" step="2" value="${options.height}"></label><label>Sight radius<input name="radius" type="number" min="1" max="20" step="1" value="${options.radius}"></label><button type="submit">Generate</button></form>`;
  let content = `<p class="status" role="status">${escapeHtml(status)}</p>`;
  if (result) {
    const route = new Set(result.path.map(({ x, y }) => `${x},${y}`));
    let seen = 0, walkable = 0;
    const rows = result.map.map((row, y) => {
      const cells = [...row].map((value, x) => {
        const px = x + 1, py = y + 1;
        const floor = value === "1";
        const inSight = result.visible[y][x] === "1";
        if (floor) walkable++;
        if (inSight) seen++;
        const start = px === 2 && py === 2;
        const goal = px === result.width - 1 && py === result.height - 1;
        const onRoute = route.has(`${px},${py}`);
        const kind = start ? "start" : goal ? "goal" : onRoute ? "route" : floor ? "floor" : "wall";
        const label = `${px}, ${py}: ${floor ? "floor" : "wall"}${inSight ? ", visible" : ""}${onRoute ? ", on route" : ""}${start ? ", start" : ""}${goal ? ", goal" : ""}`;
        return `<button type="button" class="cell ${kind}${inSight ? " visible" : ""}" role="gridcell" aria-label="${label}" tabindex="-1" data-x="${px}" data-y="${py}">${start ? "S" : goal ? "G" : onRoute ? "·" : floor ? "" : "■"}</button>`;
      }).join("");
      return `<div class="row" role="row">${cells}</div>`;
    }).join("");
    content = `<div class="summary"><strong>${result.path.length ? `${result.path.length - 1} ${result.path.length === 2 ? "step" : "steps"}` : "No route"}</strong><span>${walkable} walkable cells</span><span>${seen} visible cells</span><span>Generated ${escapeHtml(result.algorithm)} · seed ${result.seed} · ${result.width} × ${result.height}</span></div><p class="key">S start · G goal · dotted cells route · outlined cells visible from start</p><div class="map-scroll" role="region" aria-label="Generated map; use arrow keys to inspect cells" tabindex="0"><div class="map" role="grid" aria-label="${escapeHtml(result.algorithm)} map, ${result.width} by ${result.height}" aria-rowcount="${result.height}" aria-colcount="${result.width}">${rows}</div></div><output id="cell-detail">Choose a cell to inspect its coordinates.</output><p class="note">This is a seeded sample using Tua’s bundled Rot runtime. It does not read or run your game project.</p>`;
  }
  return webviewDocument(webview, {
    title: "Procedural Map Preview",
    body: `<header><h1>Procedural map</h1><p>Inspect maze shape, route, and sight from the start.</p></header><main>${controls}<p id="pending-sample" class="status" role="status" hidden></p>${content}</main>`,
    styles: `body{margin:0;color:var(--vscode-foreground);background:var(--vscode-editor-background);font-family:var(--vscode-font-family);font-size:var(--vscode-font-size)}header,main{padding:16px 20px}header{border-bottom:1px solid var(--vscode-panel-border)}h1{margin:0 0 4px;font-size:1.25em}p{margin:4px 0 12px}form{display:flex;flex-wrap:wrap;align-items:end;gap:12px;margin-bottom:16px}label{display:grid;gap:4px}input,select{box-sizing:border-box;min-width:90px;padding:5px;color:var(--vscode-input-foreground);background:var(--vscode-input-background);border:1px solid var(--vscode-input-border)}button[type=submit]{padding:6px 12px;color:var(--vscode-button-foreground);background:var(--vscode-button-background);border:0;cursor:pointer}button:focus-visible,input:focus-visible,select:focus-visible,.map-scroll:focus-visible{outline:2px solid var(--vscode-focusBorder);outline-offset:2px}.status{color:var(--vscode-descriptionForeground)}.status[hidden]{display:none}.summary{display:flex;flex-wrap:wrap;gap:16px;align-items:baseline;margin-bottom:8px}.summary strong{font-size:1.3em}.key,.note{color:var(--vscode-descriptionForeground)}.map-scroll{max-width:100%;overflow:auto;padding:8px 0 12px}.map{width:max-content;border:1px solid var(--vscode-panel-border)}.row{display:flex}.cell{display:grid;place-items:center;width:19px;height:19px;padding:0;border:1px solid var(--vscode-panel-border);font-size:12px;line-height:1;color:var(--vscode-foreground);background:var(--vscode-editor-background);cursor:crosshair}.cell.wall{background:var(--vscode-editorWidget-background)}.cell.route{color:var(--vscode-charts-orange);font-weight:bold}.cell.visible{border-color:var(--vscode-charts-blue)}.cell.start,.cell.goal{background:var(--vscode-editor-selectionBackground);font-weight:bold}.cell:focus-visible{outline:2px solid var(--vscode-focusBorder);outline-offset:-2px}output{display:block;min-height:1.5em;margin:8px 0}.vscode-high-contrast .cell{border-color:var(--vscode-contrastBorder)}.vscode-high-contrast .cell.visible{outline:1px solid var(--vscode-focusBorder);outline-offset:-2px}@media (prefers-reduced-motion:reduce){*{scroll-behavior:auto!important}}`,
    script: `const form=document.getElementById("controls");form.addEventListener("submit",event=>{event.preventDefault();const data=new FormData(form);const pending=document.getElementById("pending-sample");pending.textContent="Generating seed "+data.get("seed")+(document.querySelector(".map")?"; the map below is the previous sample.":"; waiting for a sample.");pending.hidden=false;vscode.postMessage({command:"generate",options:{algorithm:data.get("algorithm"),seed:Number(data.get("seed")),width:Number(data.get("width")),height:Number(data.get("height")),radius:Number(data.get("radius"))}})});const cells=[...document.querySelectorAll(".cell")];const detail=document.getElementById("cell-detail");function select(cell){if(!cell)return;cells.forEach(item=>item.tabIndex=-1);cell.tabIndex=0;cell.focus();if(detail)detail.textContent=cell.getAttribute("aria-label")||""}cells.forEach(cell=>{cell.addEventListener("click",()=>select(cell));cell.addEventListener("keydown",event=>{const dx=event.key==="ArrowRight"?1:event.key==="ArrowLeft"?-1:0;const dy=event.key==="ArrowDown"?1:event.key==="ArrowUp"?-1:0;if(!dx&&!dy)return;event.preventDefault();select(document.querySelector('.cell[data-x="'+(Number(cell.dataset.x)+dx)+'"][data-y="'+(Number(cell.dataset.y)+dy)+'"]'))})});if(cells[0])cells[0].tabIndex=0;`,
  });
}

function registerProceduralPreview({ configurationTarget, getServerCommand }) {
  const runner = createProceduralRunner({ getServerCommand, getLoveCommand: configuredLoveCommand });
  let panel, generation = 0, options = DEFAULTS, result;
  const generate = async (input) => {
    try {
      options = validateOptions(input);
    } catch (error) {
      generation++;
      runner.stop();
      result = undefined;
      if (panel) panel.webview.html = proceduralPreviewHtml(panel.webview, options, result, error.message);
      return;
    }
    const current = ++generation;
    result = undefined;
    if (!vscode.workspace.isTrusted) {
      panel.webview.html = proceduralPreviewHtml(panel.webview, options, undefined, "Trust this workspace to run an isolated procedural sample.");
      return;
    }
    panel.webview.html = proceduralPreviewHtml(panel.webview, options, undefined, "Building an isolated sample…");
    try {
      const next = await runner.generate(options);
      if (current !== generation || !panel) return;
      result = next;
      panel.webview.html = proceduralPreviewHtml(panel.webview, options, result, "Generated with the bundled Rot runtime.");
    } catch (error) {
      if (current !== generation || !panel) return;
      panel.webview.html = proceduralPreviewHtml(panel.webview, options, undefined, error.message);
    }
  };
  const command = vscode.commands.registerCommand(COMMANDS.previewProceduralMap, async () => {
    if (!(await ensureExperimentalPreviewEnabled(configurationTarget, "Tua procedural map previews"))) return;
    if (!panel) {
      panel = vscode.window.createWebviewPanel("tuaProceduralMapPreview", "Tua Procedural Map", vscode.ViewColumn.Beside, { enableScripts: true, retainContextWhenHidden: true });
      panel.webview.onDidReceiveMessage((message) => { if (message?.command === "generate") void generate(message.options); });
      panel.onDidDispose(() => { generation++; runner.stop(); panel = undefined; });
    } else panel.reveal(vscode.ViewColumn.Beside, false);
    await generate(options);
  });
  return vscode.Disposable.from(command, { dispose() { generation++; runner.stop(); panel?.dispose(); } });
}

module.exports = { proceduralPreviewHtml, registerProceduralPreview };
