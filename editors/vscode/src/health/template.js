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
  const diagnosis = healthDiagnosis(viewModel);
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
      <section class="diagnosis" aria-label="Current snapshot interpretation">
        <h2>${escapeHtml(diagnosis.title)}</h2>
        <p>${escapeHtml(diagnosis.detail)}</p>
        <dl class="facts">
          ${fact("Queued now", viewModel.queue.total)}
          ${fact("Warm files", `${viewModel.warm.warmed}/${viewModel.warm.discovered}`)}
          ${fact("Request p95", latencyLabel(viewModel.latency.p95Ms, viewModel.latency.recentCount))}
          ${fact("Stale results since start", viewModel.stale.total)}
        </dl>
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
          ${kv("p50", latencyLabel(viewModel.latency.p50Ms, viewModel.latency.recentCount))}
          ${kv("p95", latencyLabel(viewModel.latency.p95Ms, viewModel.latency.recentCount))}
          ${kv("p99", latencyLabel(viewModel.latency.p99Ms, viewModel.latency.recentCount))}
          ${kv("Max", latencyLabel(viewModel.latency.maxMs, viewModel.latency.recentCount))}
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

function fact(label, value) {
  return `<div><dt>${escapeHtml(label)}</dt><dd>${escapeHtml(value)}</dd></div>`;
}

function healthDiagnosis(viewModel) {
  if (viewModel.warm.failed > 0) {
    return { title: `${viewModel.warm.failed} file analyses failed`, detail: "The workspace snapshot is incomplete. Check Tua language-server output for the affected files." };
  }
  if (viewModel.queue.total > 0) {
    return { title: `${viewModel.queue.total} requests queued`, detail: "This is one server snapshot. Reload to see whether the queue drains after editing stops." };
  }
  if (viewModel.latency.recentCount === 0) {
    return { title: "No request timings captured yet", detail: "Use completion or hover in a Tua file, then reload this view. Queue and stale-result counts are shown separately below." };
  }
  const method = [...viewModel.latency.methods]
    .filter((row) => row.count > 0)
    .sort((left, right) => right.p95Ms - left.p95Ms)[0];
  return method
    ? { title: `Highest sampled p95: ${method.method}`, detail: `${method.p95Ms} ms across ${method.count} sampled requests. These are server timings, not visible editor delay.` }
    : { title: "No requests queued at this snapshot", detail: "Recent request timing is available below. Server timings do not include VS Code menu paint." };
}

function kv(label, value) {
  return `<div class="kv"><span>${escapeHtml(label)}</span><strong>${escapeHtml(value)}</strong></div>`;
}

function rateLabel({ hits, misses, hitRate }) {
  return hits + misses > 0 ? `${hitRate}%` : "No samples";
}

function latencyLabel(milliseconds, count) {
  return count > 0 ? `${milliseconds}ms` : "No samples";
}

function snapshotLabel(viewModel) {
  if (viewModel.warm.snapshotReadyRevision !== null) {
    return `ready r${viewModel.warm.snapshotReadyRevision}`;
  }
  if (viewModel.warm.snapshotFailedRevision !== null) {
    return `failed r${viewModel.warm.snapshotFailedRevision}`;
  }

  return viewModel.warm.enabled ? "pending" : "not enabled";
}

function cacheTable(rows) {
  return table(
    ["Cache", "Hits", "Misses", "Hit rate"],
    rows.map((row) => [
      row.label,
      row.hits,
      row.misses,
      rateLabel(row),
    ]),
  );
}

function latencyTable(rows) {
  return table(
    ["Method", "Samples", "p95", "Max"],
    [...rows].sort((left, right) => right.p95Ms - left.p95Ms).map((row) => [
      row.method,
      row.count,
      latencyLabel(row.p95Ms, row.count),
      latencyLabel(row.maxMs, row.count),
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
      .diagnosis {
        padding: 2px 0 16px;
        margin-bottom: 16px;
        border-bottom: 1px solid var(--vscode-panel-border);
      }
      .diagnosis h2 { font-size: 16px; margin-bottom: 4px; }
      .diagnosis p { margin: 0 0 14px; color: var(--vscode-descriptionForeground); max-width: 70ch; }
      .facts { display: flex; flex-wrap: wrap; gap: 12px 28px; margin: 0; }
      .facts > div { min-width: 120px; }
      .facts dt { color: var(--vscode-descriptionForeground); font-size: 12px; }
      .facts dd { margin: 2px 0 0; font-variant-numeric: tabular-nums; font-weight: 600; }
      article {
        border: 1px solid var(--vscode-panel-border);
        border-radius: 6px;
        background: var(--vscode-editorWidget-background);
      }
      .kv span {
        color: var(--vscode-descriptionForeground);
        font-size: 12px;
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
      button:focus-visible { outline: 2px solid var(--vscode-focusBorder); outline-offset: 2px; }
    `,
  });
}

module.exports = {
  serverHealthErrorHtml,
  serverHealthHtml,
  serverHealthLoadingHtml,
};
