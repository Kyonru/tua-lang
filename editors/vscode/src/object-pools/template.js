"use strict";

const { escapeHtml, jsonForHtml, webviewDocument } = require("../shared/webview");

function objectPoolExplorerLoadingHtml(webview) {
  return documentHtml(
    webview,
    "Object Pool Explorer",
    '<main class="state"><div class="spinner" aria-hidden="true"></div><strong>Loading object-pool index...</strong></main>',
    "",
  );
}

function objectPoolExplorerErrorHtml(webview, error) {
  const message = escapeHtml(error instanceof Error ? error.message : String(error));

  return documentHtml(
    webview,
    "Object Pool Explorer",
    `<main class="state error"><strong>Object Pool Explorer unavailable</strong><span>${message}</span><button id="reload" type="button">Reload</button></main>`,
    'document.getElementById("reload").addEventListener("click", () => vscode.postMessage({ command: "reload" }));',
  );
}

function objectPoolExplorerHtml(webview, viewModel) {
  const data = jsonForHtml(viewModel);
  const body = `
    <header class="toolbar">
      <div class="title-block">
        <h1>Object Pool Explorer</h1>
        <span id="summary" class="muted"></span>
      </div>
      <div class="tools">
        <label class="search"><span>Filter</span><input id="filter" type="search" placeholder="Pools, items, operations" autocomplete="off"></label>
        <button id="reload" class="command-button" type="button" title="Reload object-pool index">Reload</button>
      </div>
    </header>
    <nav class="segments" aria-label="Object pool explorer mode">
      <button type="button" data-mode="pools">Pools <span id="pool-count"></span></button>
      <button type="button" data-mode="operations">Lifecycle <span id="operation-count"></span></button>
      <button type="button" data-mode="insights">Insights <span id="insight-count"></span></button>
    </nav>
    <main class="layout">
      <aside class="catalog" aria-label="Object pool items">
        <div id="list" class="list"></div>
      </aside>
      <section id="detail" class="detail" aria-live="polite"></section>
    </main>`;
  const script = `
    const model = ${data};
    const listRoot = document.getElementById("list");
    const detailRoot = document.getElementById("detail");
    const filterInput = document.getElementById("filter");
    const modeButtons = Array.from(document.querySelectorAll("[data-mode]"));
    const collections = {
      pools: model.pools,
      operations: model.operations,
      insights: model.insights,
    };
    let mode = initialMode();
    let filter = "";
    let selectedId = initialSelection(mode);

    document.getElementById("pool-count").textContent = model.pools.length;
    document.getElementById("operation-count").textContent = model.operations.length;
    document.getElementById("insight-count").textContent = model.insights.length;
    document.getElementById("summary").textContent =
      "analysis r" + model.analysisRevision + " · " + model.pools.length + " pools · " + model.operations.length + " operations";
    document.getElementById("reload").addEventListener("click", () => {
      vscode.postMessage({ command: "reload" });
    });
    filterInput.addEventListener("input", () => {
      filter = filterInput.value.trim().toLowerCase();
      render();
    });
    for (const button of modeButtons) {
      button.addEventListener("click", () => {
        mode = button.dataset.mode;
        selectedId = initialSelection(mode);
        render();
      });
    }

    function initialMode() {
      if (model.insights.length) return "insights";
      if (model.pools.length) return "pools";
      return "operations";
    }

    function initialSelection(targetMode) {
      const items = collections[targetMode] || [];
      return items.find((item) => item.uri === model.focusUri)?.id || items[0]?.id || null;
    }

    function visibleItems() {
      const items = collections[mode] || [];
      if (!filter) return items;
      return items.filter((item) => searchableText(item).includes(filter));
    }

    function searchableText(item) {
      return JSON.stringify([
        item.name,
        item.itemName,
        item.itemType,
        item.poolName,
        item.kind,
        item.title,
        item.message,
        item.file,
      ]).toLowerCase();
    }

    function render() {
      for (const button of modeButtons) {
        button.classList.toggle("active", button.dataset.mode === mode);
      }

      const items = visibleItems();
      if (!items.some((item) => item.id === selectedId)) {
        selectedId = items[0]?.id || null;
      }
      renderList(items);
      renderDetail(items.find((item) => item.id === selectedId));
    }

    function renderList(items) {
      listRoot.replaceChildren();
      if (!items.length) {
        listRoot.append(emptyState(filter ? "No matching object-pool items" : "No indexed object pools"));
        return;
      }

      for (const item of items) {
        const button = element("button", "list-row");
        button.type = "button";
        button.classList.toggle("selected", item.id === selectedId);
        button.append(label(itemTitle(item), "row-title"));
        button.append(label(itemMeta(item), "row-meta"));
        const badgeText = itemBadge(item);
        if (badgeText) button.append(label(badgeText, "count-badge"));
        button.addEventListener("click", () => {
          selectedId = item.id;
          render();
        });
        listRoot.append(button);
      }
    }

    function renderDetail(item) {
      detailRoot.replaceChildren();
      if (!item) {
        detailRoot.append(emptyState("Select an object-pool item"));
        return;
      }

      const header = element("header", "detail-header");
      const heading = element("div", "detail-heading");
      heading.append(label(itemTitle(item), "detail-title", "h2"));
      heading.append(label(item.file + ":" + item.line, "muted"));
      header.append(heading);
      header.append(locationButton("Open", item.uri, item.nameRange || item.itemRange || item.range));
      detailRoot.append(header);

      if (mode === "pools") renderPool(item);
      if (mode === "operations") renderOperation(item);
      if (mode === "insights") renderInsight(item);
    }

    function renderPool(pool) {
      const body = element("div", "detail-body");
      const operations = pool.operationIds.map(operationById).filter(Boolean);
      const insights = pool.insightIds.map(insightById).filter(Boolean);

      body.append(metricStrip([
        [pool.itemType, "Item type"],
        [operations.length, "Operations"],
        [insights.length, "Insights"],
      ]));
      body.append(sectionHeading("Lifecycle"));
      body.append(operationRows(operations));
      if (insights.length) {
        body.append(sectionHeading("Conservative warnings"));
        body.append(insightRows(insights));
      }
      detailRoot.append(body);
    }

    function renderOperation(operation) {
      const body = element("div", "detail-body");
      body.append(metricStrip([
        [operationLabel(operation.kind), "Operation"],
        [operation.poolName, "Pool"],
        [operation.itemName || "none", "Item"],
      ]));
      const relatedPool = poolById(operation.poolId);
      if (relatedPool) {
        body.append(sectionHeading("Pool"));
        body.append(linkRows([{
          title: relatedPool.name,
          meta: relatedPool.itemType + " · " + relatedPool.file + ":" + relatedPool.line,
          uri: relatedPool.uri,
          range: relatedPool.nameRange || relatedPool.range,
        }]));
      }
      const relatedInsight = model.insights.find((insight) => insight.acquireOperationId === operation.id);
      if (relatedInsight) {
        body.append(sectionHeading("Insight"));
        body.append(insightRows([relatedInsight]));
      }
      detailRoot.append(body);
    }

    function renderInsight(insight) {
      const body = element("div", "detail-body");
      const callout = element("div", "warning-callout");
      callout.append(label(insight.kind, "warning-label"));
      callout.append(label(insight.message, "body-copy"));
      body.append(callout);

      const operation = operationById(insight.acquireOperationId);
      const pool = poolById(insight.poolId);
      const rows = [];
      if (operation) {
        rows.push({
          title: operationLabel(operation.kind) + (operation.itemName ? " " + operation.itemName : ""),
          meta: operation.file + ":" + operation.line,
          uri: operation.uri,
          range: operation.itemRange || operation.range,
        });
      }
      if (pool) {
        rows.push({
          title: pool.name,
          meta: pool.itemType + " · " + pool.file + ":" + pool.line,
          uri: pool.uri,
          range: pool.nameRange || pool.range,
        });
      }
      if (rows.length) {
        body.append(sectionHeading("Related locations"));
        body.append(linkRows(rows));
      }
      detailRoot.append(body);
    }

    function operationRows(operations) {
      if (!operations.length) return note("No visible lifecycle calls for this pool.");
      return linkRows(operations.map((operation) => ({
        title: operationLabel(operation.kind) + (operation.itemName ? " " + operation.itemName : ""),
        meta: operation.file + ":" + operation.line,
        badge: operation.kind,
        uri: operation.uri,
        range: operation.itemRange || operation.range,
      })));
    }

    function insightRows(insights) {
      if (!insights.length) return note("No conservative lifecycle warnings.");
      return linkRows(insights.map((insight) => ({
        title: insight.title,
        meta: insight.message,
        badge: insight.kind,
        uri: insight.uri,
        range: insight.range,
        className: "warning-row",
      })));
    }

    function linkRows(rows) {
      const root = element("div", "rows");
      for (const entry of rows) {
        const row = element("button", "data-row " + (entry.className || ""));
        row.type = "button";
        row.append(label(entry.title, "data-name"));
        if (entry.badge) row.append(label(entry.badge, "kind-badge " + entry.badge));
        row.append(label(entry.meta, "row-meta"));
        row.addEventListener("click", () => openLocation(entry.uri, entry.range));
        root.append(row);
      }
      return root;
    }

    function poolById(id) {
      return model.pools.find((pool) => pool.id === id);
    }

    function operationById(id) {
      return model.operations.find((operation) => operation.id === id);
    }

    function insightById(id) {
      return model.insights.find((insight) => insight.id === id);
    }

    function operationLabel(kind) {
      return ({ acquire: "acquire", release: "release", clear: "clear", prewarm: "prewarm" })[kind] || "operation";
    }

    function itemTitle(item) {
      if (mode === "pools") return item.name;
      if (mode === "operations") return operationLabel(item.kind) + " " + item.poolName;
      return item.title;
    }

    function itemMeta(item) {
      if (mode === "pools") return item.itemType + " · " + item.file + ":" + item.line;
      if (mode === "operations") return (item.itemName || "no item") + " · " + item.file + ":" + item.line;
      return item.file + ":" + item.line;
    }

    function itemBadge(item) {
      if (mode === "pools") return String(item.insightIds.length || item.operationIds.length);
      if (mode === "operations") return item.kind;
      return item.kind;
    }

    function metricStrip(metrics) {
      const strip = element("div", "metrics");
      for (const [value, name] of metrics) {
        const metric = element("div", "metric");
        metric.append(label(String(value), "metric-value"));
        metric.append(label(name, "metric-name"));
        strip.append(metric);
      }
      return strip;
    }

    function sectionHeading(text) {
      return label(text, "section-heading", "h3");
    }

    function note(text) {
      return label(text, "body-copy note", "p");
    }

    function locationButton(text, uri, range) {
      const button = element("button", "command-button");
      button.type = "button";
      button.textContent = text;
      button.addEventListener("click", () => openLocation(uri, range));
      return button;
    }

    function openLocation(uri, range) {
      vscode.postMessage({ command: "openLocation", uri, range });
    }

    function emptyState(text) {
      const state = element("div", "empty-state");
      state.append(label(text, "muted"));
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

    render();
  `;

  return documentHtml(webview, "Object Pool Explorer", body, script);
}

function documentHtml(webview, title, body, script) {
  return webviewDocument(webview, { title, body, script, styles: `
    :root { color-scheme: light dark; }
    * { box-sizing: border-box; }
    html, body { height: 100%; }
    body { margin: 0; color: var(--vscode-foreground); background: var(--vscode-editor-background); font-family: var(--vscode-font-family); font-size: var(--vscode-font-size); letter-spacing: 0; overflow: hidden; }
    button, input { color: inherit; font: inherit; letter-spacing: 0; }
    .toolbar { min-height: 58px; padding: 9px 14px; border-bottom: 1px solid var(--vscode-panel-border); display: flex; align-items: center; justify-content: space-between; gap: 16px; background: var(--vscode-sideBar-background); }
    .title-block { min-width: 190px; }
    h1 { margin: 0; font-size: 15px; line-height: 20px; font-weight: 600; }
    .muted, .row-meta, .metric-name { color: var(--vscode-descriptionForeground); font-size: 11px; }
    .tools { min-width: 0; display: flex; align-items: center; gap: 8px; }
    .search { display: flex; align-items: center; gap: 7px; color: var(--vscode-descriptionForeground); }
    .search input { width: min(300px, 38vw); height: 28px; padding: 3px 8px; border: 1px solid var(--vscode-input-border, transparent); border-radius: 3px; outline: none; color: var(--vscode-input-foreground); background: var(--vscode-input-background); }
    .search input:focus { border-color: var(--vscode-focusBorder); }
    .command-button { min-height: 28px; padding: 4px 10px; border: 1px solid var(--vscode-button-border, transparent); border-radius: 3px; color: var(--vscode-button-secondaryForeground); background: var(--vscode-button-secondaryBackground); cursor: pointer; white-space: nowrap; }
    .command-button:hover { background: var(--vscode-button-secondaryHoverBackground); }
    .segments { min-height: 34px; padding: 0 14px; border-bottom: 1px solid var(--vscode-panel-border); display: flex; align-items: stretch; gap: 18px; background: var(--vscode-editor-background); overflow-x: auto; }
    .segments button { padding: 0 1px; border: 0; border-bottom: 2px solid transparent; background: transparent; cursor: pointer; white-space: nowrap; }
    .segments button:hover { color: var(--vscode-textLink-activeForeground); }
    .segments button.active { color: var(--vscode-textLink-foreground); border-bottom-color: var(--vscode-focusBorder); }
    .segments span { margin-left: 4px; color: var(--vscode-descriptionForeground); font-size: 10px; }
    .layout { height: calc(100vh - 92px); display: grid; grid-template-columns: minmax(240px, 32%) minmax(380px, 1fr); }
    .catalog { min-width: 0; border-right: 1px solid var(--vscode-panel-border); overflow: auto; background: var(--vscode-sideBar-background); }
    .list { padding: 5px 0 24px; }
    .list-row { position: relative; width: 100%; min-height: 54px; padding: 8px 66px 8px 14px; border: 0; display: flex; flex-direction: column; align-items: flex-start; gap: 4px; text-align: left; background: transparent; cursor: pointer; }
    .list-row:hover { background: var(--vscode-list-hoverBackground); }
    .list-row.selected { color: var(--vscode-list-activeSelectionForeground); background: var(--vscode-list-activeSelectionBackground); }
    .row-title { width: 100%; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .row-meta { width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .list-row.selected .row-meta { color: inherit; opacity: .78; }
    .count-badge { position: absolute; right: 14px; top: 18px; max-width: 48px; padding: 1px 6px; border: 1px solid currentColor; border-radius: 8px; text-align: center; font-size: 10px; opacity: .75; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .detail { min-width: 0; overflow: auto; }
    .detail-header { min-height: 70px; padding: 13px 18px; border-bottom: 1px solid var(--vscode-panel-border); display: flex; align-items: center; justify-content: space-between; gap: 16px; background: var(--vscode-editorWidget-background); }
    .detail-heading { min-width: 0; display: flex; flex-direction: column; gap: 4px; }
    .detail-title { margin: 0; font-size: 16px; font-weight: 600; overflow-wrap: anywhere; }
    .detail-body { padding: 16px 18px 36px; }
    .metrics { display: grid; grid-template-columns: repeat(3, minmax(90px, 1fr)); border-top: 1px solid var(--vscode-panel-border); border-bottom: 1px solid var(--vscode-panel-border); }
    .metric { min-width: 0; padding: 9px 10px; border-right: 1px solid var(--vscode-panel-border); }
    .metric:last-child { border-right: 0; }
    .metric-value { display: block; font-size: 16px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .metric-name { display: block; margin-top: 2px; }
    .section-heading { margin: 20px 0 8px; font-size: 11px; font-weight: 600; text-transform: uppercase; }
    .rows { border-top: 1px solid var(--vscode-panel-border); }
    .data-row { width: 100%; min-height: 48px; padding: 7px 8px; border: 0; border-bottom: 1px solid var(--vscode-panel-border); display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 5px 12px; text-align: left; background: transparent; cursor: pointer; }
    .data-row:hover { background: var(--vscode-list-hoverBackground); }
    .data-name { min-width: 0; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .data-row .row-meta { grid-column: 1 / -1; }
    .kind-badge { padding: 1px 6px; border: 1px solid currentColor; border-radius: 3px; font-size: 10px; white-space: nowrap; }
    .kind-badge.acquire { color: var(--vscode-symbolIcon-methodForeground, var(--vscode-textLink-foreground)); }
    .kind-badge.release { color: var(--vscode-testing-iconPassed, #3fb950); }
    .kind-badge.clear, .kind-badge.prewarm { color: var(--vscode-descriptionForeground); }
    .kind-badge.unreleasedAcquire { color: var(--vscode-editorWarning-foreground); }
    .body-copy { margin: 0; color: var(--vscode-descriptionForeground); line-height: 1.5; }
    .note { padding: 8px 0; }
    .warning-callout { padding: 12px 14px; border-left: 2px solid var(--vscode-editorWarning-foreground); background: var(--vscode-editorWidget-background); }
    .warning-label { display: block; margin-bottom: 5px; color: var(--vscode-editorWarning-foreground); font-size: 11px; font-weight: 600; text-transform: uppercase; }
    .warning-row { border-left: 2px solid var(--vscode-editorWarning-foreground); }
    .empty-state { min-height: 180px; padding: 30px 18px; display: flex; align-items: center; justify-content: center; text-align: center; }
    .state { height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; }
    .state.error span { max-width: 560px; color: var(--vscode-errorForeground); }
    .spinner { width: 18px; height: 18px; border: 2px solid var(--vscode-panel-border); border-top-color: var(--vscode-focusBorder); border-radius: 50%; animation: spin .8s linear infinite; }
    @keyframes spin { to { transform: rotate(360deg); } }
    @media (max-width: 720px) {
      body { overflow: auto; }
      .toolbar { align-items: flex-start; flex-direction: column; }
      .tools { width: 100%; }
      .search { min-width: 0; flex: 1; }
      .search input { width: 100%; }
      .layout { height: auto; min-height: calc(100vh - 126px); grid-template-columns: 1fr; }
      .catalog { max-height: 40vh; border-right: 0; border-bottom: 1px solid var(--vscode-panel-border); }
      .detail { min-height: 55vh; }
    }
  ` });
}

module.exports = {
  objectPoolExplorerErrorHtml,
  objectPoolExplorerHtml,
  objectPoolExplorerLoadingHtml,
};
