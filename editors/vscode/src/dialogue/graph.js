"use strict";

const { escapeHtml, jsonForHtml } = require("../shared/webview");
const { createGraphViewport } = require("../shared/graph-viewport");

const ROUTE_LABELS = {
  choice: "Choice", fallback: "No available choices", wait: "After wait",
  branchTrue: "Condition true", branchFalse: "Condition false",
  actionSuccess: "Action succeeds", actionFailure: "Action fails",
};

// Source order is deliberate: disconnected sections and cycles remain visible.
// This is geometry over compiler facts, not a reachability analysis.
function dialogueGraphLayout(preview) {
  const nodes = preview.sections.slice(0, 40).map((section, index) => ({
    ...section, index, y: 20 + index * 100,
  }));
  const labels = new Map();
  preview.sections.forEach((section, index) => {
    const matches = labels.get(section.label) || [];
    matches.push(index);
    labels.set(section.label, matches);
  });
  const lanes = [];
  const edges = [];
  let omitted = 0;
  for (const route of preview.routes) {
    const targets = labels.get(route.target);
    if (!nodes[route.section] || targets?.length !== 1 || !nodes[targets[0]] || edges.length >= 80) {
      omitted++;
      continue;
    }
    const source = nodes[route.section];
    const target = nodes[targets[0]];
    const start = source.y + 20;
    const end = target.y + 50;
    const low = Math.min(start, end) - 16;
    const high = Math.max(start, end) + 16;
    let lane = lanes.findIndex(intervals => intervals.every(([a, b]) => high < a || low > b));
    if (lane < 0) lane = lanes.push([]) - 1;
    lanes[lane].push([low, high]);
    const x = 300 + lane * 36;
    edges.push({
      ...route, number: edges.length + 1, x, y: (start + end) / 2,
      label: `${source.label} → ${route.target}: ${ROUTE_LABELS[route.kind] || route.kind}`,
      path: `M 260 ${start} H ${x} V ${end} H 260`,
    });
  }
  return { nodes, edges, omitted, width: Math.max(360, 340 + lanes.length * 36), height: nodes.length * 100 + 20 };
}

function dialogueGraph(preview, link, uri = "") {
  if (!preview?.sections?.length) return { html: "", styles: "", script: "" };
  const graph = dialogueGraphLayout(preview);
  const positions = graph.nodes.map(node => `.dialogue-node-${node.index}{top:${node.y}px}`).join("")
    + graph.edges.map(edge => `.dialogue-edge-${edge.number}{left:${edge.x - 12}px;top:${edge.y - 12}px}`).join("");
  const nodes = graph.nodes.map(node => `<div class="dialogue-node dialogue-node-${node.index}" title="${escapeHtml(node.label)}">${link(node.label, node.range)}<small>${node.index === 0 ? "Default entry" : node.action ? "Action" : node.wait != null ? "Wait" : "Section"}</small></div>`).join("");
  const paths = graph.edges.map(edge => `<path id="dialogue-path-${edge.number}" d="${edge.path}" marker-end="url(#dialogue-arrow)"/>`).join("");
  const badges = graph.edges.map(edge => `<button class="dialogue-edge dialogue-edge-${edge.number}" data-route="${edge.number}" aria-label="${escapeHtml(edge.label)}" title="${escapeHtml(edge.label)}">${edge.number}</button>`).join("");
  const routes = graph.edges.map(edge => `<li id="dialogue-route-${edge.number}" data-route="${edge.number}">${link(edge.label, edge.range)}</li>`).join("");
  return {
    html: `<details class="dialogue-map"><summary>Conversation map · ${graph.nodes.length} sections · ${graph.edges.length} routes</summary>
      <p class="meta">Authored routes in source order. Arrows point to destinations. Select a route number to find its source link below. Mock results do not remove routes.</p>
      <div class="dialogue-zoom" role="group" aria-label="Conversation map zoom"><button id="dialogue-zoom-out" aria-label="Zoom out">−</button><output id="dialogue-zoom-level">100%</output><button id="dialogue-zoom-in" aria-label="Zoom in">+</button><button id="dialogue-zoom-reset">Reset zoom</button><button id="dialogue-zoom-fit">Fit</button></div>
      <div id="dialogue-graph-scroll" tabindex="0" role="region" aria-label="Conversation map; drag to pan, Control and wheel or plus and minus keys to zoom">
        <div id="dialogue-graph"><svg width="${graph.width}" height="${graph.height}" aria-hidden="true"><defs><marker id="dialogue-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z"/></marker></defs>${paths}</svg>${nodes}${badges}</div>
      </div>
      ${graph.omitted || preview.sections.length > graph.nodes.length ? '<p class="meta">Map limited to 40 sections and 80 routes. Routes with missing, ambiguous or undisplayed destinations are omitted. Use the outline and Problems for full source details.</p>' : ""}
      ${routes ? `<ol class="dialogue-route-list" aria-label="Map route source links">${routes}</ol>` : '<p>No drawable routes.</p>'}
      </details>`,
    styles: `
      .dialogue-map{margin:24px 0}.dialogue-map>summary{font-weight:600;color:var(--vscode-foreground)}
      .dialogue-zoom{display:flex;gap:8px;align-items:center;flex-wrap:wrap;margin:12px 0}
      .dialogue-zoom output{min-width:4ch;font-variant-numeric:tabular-nums}
      #dialogue-graph-scroll{height:360px;overflow:auto;border:1px solid var(--vscode-contrastBorder,var(--vscode-panel-border));touch-action:none}
      #dialogue-graph-scroll:focus-visible{outline:2px solid var(--vscode-focusBorder);outline-offset:2px}
      #dialogue-graph{position:relative}#dialogue-graph svg{position:absolute;inset:0;pointer-events:none}
      #dialogue-graph svg>path{fill:none;stroke:var(--vscode-descriptionForeground);stroke-width:1.5}
      #dialogue-graph svg>path.selected{stroke:var(--vscode-focusBorder);stroke-width:3}
      #dialogue-arrow path{fill:var(--vscode-descriptionForeground)}
      .dialogue-node{position:absolute;left:20px;width:240px;height:70px;box-sizing:border-box;padding:8px 12px;background:var(--vscode-editor-background);border:1px solid var(--vscode-foreground)}
      .dialogue-node .source{display:block;width:100%;overflow:hidden;white-space:nowrap;text-overflow:ellipsis}
      .dialogue-node small{color:var(--vscode-descriptionForeground)}
      .dialogue-edge{position:absolute;width:24px;height:24px;padding:0;font-size:11px}
      .dialogue-route-list li:focus-within{outline:1px solid var(--vscode-focusBorder)}
      ${positions}`,
    script: `{
      const saved = vscode.getState();
      const state = saved?.dialogueMap?.uri === ${jsonForHtml(uri)} ? saved.dialogueMap : { uri: ${jsonForHtml(uri)}, open: false, scale: 1 };
      const details = document.querySelector(".dialogue-map");
      details.open = state.open === true;
      const save = () => vscode.setState({ ...vscode.getState(), dialogueMap: state });
      details.addEventListener("toggle", () => { state.open = details.open; save(); });
      const scroll = document.getElementById("dialogue-graph-scroll");
      const graph = document.getElementById("dialogue-graph");
      const viewport = (${createGraphViewport.toString()})(scroll, graph, {
        initialScale: state.scale,
        onScale: scale => {
          document.getElementById("dialogue-zoom-level").textContent = Math.round(scale * 100) + "%";
          state.scale = scale;
          save();
        }
      });
      viewport.setSize(${graph.width}, ${graph.height});
      document.getElementById("dialogue-zoom-in").onclick = () => viewport.zoom(viewport.scale() * 1.2);
      document.getElementById("dialogue-zoom-out").onclick = () => viewport.zoom(viewport.scale() / 1.2);
      document.getElementById("dialogue-zoom-reset").onclick = () => viewport.zoom(1);
      document.getElementById("dialogue-zoom-fit").onclick = () => viewport.fit();
      function highlight(number) {
        graph.querySelectorAll("svg>path").forEach(path => path.classList.toggle("selected", path.id === "dialogue-path-" + number));
      }
      document.querySelectorAll("[data-route]").forEach(element => {
        element.addEventListener("focusin", () => highlight(element.dataset.route));
        element.addEventListener("mouseenter", () => highlight(element.dataset.route));
      });
      graph.querySelectorAll("[data-route]").forEach(button => {
        button.onclick = () => document.querySelector("#dialogue-route-" + button.dataset.route + " button").focus();
      });
    }`,
  };
}

module.exports = { dialogueGraphLayout, dialogueGraph };
