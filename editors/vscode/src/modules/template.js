"use strict";

const { escapeHtml, jsonForHtml, webviewDocument } = require("../shared/webview");

function moduleGraphLoadingHtml(webview) {
  return documentHtml(
    webview,
    "Module Graph",
    '<main class="state"><div class="spinner" aria-hidden="true"></div><strong>Loading module graph...</strong></main>',
    "",
  );
}

function moduleGraphErrorHtml(webview, error) {
  const message = escapeHtml(error instanceof Error ? error.message : String(error));

  return documentHtml(
    webview,
    "Module Graph",
    `<main class="state error"><strong>Module graph unavailable</strong><span>${message}</span><button id="reload" type="button">Reload</button></main>`,
    'document.getElementById("reload").addEventListener("click", () => vscode.postMessage({ command: "reload" }));',
  );
}

function moduleGraphHtml(webview, viewModel) {
  const graphJson = jsonForHtml(viewModel);
  const body = `
    <header class="toolbar">
      <div class="title-block">
        <h1>Module Graph</h1>
        <span id="summary" class="summary"></span>
      </div>
      <div class="tools">
        <label class="search"><span>Filter</span><input id="filter" type="search" placeholder="Modules, imports, or exports" autocomplete="off"></label>
        <div class="segments" role="group" aria-label="Module graph filter">
          <button type="button" data-mode="all" class="active">All</button>
          <button type="button" data-mode="unresolved">Unresolved</button>
          <button type="button" data-mode="cycles">Cycles</button>
          <button type="button" data-mode="roots">Roots</button>
        </div>
        <button id="reload" class="icon-button" type="button" title="Reload graph" aria-label="Reload graph">&#8635;</button>
      </div>
    </header>
    <main class="layout">
      <section class="graph-band" aria-label="Module dependency graph">
        <div class="graph-heading">
          <span>Dependency flow</span>
          <span id="revision" class="revision"></span>
        </div>
        <div id="graph-scroll" class="graph-scroll">
          <div id="graph" class="graph">
            <svg id="connections" class="connections" aria-hidden="true"></svg>
            <div id="nodes" class="nodes"></div>
          </div>
        </div>
      </section>
      <aside id="inspector" class="inspector" aria-live="polite"></aside>
    </main>`;

  const script = `
    const graphData = ${graphJson};
    const graphRoot = document.getElementById("graph");
    const nodesRoot = document.getElementById("nodes");
    const connections = document.getElementById("connections");
    const inspector = document.getElementById("inspector");
    const filterInput = document.getElementById("filter");
    const summary = document.getElementById("summary");
    const revision = document.getElementById("revision");
    const segmentButtons = Array.from(document.querySelectorAll("[data-mode]"));
    const moduleByUri = new Map(graphData.modules.map((module) => [module.uri, module]));
    let mode = "all";
    let query = "";
    let selectedUri = graphData.focusUri || graphData.modules[0]?.uri || null;
    let selectedTab = graphData.initialTab || "overview";

    revision.textContent = "analysis r" + graphData.analysisRevision;
    document.getElementById("reload").addEventListener("click", () => {
      vscode.postMessage({ command: "reload" });
    });
    filterInput.addEventListener("input", () => {
      query = filterInput.value.trim().toLowerCase();
      render();
    });
    for (const button of segmentButtons) {
      button.addEventListener("click", () => {
        mode = button.dataset.mode;
        for (const candidate of segmentButtons) {
          candidate.classList.toggle("active", candidate === button);
        }
        render();
      });
    }
    window.addEventListener("resize", debounce(drawConnections, 80));

    function moduleMatches(module) {
      if (mode === "unresolved" && module.unresolvedDependencyCount === 0) return false;
      if (mode === "cycles" && !module.inCycle) return false;
      if (mode === "roots" && !module.root) return false;
      if (!query) return true;

      const dependencies = graphData.edges.filter((edge) => edge.sourceUri === module.uri);
      return [
        module.file,
        module.sourceKind,
        ...module.exports.flatMap((item) => [item.name, item.type]),
        ...dependencies.flatMap((edge) => [edge.specifier, edge.resolution.targetFile || ""]),
      ].some((value) => String(value).toLowerCase().includes(query));
    }

    function visibleGraph() {
      const modules = graphData.modules.filter(moduleMatches);
      const moduleUris = new Set(modules.map((module) => module.uri));
      const edges = graphData.edges.filter((edge) => {
        if (!moduleUris.has(edge.sourceUri)) return false;
        if (edge.resolution.targetUri) return moduleUris.has(edge.resolution.targetUri);
        return true;
      });
      return { modules, edges };
    }

    function render() {
      const visible = visibleGraph();
      renderGraph(visible);
      renderInspector();
      const unresolved = graphData.edges.filter((edge) =>
        edge.resolution.status === "missing" || edge.resolution.status === "invalid"
      ).length;
      summary.textContent = graphData.modules.length + " modules / " + graphData.edges.length +
        " dependencies / " + unresolved + " unresolved / " + graphData.cycles.length + " cycles";
      requestAnimationFrame(drawConnections);
    }

    function renderGraph(visible) {
      nodesRoot.replaceChildren();
      const maxKnownDepth = Math.max(0, ...visible.modules.map(moduleDepth));
      const rowsByDepth = new Map();

      for (const module of visible.modules) {
        const depth = moduleDepth(module);
        const row = rowsByDepth.get(depth) || [];
        row.push({ kind: "module", module });
        rowsByDepth.set(depth, row);
      }
      for (const edge of visible.edges) {
        if (edge.resolution.targetUri) continue;
        const source = moduleByUri.get(edge.sourceUri);
        const depth = Math.min(moduleDepth(source) + 1, maxKnownDepth + 1);
        const row = rowsByDepth.get(depth) || [];
        row.push({ kind: "unresolved", edge });
        rowsByDepth.set(depth, row);
      }

      const depths = Array.from(rowsByDepth.keys()).sort((left, right) => left - right);
      if (depths.length === 0) {
        nodesRoot.append(emptyState("No modules match this view."));
        graphRoot.style.minWidth = "0";
        graphRoot.style.minHeight = "320px";
        return;
      }

      const columnWidth = 250;
      const rowHeight = 118;
      const maxRows = Math.max(...depths.map((depth) => rowsByDepth.get(depth).length));
      graphRoot.style.minWidth = Math.max(620, (Math.max(...depths) + 1) * columnWidth + 100) + "px";
      graphRoot.style.minHeight = Math.max(360, maxRows * rowHeight + 100) + "px";

      for (const depth of depths) {
        const column = element("section", "graph-column");
        column.style.left = (depth * columnWidth + 28) + "px";
        column.append(label(depth === 0 ? "Roots" : "Depth " + depth, "depth-label"));
        const entries = rowsByDepth.get(depth).sort(compareEntries);
        for (const entry of entries) {
          column.append(entry.kind === "module" ? moduleNode(entry.module) : unresolvedNode(entry.edge));
        }
        nodesRoot.append(column);
      }
    }

    function moduleDepth(module) {
      if (!module) return 0;
      if (module.inclusionPath) return module.inclusionPath.steps.length;
      if (module.root) return 0;
      return 1;
    }

    function compareEntries(left, right) {
      const leftName = left.kind === "module" ? left.module.file : left.edge.specifier;
      const rightName = right.kind === "module" ? right.module.file : right.edge.specifier;
      return leftName.localeCompare(rightName);
    }

    function moduleNode(module) {
      const button = element("button", "module-node" + (module.uri === selectedUri ? " selected" : ""));
      button.type = "button";
      button.dataset.moduleUri = module.uri;
      button.dataset.nodeUri = module.uri;
      button.title = module.file;
      const heading = element("span", "node-heading");
      heading.append(label(module.file, "node-file"), badge(module.sourceKind.toUpperCase(), "kind"));
      button.append(heading);
      const badges = element("span", "badges");
      if (module.root) badges.append(badge("root", "root"));
      if (module.inCycle) badges.append(badge("cycle", "cycle"));
      if (module.unresolvedDependencyCount > 0) {
        badges.append(badge(module.unresolvedDependencyCount + " unresolved", "unresolved"));
      }
      if (badges.childNodes.length > 0) button.append(badges);
      button.append(label(
        module.dependencyCount + " deps / " + module.reverseDependencyCount + " dependents / " +
          module.exports.length + " exports",
        "node-meta",
      ));
      button.addEventListener("click", () => {
        selectedUri = module.uri;
        render();
      });
      button.addEventListener("dblclick", () => openLocation(module.uri));
      return button;
    }

    function unresolvedNode(edge) {
      const button = element("button", "module-node unresolved-node");
      button.type = "button";
      button.dataset.unresolvedEdge = edge.id;
      button.dataset.nodeUri = "unresolved:" + edge.id;
      button.append(label(edge.specifier || "Invalid module", "node-file"));
      button.append(badge(edge.resolution.status, "unresolved"));
      button.append(label(edge.kind + " from " + edge.sourceFile, "node-meta"));
      button.addEventListener("click", () => {
        selectedUri = edge.sourceUri;
        selectedTab = "dependencies";
        render();
      });
      button.addEventListener("dblclick", () => openLocation(edge.sourceUri, edge.range));
      return button;
    }

    function drawConnections() {
      connections.replaceChildren();
      const visible = visibleGraph();
      const graphRect = graphRoot.getBoundingClientRect();
      connections.setAttribute("viewBox", "0 0 " + graphRoot.scrollWidth + " " + graphRoot.scrollHeight);
      connections.setAttribute("width", graphRoot.scrollWidth);
      connections.setAttribute("height", graphRoot.scrollHeight);

      for (const edge of visible.edges) {
        const source = graphRoot.querySelector('[data-node-uri="' + cssEscape(edge.sourceUri) + '"]');
        const targetKey = edge.resolution.targetUri || ("unresolved:" + edge.id);
        const target = graphRoot.querySelector('[data-node-uri="' + cssEscape(targetKey) + '"]');
        if (!source || !target) continue;
        const sourceRect = source.getBoundingClientRect();
        const targetRect = target.getBoundingClientRect();
        const x1 = sourceRect.right - graphRect.left;
        const y1 = sourceRect.top + sourceRect.height / 2 - graphRect.top;
        const x2 = targetRect.left - graphRect.left;
        const y2 = targetRect.top + targetRect.height / 2 - graphRect.top;
        const bend = Math.max(26, Math.abs(x2 - x1) * 0.42);
        const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
        path.setAttribute("d", "M " + x1 + " " + y1 + " C " + (x1 + bend) + " " + y1 + ", " +
          (x2 - bend) + " " + y2 + ", " + x2 + " " + y2);
        path.setAttribute("class", "connection " + (edge.inCycle ? "cycle" : edge.resolution.status));
        connections.append(path);
      }
    }

    function renderInspector() {
      inspector.replaceChildren();
      const module = moduleByUri.get(selectedUri);
      if (!module) {
        inspector.append(emptyState("Select a module to inspect its dependencies."));
        return;
      }

      const header = element("header", "inspector-header");
      const titleRow = element("div", "inspector-title-row");
      const title = element("button", "file-link");
      title.type = "button";
      title.textContent = module.file;
      title.title = "Open module";
      title.addEventListener("click", () => openLocation(module.uri));
      titleRow.append(title, badge(module.sourceKind.toUpperCase(), "kind"));
      header.append(titleRow, label(inspectorSubtitle(module), "inspector-subtitle"));
      inspector.append(header);

      const tabs = element("div", "tabs");
      const definitions = [
        ["overview", "Overview"],
        ["dependencies", "Dependencies"],
        ["dependents", "Dependents"],
        ["exports", "Exports"],
        ["why", "Why included"],
      ];
      for (const [id, titleText] of definitions) {
        const button = element("button", "tab" + (selectedTab === id ? " active" : ""));
        button.type = "button";
        button.textContent = titleText;
        button.addEventListener("click", () => {
          selectedTab = id;
          renderInspector();
        });
        tabs.append(button);
      }
      inspector.append(tabs);

      const content = element("div", "inspector-content");
      if (selectedTab === "dependencies") renderDependencies(content, module);
      else if (selectedTab === "dependents") renderDependents(content, module);
      else if (selectedTab === "exports") renderExports(content, module);
      else if (selectedTab === "why") renderWhy(content, module);
      else renderOverview(content, module);
      inspector.append(content);
    }

    function inspectorSubtitle(module) {
      const parts = [];
      if (module.root) parts.push("root candidate");
      if (module.inCycle) parts.push("cycle member");
      parts.push(module.dependencyCount + " dependencies");
      parts.push(module.reverseDependencyCount + " reverse dependencies");
      return parts.join(" / ");
    }

    function renderOverview(content, module) {
      const metrics = element("div", "metrics");
      metrics.append(
        metric(module.dependencyCount, "Dependencies"),
        metric(module.reverseDependencyCount, "Dependents"),
        metric(module.exports.length, "Exports"),
        metric(module.unresolvedDependencyCount, "Unresolved", module.unresolvedDependencyCount > 0),
      );
      content.append(metrics);

      const unresolved = outgoingEdges(module).filter((edge) =>
        edge.resolution.status === "missing" || edge.resolution.status === "invalid"
      );
      if (unresolved.length > 0) {
        content.append(sectionHeading("Unresolved modules"));
        const list = element("div", "rows");
        for (const edge of unresolved) list.append(edgeRow(edge, false));
        content.append(list);
      }

      const moduleCycles = graphData.cycles.filter((cycle) => cycle.moduleUris.includes(module.uri));
      if (moduleCycles.length > 0) {
        content.append(sectionHeading("Cycles"));
        for (const cycle of moduleCycles) {
          const chain = element("div", "cycle-chain");
          cycle.moduleUris.forEach((uri, index) => {
            if (index > 0) chain.append(label("->", "chain-arrow"));
            const item = element("button", "inline-link");
            item.type = "button";
            item.textContent = moduleByUri.get(uri)?.file || uri;
            item.addEventListener("click", () => selectModule(uri));
            chain.append(item);
          });
          content.append(chain);
        }
      }

      if (unresolved.length === 0 && moduleCycles.length === 0) {
        content.append(emptyState("No unresolved dependencies or module cycles."));
      }
    }

    function renderDependencies(content, module) {
      const edges = outgoingEdges(module);
      if (edges.length === 0) {
        content.append(emptyState("This module has no require or import dependencies."));
        return;
      }
      const list = element("div", "rows");
      for (const edge of edges) list.append(edgeRow(edge, false));
      content.append(list);
    }

    function renderDependents(content, module) {
      const edges = incomingEdges(module);
      if (edges.length === 0) {
        content.append(emptyState("No resolved module imports this file. It is a project root candidate."));
        return;
      }
      const list = element("div", "rows");
      for (const edge of edges) list.append(edgeRow(edge, true));
      content.append(list);
    }

    function renderExports(content, module) {
      if (module.exports.length === 0) {
        content.append(emptyState("No exported surface was inferred for this module."));
        return;
      }
      const list = element("div", "rows");
      for (const item of module.exports) {
        const row = element("button", "row export-row");
        row.type = "button";
        row.append(label(item.name, "row-title"), label(item.type, "type"));
        const meta = item.documentation ? item.origin + " / " + item.documentation : item.origin;
        row.append(label(meta, "row-meta"));
        row.addEventListener("click", () => openLocation(module.uri, item.range));
        list.append(row);
      }
      content.append(list);
    }

    function renderWhy(content, module) {
      if (module.root) {
        const callout = element("div", "callout");
        callout.append(sectionHeading("Project root candidate"));
        callout.append(label("No resolved module imports this file, so analysis treats it as an entry point for inclusion paths.", "body-copy"));
        content.append(callout);
      }

      if (!module.inclusionPath) {
        if (!module.root) {
          const callout = element("div", "callout warning");
          callout.append(sectionHeading("No root path"));
          callout.append(label(
            module.inCycle
              ? "This module is only reachable inside a dependency cycle with no resolved root importer."
              : "No resolved path from a project root candidate reaches this module.",
            "body-copy",
          ));
          content.append(callout);
        }
        return;
      }

      content.append(sectionHeading("Shortest inclusion path"));
      const timeline = element("ol", "timeline");
      const root = moduleByUri.get(module.inclusionPath.rootUri);
      const rootItem = element("li", "timeline-item root-step");
      const rootButton = element("button", "timeline-link");
      rootButton.type = "button";
      rootButton.textContent = root?.file || module.inclusionPath.rootFile;
      rootButton.addEventListener("click", () => openLocation(module.inclusionPath.rootUri));
      rootItem.append(label("Root", "timeline-kind"), rootButton);
      timeline.append(rootItem);

      for (const step of module.inclusionPath.steps) {
        const item = element("li", "timeline-item");
        const target = element("button", "timeline-link");
        target.type = "button";
        target.textContent = step.targetFile;
        target.addEventListener("click", () => openLocation(step.sourceUri, step.range));
        item.append(
          label(step.kind + " " + JSON.stringify(step.specifier), "timeline-kind"),
          target,
          label("from " + step.sourceFile, "row-meta"),
        );
        timeline.append(item);
      }
      content.append(timeline);
      content.append(label("The path is deterministic and chooses the shortest resolved chain from any root candidate.", "footnote"));
    }

    function edgeRow(edge, reverse) {
      const row = element("button", "row dependency-row " + edge.resolution.status);
      row.type = "button";
      const titleText = reverse
        ? edge.sourceFile
        : (edge.resolution.targetFile || edge.specifier || "Invalid module");
      row.append(label(titleText, "row-title"));
      row.append(badge(edge.kind, "kind"), badge(edge.resolution.status, edge.resolution.status));
      if (edge.inCycle) row.append(badge("cycle", "cycle"));
      const details = reverse
        ? edge.kind + " " + JSON.stringify(edge.specifier) + " at line " + (edge.range.start.line + 1)
        : edge.sourceFile + ":" + (edge.range.start.line + 1) + resolutionDetail(edge);
      row.append(label(details, "row-meta"));
      row.addEventListener("click", () => openLocation(edge.sourceUri, edge.range));
      const sourceButton = element("span", "row-action");
      sourceButton.textContent = reverse ? "Open import" : "Open source";
      row.append(sourceButton);
      return row;
    }

    function resolutionDetail(edge) {
      if (edge.resolution.status === "missing") {
        return edge.resolution.searchRoots.length > 0
          ? " / searched " + edge.resolution.searchRoots.join(", ")
          : " / module not found";
      }
      if (edge.resolution.status === "invalid") {
        return edge.resolution.reason ? " / " + edge.resolution.reason : " / invalid specifier";
      }
      if (edge.resolution.status === "duplicate") {
        return " / " + edge.resolution.candidates.length + " candidates";
      }
      return "";
    }

    function outgoingEdges(module) {
      return graphData.edges.filter((edge) => edge.sourceUri === module.uri);
    }

    function incomingEdges(module) {
      return graphData.edges.filter((edge) => edge.resolution.targetUri === module.uri);
    }

    function selectModule(uri) {
      selectedUri = uri;
      render();
    }

    function openLocation(uri, range) {
      vscode.postMessage({ command: "openLocation", uri, range });
    }

    function metric(value, title, warning = false) {
      const item = element("div", "metric" + (warning ? " warning" : ""));
      item.append(label(String(value), "metric-value"), label(title, "metric-label"));
      return item;
    }

    function sectionHeading(text) {
      return label(text, "section-heading", "h2");
    }

    function badge(text, kind) {
      return label(text, "badge " + kind);
    }

    function emptyState(text) {
      const state = element("div", "empty-state");
      state.append(label(text, "body-copy"));
      return state;
    }

    function label(text, className, tag = "span") {
      const node = element(tag, className);
      node.textContent = text;
      return node;
    }

    function element(tag, className) {
      const node = document.createElement(tag);
      if (className) node.className = className;
      return node;
    }

    function cssEscape(value) {
      return globalThis.CSS?.escape ? CSS.escape(value) : value.replace(/["\\\\]/g, "\\\\$&");
    }

    function debounce(callback, wait) {
      let timer;
      return (...args) => {
        clearTimeout(timer);
        timer = setTimeout(() => callback(...args), wait);
      };
    }

    render();
  `;

  return documentHtml(webview, "Module Graph", body, script);
}

function documentHtml(webview, title, body, script) {
  return webviewDocument(webview, { title, body, script, styles: `
    :root { color-scheme: light dark; }
    * { box-sizing: border-box; }
    html, body { height: 100%; }
    body {
      margin: 0;
      color: var(--vscode-foreground);
      background: var(--vscode-editor-background);
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
      letter-spacing: 0;
      overflow: hidden;
    }
    button, input { font: inherit; letter-spacing: 0; }
    button { color: inherit; }
    .toolbar {
      min-height: 58px;
      padding: 9px 14px;
      border-bottom: 1px solid var(--vscode-panel-border);
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 18px;
      background: var(--vscode-sideBar-background);
    }
    .title-block { min-width: 160px; }
    h1 { margin: 0; font-size: 15px; line-height: 20px; font-weight: 600; }
    .summary, .revision, .inspector-subtitle, .row-meta, .node-meta, .footnote {
      color: var(--vscode-descriptionForeground);
      font-size: 11px;
    }
    .tools { display: flex; align-items: center; justify-content: flex-end; gap: 8px; min-width: 0; }
    .search { display: flex; align-items: center; gap: 6px; color: var(--vscode-descriptionForeground); }
    .search input {
      width: min(260px, 30vw);
      height: 28px;
      padding: 3px 8px;
      border: 1px solid var(--vscode-input-border, transparent);
      border-radius: 3px;
      color: var(--vscode-input-foreground);
      background: var(--vscode-input-background);
      outline: none;
    }
    .search input:focus { border-color: var(--vscode-focusBorder); }
    .segments { height: 28px; display: flex; border: 1px solid var(--vscode-panel-border); border-radius: 4px; overflow: hidden; }
    .segments button, .tab {
      border: 0;
      border-right: 1px solid var(--vscode-panel-border);
      padding: 4px 9px;
      background: transparent;
      cursor: pointer;
    }
    .segments button:last-child { border-right: 0; }
    .segments button:hover, .tab:hover { background: var(--vscode-toolbar-hoverBackground); }
    .segments button.active { background: var(--vscode-button-secondaryBackground); color: var(--vscode-button-secondaryForeground); }
    .icon-button {
      width: 28px;
      height: 28px;
      border: 0;
      border-radius: 3px;
      background: transparent;
      cursor: pointer;
      font-size: 18px;
    }
    .icon-button:hover { background: var(--vscode-toolbar-hoverBackground); }
    .layout { height: calc(100vh - 58px); display: grid; grid-template-columns: minmax(420px, 1fr) minmax(320px, 38%); }
    .graph-band { min-width: 0; display: flex; flex-direction: column; border-right: 1px solid var(--vscode-panel-border); }
    .graph-heading { height: 32px; padding: 7px 13px; display: flex; justify-content: space-between; border-bottom: 1px solid var(--vscode-panel-border); font-size: 11px; font-weight: 600; text-transform: uppercase; }
    .graph-scroll { min-height: 0; flex: 1; overflow: auto; background: var(--vscode-editor-background); }
    .graph { position: relative; min-height: 360px; isolation: isolate; }
    .connections { position: absolute; inset: 0; z-index: 0; pointer-events: none; overflow: visible; }
    .connection { fill: none; stroke: var(--vscode-editorWidget-border); stroke-width: 1.5; opacity: .9; }
    .connection.cycle { stroke: var(--vscode-editorWarning-foreground); stroke-dasharray: 5 4; }
    .connection.missing, .connection.invalid { stroke: var(--vscode-editorError-foreground); stroke-dasharray: 3 4; }
    .connection.duplicate { stroke: var(--vscode-editorWarning-foreground); }
    .nodes { position: relative; z-index: 1; }
    .graph-column { position: absolute; top: 18px; width: 214px; display: flex; flex-direction: column; gap: 12px; }
    .depth-label { height: 18px; color: var(--vscode-descriptionForeground); font-size: 10px; text-transform: uppercase; }
    .module-node {
      width: 214px;
      min-height: 82px;
      padding: 9px 10px;
      border: 1px solid var(--vscode-panel-border);
      border-radius: 5px;
      display: flex;
      flex-direction: column;
      align-items: stretch;
      gap: 6px;
      text-align: left;
      background: var(--vscode-editorWidget-background);
      cursor: pointer;
      overflow: hidden;
    }
    .module-node:hover { border-color: var(--vscode-focusBorder); }
    .module-node.selected { outline: 1px solid var(--vscode-focusBorder); border-color: var(--vscode-focusBorder); }
    .unresolved-node { border-color: var(--vscode-editorError-foreground); border-style: dashed; }
    .node-heading, .inspector-title-row { min-width: 0; display: flex; align-items: center; gap: 7px; }
    .node-file { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }
    .badges { display: flex; flex-wrap: wrap; gap: 4px; }
    .badge {
      width: fit-content;
      max-width: 100%;
      padding: 1px 5px;
      border: 1px solid var(--vscode-panel-border);
      border-radius: 3px;
      color: var(--vscode-descriptionForeground);
      font-size: 10px;
      font-weight: 500;
      white-space: nowrap;
    }
    .badge.kind { color: var(--vscode-symbolIcon-moduleForeground, var(--vscode-foreground)); }
    .badge.root { color: var(--vscode-testing-iconPassed); }
    .badge.cycle, .metric.warning .metric-value { color: var(--vscode-editorWarning-foreground); }
    .badge.unresolved, .badge.missing, .badge.invalid { color: var(--vscode-editorError-foreground); }
    .inspector { min-width: 0; overflow: auto; background: var(--vscode-sideBar-background); }
    .inspector-header { padding: 14px 16px 11px; border-bottom: 1px solid var(--vscode-panel-border); }
    .file-link { min-width: 0; padding: 0; border: 0; background: transparent; font-size: 14px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; cursor: pointer; }
    .file-link:hover, .inline-link:hover, .timeline-link:hover { color: var(--vscode-textLink-activeForeground); text-decoration: underline; }
    .inspector-subtitle { display: block; margin-top: 5px; }
    .tabs { display: flex; overflow-x: auto; border-bottom: 1px solid var(--vscode-panel-border); }
    .tab { min-height: 32px; flex: 0 0 auto; border-right: 0; border-bottom: 2px solid transparent; font-size: 11px; }
    .tab.active { color: var(--vscode-textLink-foreground); border-bottom-color: var(--vscode-focusBorder); }
    .inspector-content { padding: 14px 16px 30px; }
    .metrics { display: grid; grid-template-columns: repeat(4, minmax(64px, 1fr)); border: 1px solid var(--vscode-panel-border); border-radius: 5px; overflow: hidden; }
    .metric { min-width: 0; padding: 9px; border-right: 1px solid var(--vscode-panel-border); }
    .metric:last-child { border-right: 0; }
    .metric-value { display: block; font-size: 16px; font-weight: 600; }
    .metric-label { display: block; margin-top: 2px; color: var(--vscode-descriptionForeground); font-size: 10px; overflow-wrap: anywhere; }
    .section-heading { display: block; margin: 18px 0 8px; font-size: 11px; font-weight: 600; text-transform: uppercase; }
    .rows { display: flex; flex-direction: column; border-top: 1px solid var(--vscode-panel-border); }
    .row {
      position: relative;
      width: 100%;
      min-height: 62px;
      padding: 8px 80px 8px 8px;
      border: 0;
      border-bottom: 1px solid var(--vscode-panel-border);
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto auto;
      align-items: center;
      gap: 5px;
      text-align: left;
      background: transparent;
      cursor: pointer;
    }
    .row:hover { background: var(--vscode-list-hoverBackground); }
    .row-title { min-width: 0; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .row-meta { grid-column: 1 / -1; overflow-wrap: anywhere; }
    .row-action { position: absolute; right: 8px; top: 10px; color: var(--vscode-textLink-foreground); font-size: 10px; }
    .export-row { grid-template-columns: minmax(0, 1fr) minmax(80px, auto); padding-right: 8px; }
    .type { min-width: 0; color: var(--vscode-symbolIcon-typeParameterForeground, var(--vscode-textLink-foreground)); font-family: var(--vscode-editor-font-family); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .cycle-chain { padding: 9px; border-left: 2px solid var(--vscode-editorWarning-foreground); display: flex; flex-wrap: wrap; align-items: center; gap: 6px; background: var(--vscode-editorWidget-background); }
    .inline-link, .timeline-link { padding: 0; border: 0; color: var(--vscode-textLink-foreground); background: transparent; cursor: pointer; text-align: left; }
    .chain-arrow { color: var(--vscode-descriptionForeground); }
    .callout { padding: 10px 12px; border-left: 2px solid var(--vscode-focusBorder); background: var(--vscode-editorWidget-background); }
    .callout.warning { border-left-color: var(--vscode-editorWarning-foreground); }
    .callout .section-heading { margin: 0 0 5px; }
    .body-copy { color: var(--vscode-descriptionForeground); line-height: 1.45; }
    .timeline { margin: 0; padding: 0; list-style: none; }
    .timeline-item { position: relative; margin-left: 7px; padding: 0 0 18px 18px; border-left: 1px solid var(--vscode-panel-border); display: flex; flex-direction: column; gap: 3px; }
    .timeline-item::before { content: ""; position: absolute; left: -4px; top: 3px; width: 7px; height: 7px; border-radius: 50%; background: var(--vscode-focusBorder); }
    .timeline-item:last-child { border-left-color: transparent; }
    .timeline-kind { color: var(--vscode-descriptionForeground); font-size: 10px; }
    .timeline-link { font-weight: 600; }
    .footnote { display: block; margin-top: 4px; line-height: 1.4; }
    .empty-state { min-height: 120px; padding: 28px 16px; display: flex; align-items: center; justify-content: center; text-align: center; }
    .state { height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; }
    .state.error span { max-width: 560px; color: var(--vscode-errorForeground); }
    .state button { padding: 5px 12px; border: 1px solid var(--vscode-button-border, transparent); border-radius: 3px; color: var(--vscode-button-foreground); background: var(--vscode-button-background); cursor: pointer; }
    .spinner { width: 18px; height: 18px; border: 2px solid var(--vscode-panel-border); border-top-color: var(--vscode-focusBorder); border-radius: 50%; animation: spin .8s linear infinite; }
    @keyframes spin { to { transform: rotate(360deg); } }
    @media (max-width: 900px) {
      .toolbar { align-items: flex-start; }
      .tools { flex-wrap: wrap; }
      .search { order: 3; width: 100%; }
      .search input { width: 100%; }
      .layout { grid-template-columns: minmax(300px, 1fr) minmax(280px, 44%); }
      .metrics { grid-template-columns: repeat(2, 1fr); }
      .metric:nth-child(2) { border-right: 0; }
      .metric:nth-child(-n+2) { border-bottom: 1px solid var(--vscode-panel-border); }
    }
    @media (max-width: 650px) {
      body { overflow: auto; }
      .toolbar { position: sticky; top: 0; z-index: 4; flex-direction: column; }
      .tools { width: 100%; justify-content: flex-start; }
      .segments { max-width: 100%; overflow-x: auto; }
      .layout { height: auto; min-height: calc(100vh - 100px); grid-template-columns: 1fr; }
      .graph-band { min-height: 420px; border-right: 0; border-bottom: 1px solid var(--vscode-panel-border); }
      .inspector { min-height: 420px; }
    }
  ` });
}

module.exports = {
  moduleGraphErrorHtml,
  moduleGraphHtml,
  moduleGraphLoadingHtml,
};
