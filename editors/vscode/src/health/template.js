"use strict";

const { escapeHtml, jsonForHtml, webviewDocument } = require("../shared/webview");

function serverHealthLoadingHtml(webview) {
  return documentHtml(
    webview,
    "Server Health",
    '<main class="state"><div class="spinner" aria-hidden="true"></div><strong>Loading server health...</strong></main>',
    "",
  );
}

function serverHealthErrorHtml(webview, error) {
  const message = escapeHtml(error instanceof Error ? error.message : String(error));

  return documentHtml(
    webview,
    "Server Health",
    `<main class="state error"><strong>Server health unavailable</strong><span>${message}</span><button id="reload" type="button">Reload</button></main>`,
    'document.getElementById("reload").addEventListener("click", () => vscode.postMessage({ command: "reload" }));',
  );
}

function serverHealthHtml(webview, viewModel) {
  const modelJson = jsonForHtml(viewModel);
  const body = `
    <header class="toolbar">
      <div class="title-block">
        <h1>Language Server Health</h1>
        <span>Revision ${escapeHtml(viewModel.revision)} · ${escapeHtml(viewModel.refreshedAt)}</span>
      </div>
      <div class="tools">
        <button id="copy-json" type="button">Copy JSON</button>
        <button id="reload" type="button">Reload</button>
      </div>
    </header>
    <main>
      <section class="metrics" aria-label="Server health summary">
        ${metric("Revision", viewModel.revision)}
        ${metric("Warm coverage", `${viewModel.warm.coveragePercent}%`)}
        ${metric("Queue depth", viewModel.queue.total)}
        ${metric("Cache hit rate", `${viewModel.cache.hitRate}%`)}
        ${metric("Stale results", viewModel.stale.total)}
        ${metric("Recent p95", `${viewModel.latency.p95Ms}ms`)}
      </section>
      <section class="grid">
        <article>
          <h2>Workspace Warm</h2>
          ${kv("Enabled", viewModel.warm.enabled ? "yes" : "no")}
          ${kv("Files", `${viewModel.warm.warmed}/${viewModel.warm.discovered}`)}
          ${kv("Queued", `${viewModel.warm.queued} (${viewModel.warm.queuedHigh} high / ${viewModel.warm.queuedLow} low)`)}
          ${kv("Failed", viewModel.warm.failed)}
          ${kv("Snapshot", snapshotLabel(viewModel))}
        </article>
        <article>
          <h2>Queues</h2>
          ${kv("Coordinator", viewModel.queue.coordinator)}
          ${kv("In flight", viewModel.queue.inFlight)}
          ${kv("Warm high", viewModel.queue.workspaceWarmHigh)}
          ${kv("Warm low", viewModel.queue.workspaceWarmLow)}
          ${kv("Open docs", viewModel.documents.open)}
        </article>
        <article>
          <h2>Stale Results</h2>
          ${kv("Requests", viewModel.stale.requests)}
          ${kv("Workspace warm", viewModel.stale.workspaceWarm)}
          ${kv("Recent changes", viewModel.documents.recentChanges)}
        </article>
        <article>
          <h2>Latency</h2>
          ${kv("Samples", viewModel.latency.recentCount)}
          ${kv("p50", `${viewModel.latency.p50Ms}ms`)}
          ${kv("p95", `${viewModel.latency.p95Ms}ms`)}
          ${kv("p99", `${viewModel.latency.p99Ms}ms`)}
          ${kv("Max", `${viewModel.latency.maxMs}ms`)}
        </article>
      </section>
      <section class="tables">
        <article>
          <h2>Cache</h2>
          ${cacheTable(viewModel.cache.rows)}
        </article>
        <article>
          <h2>Latency By Method</h2>
          ${latencyTable(viewModel.latency.methods)}
        </article>
      </section>
    </main>`;

  const script = `
    const healthData = ${modelJson};
    document.getElementById("reload").addEventListener("click", () => {
      vscode.postMessage({ command: "reload" });
    });
    document.getElementById("copy-json").addEventListener("click", () => {
      vscode.postMessage({ command: "copyJson" });
    });
    window.__tuaServerHealth = healthData;
  `;

  return documentHtml(webview, "Server Health", body, script);
}

function metric(label, value) {
  return `<div class="metric"><span>${escapeHtml(label)}</span><strong>${escapeHtml(value)}</strong></div>`;
}

function kv(label, value) {
  return `<div class="kv"><span>${escapeHtml(label)}</span><strong>${escapeHtml(value)}</strong></div>`;
}

function snapshotLabel(viewModel) {
  if (viewModel.warm.snapshotReadyRevision !== null) {
    return `ready r${viewModel.warm.snapshotReadyRevision}`;
  }
  if (viewModel.warm.snapshotFailedRevision !== null) {
    return `failed r${viewModel.warm.snapshotFailedRevision}`;
  }

  return "pending";
}

function cacheTable(rows) {
  return table(
    ["Cache", "Hits", "Misses", "Hit rate"],
    rows.map((row) => [
      row.label,
      row.hits,
      row.misses,
      `${row.hitRate}%`,
    ]),
  );
}

function latencyTable(rows) {
  return table(
    ["Method", "Samples", "p95", "Max"],
    rows.map((row) => [
      row.method,
      row.count,
      `${row.p95Ms}ms`,
      `${row.maxMs}ms`,
    ]),
  );
}

function table(headers, rows) {
  if (rows.length === 0) {
    return '<div class="empty">No samples</div>';
  }

  const head = headers.map((header) => `<th>${escapeHtml(header)}</th>`).join("");
  const body = rows
    .map((row) => `<tr>${row.map((cell) => `<td>${escapeHtml(cell)}</td>`).join("")}</tr>`)
    .join("");

  return `<table><thead><tr>${head}</tr></thead><tbody>${body}</tbody></table>`;
}

function documentHtml(webview, title, body, script) {
  return webviewDocument(webview, {
    title,
    body,
    script,
    styles: `
      :root {
        color-scheme: light dark;
      }
      body {
        margin: 0;
        color: var(--vscode-foreground);
        background: var(--vscode-editor-background);
        font-family: var(--vscode-font-family);
      }
      .toolbar {
        position: sticky;
        top: 0;
        z-index: 2;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
        padding: 14px 18px;
        border-bottom: 1px solid var(--vscode-panel-border);
        background: var(--vscode-editor-background);
      }
      h1, h2 {
        margin: 0;
        font-weight: 650;
      }
      h1 {
        font-size: 18px;
      }
      h2 {
        margin-bottom: 10px;
        font-size: 13px;
      }
      .title-block {
        display: grid;
        gap: 3px;
      }
      .title-block span {
        color: var(--vscode-descriptionForeground);
        font-size: 12px;
      }
      .tools {
        display: flex;
        gap: 8px;
      }
      button {
        border: 1px solid var(--vscode-button-border, transparent);
        border-radius: 4px;
        padding: 5px 10px;
        color: var(--vscode-button-foreground);
        background: var(--vscode-button-background);
        cursor: pointer;
      }
      button:hover {
        background: var(--vscode-button-hoverBackground);
      }
      main {
        padding: 18px;
      }
      .metrics {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
        gap: 10px;
        margin-bottom: 14px;
      }
      .metric, article {
        border: 1px solid var(--vscode-panel-border);
        border-radius: 6px;
        background: var(--vscode-editorWidget-background);
      }
      .metric {
        display: grid;
        gap: 4px;
        padding: 10px;
      }
      .metric span, .kv span {
        color: var(--vscode-descriptionForeground);
        font-size: 12px;
      }
      .metric strong {
        font-size: 18px;
      }
      .grid {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
        gap: 10px;
        margin-bottom: 14px;
      }
      article {
        padding: 12px;
      }
      .kv {
        display: flex;
        justify-content: space-between;
        gap: 12px;
        padding: 5px 0;
        border-top: 1px solid color-mix(in srgb, var(--vscode-panel-border) 55%, transparent);
      }
      .kv:first-of-type {
        border-top: 0;
      }
      .tables {
        display: grid;
        grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
        gap: 10px;
      }
      table {
        width: 100%;
        border-collapse: collapse;
        table-layout: fixed;
      }
      th, td {
        padding: 6px 7px;
        border-top: 1px solid var(--vscode-panel-border);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        text-align: right;
      }
      th:first-child, td:first-child {
        text-align: left;
      }
      th {
        color: var(--vscode-descriptionForeground);
        font-weight: 500;
      }
      .state {
        min-height: 100vh;
        display: grid;
        place-items: center;
        gap: 10px;
        align-content: center;
        color: var(--vscode-descriptionForeground);
      }
      .state.error {
        color: var(--vscode-errorForeground);
      }
      .spinner {
        width: 18px;
        height: 18px;
        border: 2px solid var(--vscode-panel-border);
        border-top-color: var(--vscode-progressBar-background);
        border-radius: 50%;
        animation: spin 0.9s linear infinite;
      }
      .empty {
        color: var(--vscode-descriptionForeground);
        padding: 8px 0;
      }
      @keyframes spin {
        to { transform: rotate(360deg); }
      }
      @media (max-width: 720px) {
        .toolbar, .tools {
          align-items: stretch;
          flex-direction: column;
        }
        .tables {
          grid-template-columns: 1fr;
        }
      }
    `,
  });
}

module.exports = {
  serverHealthErrorHtml,
  serverHealthHtml,
  serverHealthLoadingHtml,
};
