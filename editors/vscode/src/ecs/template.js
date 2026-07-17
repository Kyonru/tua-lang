"use strict";

const { escapeHtml, jsonForHtml, webviewDocument } = require("../shared/webview");

function ecsExplorerLoadingHtml(webview) {
  return documentHtml(
    webview,
    "ECS Explorer",
    '<main class="state"><div class="spinner" aria-hidden="true"></div><strong>Loading ECS index...</strong></main>',
    "",
  );
}

function ecsExplorerErrorHtml(webview, error) {
  const message = escapeHtml(error instanceof Error ? error.message : String(error));

  return documentHtml(
    webview,
    "ECS Explorer",
    `<main class="state error"><strong>ECS Explorer unavailable</strong><span>${message}</span><button id="reload" type="button">Reload</button></main>`,
    'document.getElementById("reload").addEventListener("click", () => vscode.postMessage({ command: "reload" }));',
  );
}

function ecsExplorerHtml(webview, viewModel) {
  const data = jsonForHtml(viewModel);
  const body = `
    <header class="toolbar">
      <div class="title-block">
        <h1>ECS Explorer</h1>
        <span id="summary" class="muted"></span>
      </div>
      <div class="tools">
        <label class="search"><span>Filter</span><input id="filter" type="search" placeholder="Components, systems, queries" autocomplete="off"></label>
        <button id="reload" class="command-button" type="button" title="Reload ECS index">Reload</button>
      </div>
    </header>
    <nav class="segments" aria-label="ECS explorer mode">
      <button type="button" data-mode="components">Components <span id="component-count"></span></button>
      <button type="button" data-mode="systems">Systems <span id="system-count"></span></button>
      <button type="button" data-mode="queries">Queries <span id="query-count"></span></button>
      <button type="button" data-mode="insights">Insights <span id="insight-count"></span></button>
    </nav>
    <main class="layout">
      <aside class="catalog" aria-label="ECS items">
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
      components: model.components,
      systems: model.systems,
      queries: model.queries,
      insights: model.insights,
    };
    let mode = initialMode();
    let filter = "";
    let selectedId = initialSelection(mode);

    document.getElementById("component-count").textContent = model.components.length;
    document.getElementById("system-count").textContent = model.systems.length;
    document.getElementById("query-count").textContent = model.queries.length;
    document.getElementById("insight-count").textContent = model.insights.length;
    document.getElementById("summary").textContent =
      "analysis r" + model.analysisRevision + " · " + model.components.length + " components · " + model.systems.length + " systems";
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
      if (model.components.length) return "components";
      if (model.systems.length) return "systems";
      if (model.queries.length) return "queries";
      return "insights";
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
        item.localName,
        item.runtimeName,
        item.title,
        item.message,
        item.file,
        ...(item.fields || []).flatMap((field) => [field.name, field.type]),
        ...(item.components || []).flatMap((component) => [component.reference, component.access]),
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
        listRoot.append(emptyState(filter ? "No matching ECS items" : "No indexed ECS items"));
        return;
      }

      for (const item of items) {
        const button = element("button", "list-row");
        button.type = "button";
        button.classList.toggle("selected", item.id === selectedId);
        button.append(label(itemTitle(item), "row-title"));
        button.append(label(itemMeta(item), "row-meta"));
        if (mode === "systems" || mode === "queries") {
          button.append(label(String(item.components.length), "count-badge"));
        }
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
        detailRoot.append(emptyState("Select an ECS item to inspect it"));
        return;
      }

      const header = element("header", "detail-header");
      const heading = element("div", "detail-heading");
      heading.append(label(itemTitle(item), "detail-title", "h2"));
      heading.append(label(item.file + ":" + item.line, "muted"));
      header.append(heading);
      header.append(locationButton("Open definition", item.uri, item.nameRange || item.range));
      detailRoot.append(header);

      if (mode === "components") renderComponent(item);
      if (mode === "systems") renderSystem(item);
      if (mode === "queries") renderQuery(item);
      if (mode === "insights") renderInsight(item);
    }

    function renderComponent(component) {
      const body = element("div", "detail-body");
      body.append(metricStrip([
        [component.fields.length, "Fields"],
        [systemsFor(component.id).length, "Systems"],
        [queriesFor(component.id).length, "Queries"],
      ]));

      body.append(sectionHeading("Schema"));
      if (!component.fields.length) {
        body.append(note("No statically indexed schema fields."));
      } else {
        const fields = element("div", "rows");
        for (const field of component.fields) {
          const row = element("button", "data-row");
          row.type = "button";
          row.append(label(field.name, "data-name"));
          row.append(label(field.type, "type-label"));
          row.addEventListener("click", () => openLocation(component.uri, field.range));
          fields.append(row);
        }
        body.append(fields);
      }

      body.append(sectionHeading("System access"));
      const systems = systemsFor(component.id);
      if (!systems.length) {
        body.append(note("No system currently queries this component."));
      } else {
        body.append(relationshipRows(systems.map(({ item, use }) => ({
          title: item.name,
          meta: item.file + ":" + use.line,
          access: use.access,
          uri: item.uri,
          range: use.range,
        }))));
      }

      body.append(sectionHeading("Relevant queries"));
      const queries = queriesFor(component.id);
      if (!queries.length) {
        body.append(note("No standalone or system query currently references this component."));
      } else {
        body.append(relationshipRows(queries.map(({ item, use }) => ({
          title: item.name,
          meta: queryKind(item) + " · " + item.file + ":" + use.line,
          access: use.access,
          uri: item.uri,
          range: use.range,
        }))));
      }
      detailRoot.append(body);
    }

    function renderSystem(system) {
      const body = element("div", "detail-body");
      body.append(sectionHeading("Component access"));
      body.append(componentRows(system.components));

      const insights = model.insights.filter((insight) => insight.subjectId === system.id);
      if (insights.length) {
        body.append(sectionHeading("Performance insights"));
        body.append(insightRows(insights));
      }
      detailRoot.append(body);
    }

    function renderQuery(query) {
      const body = element("div", "detail-body");
      body.append(metricStrip([
        [queryKind(query), "Kind"],
        [query.components.length, "Components"],
      ]));
      body.append(sectionHeading("Matched components"));
      body.append(componentRows(query.components));

      const insights = model.insights.filter((insight) => insight.subjectId === query.id);
      if (insights.length) {
        body.append(sectionHeading("Performance insights"));
        body.append(insightRows(insights));
      }
      detailRoot.append(body);
    }

    function renderInsight(insight) {
      const body = element("div", "detail-body");
      const callout = element("div", "insight-callout");
      callout.append(label("Insight, not a diagnostic", "insight-label"));
      callout.append(label(insight.message, "body-copy"));
      body.append(callout);

      const subject = [...model.systems, ...model.queries].find((item) => item.id === insight.subjectId);
      if (subject) {
        body.append(sectionHeading("Related ECS item"));
        body.append(relationshipRows([{
          title: subject.name,
          meta: subject.file + ":" + subject.line,
          access: "",
          uri: subject.uri,
          range: subject.nameRange || subject.range,
        }]));
      }
      detailRoot.append(body);
    }

    function componentRows(uses) {
      if (!uses.length) return note("No statically indexed component list.");
      const rows = element("div", "rows");
      for (const use of uses) {
        const component = model.components.find((candidate) => candidate.id === use.componentId);
        const row = element("button", "data-row");
        row.type = "button";
        row.append(label(component?.runtimeName || use.reference, "data-name"));
        row.append(accessBadge(use.access));
        row.append(label(component ? component.file : "unresolved static reference", "row-meta"));
        row.addEventListener("click", () => openLocation(use.uri, use.range));
        rows.append(row);
      }
      return rows;
    }

    function relationshipRows(relations) {
      const rows = element("div", "rows");
      for (const relation of relations) {
        const row = element("button", "data-row");
        row.type = "button";
        row.append(label(relation.title, "data-name"));
        if (relation.access) row.append(accessBadge(relation.access));
        row.append(label(relation.meta, "row-meta"));
        row.addEventListener("click", () => openLocation(relation.uri, relation.range));
        rows.append(row);
      }
      return rows;
    }

    function insightRows(insights) {
      const rows = element("div", "rows");
      for (const insight of insights) {
        const row = element("button", "data-row insight-row");
        row.type = "button";
        row.append(label(insight.title, "data-name"));
        row.append(label(insight.message, "row-meta"));
        row.addEventListener("click", () => openLocation(insight.uri, insight.range));
        rows.append(row);
      }
      return rows;
    }

    function systemsFor(componentId) {
      return model.systems.flatMap((item) => item.components
        .filter((use) => use.componentId === componentId)
        .map((use) => ({ item, use })));
    }

    function queriesFor(componentId) {
      return model.queries.flatMap((item) => item.components
        .filter((use) => use.componentId === componentId)
        .map((use) => ({ item, use })));
    }

    function accessBadge(access) {
      const badge = label(accessLabel(access), "access " + access);
      badge.title = access === "unknown" ? "A method call may mutate this component" : "Component access: " + accessLabel(access);
      return badge;
    }

    function accessLabel(access) {
      return ({ filter: "filter", read: "read", write: "write", readWrite: "read + write", unknown: "unknown mutation" })[access] || "unknown";
    }

    function queryKind(query) {
      return query.kind === "system" ? "system query" : "standalone query";
    }

    function itemTitle(item) {
      if (mode === "components") return item.runtimeName;
      if (mode === "insights") return item.title;
      return item.name;
    }

    function itemMeta(item) {
      if (mode === "components") return item.localName + " · " + item.file + ":" + item.line;
      if (mode === "queries") return queryKind(item) + " · " + item.file + ":" + item.line;
      if (mode === "insights") return item.file + ":" + item.line;
      return item.file + ":" + item.line;
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

  return documentHtml(webview, "ECS Explorer", body, script);
}

function documentHtml(webview, title, body, script) {
  return webviewDocument(webview, { title, body, script, styles: `
    :root { color-scheme: light dark; }
    * { box-sizing: border-box; }
    html, body { height: 100%; }
    body { margin: 0; color: var(--vscode-foreground); background: var(--vscode-editor-background); font-family: var(--vscode-font-family); font-size: var(--vscode-font-size); letter-spacing: 0; overflow: hidden; }
    button, input { color: inherit; font: inherit; letter-spacing: 0; }
    .toolbar { min-height: 58px; padding: 9px 14px; border-bottom: 1px solid var(--vscode-panel-border); display: flex; align-items: center; justify-content: space-between; gap: 16px; background: var(--vscode-sideBar-background); }
    .title-block { min-width: 180px; }
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
    .layout { height: calc(100vh - 92px); display: grid; grid-template-columns: minmax(230px, 31%) minmax(380px, 1fr); }
    .catalog { min-width: 0; border-right: 1px solid var(--vscode-panel-border); overflow: auto; background: var(--vscode-sideBar-background); }
    .list { padding: 5px 0 24px; }
    .list-row { position: relative; width: 100%; min-height: 54px; padding: 8px 42px 8px 14px; border: 0; display: flex; flex-direction: column; align-items: flex-start; gap: 4px; text-align: left; background: transparent; cursor: pointer; }
    .list-row:hover { background: var(--vscode-list-hoverBackground); }
    .list-row.selected { color: var(--vscode-list-activeSelectionForeground); background: var(--vscode-list-activeSelectionBackground); }
    .row-title { width: 100%; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .row-meta { width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .list-row.selected .row-meta { color: inherit; opacity: .78; }
    .count-badge { position: absolute; right: 14px; top: 18px; min-width: 20px; padding: 1px 5px; border: 1px solid currentColor; border-radius: 8px; text-align: center; font-size: 10px; opacity: .75; }
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
    .type-label { color: var(--vscode-symbolIcon-typeParameterForeground, var(--vscode-textLink-foreground)); font-family: var(--vscode-editor-font-family); }
    .access { padding: 1px 6px; border: 1px solid currentColor; border-radius: 3px; font-size: 10px; white-space: nowrap; }
    .access.filter { color: var(--vscode-descriptionForeground); }
    .access.read { color: var(--vscode-testing-iconPassed, #3fb950); }
    .access.write { color: var(--vscode-symbolIcon-variableForeground, #75beff); }
    .access.readWrite { color: var(--vscode-symbolIcon-eventForeground, #dcdcaa); }
    .access.unknown { color: var(--vscode-editorWarning-foreground); }
    .body-copy { margin: 0; color: var(--vscode-descriptionForeground); line-height: 1.5; }
    .note { padding: 8px 0; }
    .insight-callout { padding: 12px 14px; border-left: 2px solid var(--vscode-symbolIcon-eventForeground, var(--vscode-focusBorder)); background: var(--vscode-editorWidget-background); }
    .insight-label { display: block; margin-bottom: 5px; color: var(--vscode-symbolIcon-eventForeground, var(--vscode-textLink-foreground)); font-size: 11px; font-weight: 600; text-transform: uppercase; }
    .insight-row { border-left: 2px solid var(--vscode-symbolIcon-eventForeground, var(--vscode-focusBorder)); }
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
  ecsExplorerErrorHtml,
  ecsExplorerHtml,
  ecsExplorerLoadingHtml,
};
