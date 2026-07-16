"use strict";

const path = require("path");
const vscode = require("vscode");
const { COMMANDS } = require("../constants");

let graphPanel;

function registerSignalGraphFeature({ getClient }) {
  const command = vscode.commands.registerCommand(COMMANDS.showSignalGraph, () =>
    showSignalGraph({ getClient }),
  );

  return vscode.Disposable.from(command, {
    dispose() {
      graphPanel?.dispose?.();
      graphPanel = undefined;
    },
  });
}

async function showSignalGraph({ getClient }) {
  const client = getClient();

  if (!client) {
    vscode.window.showWarningMessage("The Tua language server is not running.");
    return;
  }

  if (graphPanel) {
    graphPanel.reveal?.(vscode.ViewColumn.Beside, false);
  } else {
    graphPanel = vscode.window.createWebviewPanel(
      "tuaSignalGraph",
      "Tua Signal Graph",
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      },
    );
    graphPanel.onDidDispose?.(() => {
      graphPanel = undefined;
    });
    graphPanel.webview.onDidReceiveMessage?.((message) =>
      handleGraphMessage(message, { getClient }),
    );
  }

  await renderSignalGraph(graphPanel, client);
}

async function handleGraphMessage(message, { getClient }) {
  if (message?.command === "reload") {
    const client = getClient();

    if (client && graphPanel) {
      await renderSignalGraph(graphPanel, client);
    }
    return;
  }

  if (message?.command !== "openLocation" || typeof message.uri !== "string") {
    return;
  }

  const range = message.range;
  const start = new vscode.Position(
    Number(range?.start?.line) || 0,
    Number(range?.start?.character) || 0,
  );
  const end = new vscode.Position(
    Number(range?.end?.line) || start.line,
    Number(range?.end?.character) || start.character,
  );

  await vscode.commands.executeCommand("vscode.open", vscode.Uri.parse(message.uri), {
    preview: true,
    selection: new vscode.Range(start, end),
  });
}

async function renderSignalGraph(panel, client) {
  panel.webview.html = signalGraphLoadingHtml(panel.webview);

  try {
    const graph = await client.sendRequest("tua/signalGraph", null);
    const viewModel = signalGraphViewModel(graph);

    panel.webview.html = signalGraphHtml(panel.webview, viewModel);
  } catch (error) {
    panel.webview.html = signalGraphErrorHtml(panel.webview, error);
  }
}

function signalGraphViewModel(graph) {
  const events = Array.isArray(graph?.events) ? graph.events : [];

  return {
    events: events
      .filter((event) => typeof event?.name === "string")
      .map((event, index) => ({
        id:
          typeof event.id === "string" && event.id
            ? event.id
            : `${event.name}:${index}`,
        name: event.name,
        bus: typeof event.bus === "string" ? event.bus : "",
        busFile:
          typeof event.busUri === "string" ? displayPath(event.busUri) : "",
        calls: (Array.isArray(event.calls) ? event.calls : [])
          .filter((call) => typeof call?.uri === "string" && call?.range)
          .map((call) => ({
            uri: call.uri,
            file: displayPath(call.uri),
            receiver: typeof call.receiver === "string" ? call.receiver : "",
            method: typeof call.method === "string" ? call.method : "",
            role: knownRole(call.role),
            range: call.range,
            line: (Number(call.range?.start?.line) || 0) + 1,
            payload: Array.isArray(call.payload)
              ? call.payload.map((parameter) => ({
                  name: typeof parameter?.name === "string" ? parameter.name : "",
                  type: typeof parameter?.type === "string" ? parameter.type : "any",
                }))
              : null,
          })),
      })),
  };
}

function displayPath(uriValue) {
  const uri = vscode.Uri.parse(uriValue);

  if (typeof vscode.workspace.asRelativePath === "function") {
    const relative = vscode.workspace.asRelativePath(uri, false);

    if (relative) {
      return relative;
    }
  }

  return uri.fsPath ? path.basename(uri.fsPath) : uriValue;
}

function knownRole(role) {
  return ["declaration", "listener", "emit", "cleanup", "query"].includes(role)
    ? role
    : "query";
}

function signalGraphLoadingHtml(webview) {
  return documentHtml(
    webview,
    "Signal Graph",
    '<main class="state"><div class="spinner" aria-hidden="true"></div><strong>Loading signal graph...</strong></main>',
    "",
  );
}

function signalGraphErrorHtml(webview, error) {
  const message = escapeHtml(error instanceof Error ? error.message : String(error));

  return documentHtml(
    webview,
    "Signal Graph",
    `<main class="state error"><strong>Signal graph unavailable</strong><span>${message}</span><button id="reload" type="button">Reload</button></main>`,
    `document.getElementById("reload").addEventListener("click", () => vscode.postMessage({ command: "reload" }));`,
  );
}

function signalGraphHtml(webview, graph) {
  const graphJson = jsonForHtml(graph);
  const body = `
    <header class="toolbar">
      <div class="title-block">
        <h1>Signal Graph</h1>
        <span id="summary" class="summary"></span>
      </div>
      <div class="tools">
        <label class="search"><span>Filter</span><input id="filter" type="search" placeholder="Events or files" autocomplete="off"></label>
        <div class="segments" role="group" aria-label="Signal roles">
          <button type="button" data-mode="all" class="active">All</button>
          <button type="button" data-mode="definitions">Definitions</button>
          <button type="button" data-mode="emits">Emits</button>
        </div>
        <button id="reload" class="icon-button" type="button" title="Reload graph" aria-label="Reload graph">&#8635;</button>
      </div>
    </header>
    <main class="layout">
      <section class="graph-band" aria-label="Signal event and file graph">
        <div class="column-labels"><span>Events</span><span>Files</span></div>
        <div id="graph" class="graph"></div>
      </section>
      <aside id="inspector" class="inspector" aria-live="polite"></aside>
    </main>`;

  const script = `
    const graphData = ${graphJson};
    const root = document.getElementById("graph");
    const inspector = document.getElementById("inspector");
    const summary = document.getElementById("summary");
    const filterInput = document.getElementById("filter");
    const segmentButtons = Array.from(document.querySelectorAll("[data-mode]"));
    let mode = "all";
    let query = "";
    let selectedEvent = graphData.events[0]?.id || null;
    let selectedFile = null;

    const roleLabels = {
      declaration: "declare",
      listener: "listen",
      emit: "emit",
      cleanup: "cleanup",
      query: "query",
    };

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
    window.addEventListener("resize", debounce(render, 80));

    function visibleRole(role) {
      if (mode === "definitions") {
        return role === "declaration" || role === "listener";
      }
      if (mode === "emits") {
        return role === "emit";
      }
      return true;
    }

    function visibleEvents() {
      return graphData.events
        .map((event) => ({
          ...event,
          calls: event.calls.filter((call) => {
            if (!visibleRole(call.role)) {
              return false;
            }
            if (!query) {
              return true;
            }
            return [event.name, event.bus, event.busFile, call.file, call.receiver, call.method]
              .some((value) => value.toLowerCase().includes(query));
          }),
        }))
        .filter((event) => event.calls.length > 0);
    }

    function render() {
      const events = visibleEvents();
      const files = Array.from(new Set(events.flatMap((event) => event.calls.map((call) => call.file)))).sort();
      const callCount = events.reduce((total, event) => total + event.calls.length, 0);
      summary.textContent = events.length + " events / " + files.length + " files / " + callCount + " calls";
      root.replaceChildren();

      if (events.length === 0) {
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.textContent = graphData.events.length === 0 ? "No SignalBus calls found." : "No matching signal calls.";
        root.appendChild(empty);
        inspector.replaceChildren();
        return;
      }

      if (!events.some((event) => event.id === selectedEvent)) {
        selectedEvent = events[0].id;
        selectedFile = null;
      }
      if (selectedFile && !files.includes(selectedFile)) {
        selectedFile = null;
      }

      const width = Math.max(root.clientWidth || 0, 720);
      const rowHeight = 62;
      const top = 18;
      const nodeWidth = Math.min(260, Math.max(210, Math.floor(width * 0.31)));
      const fileX = width - nodeWidth - 18;
      const height = Math.max(events.length, files.length) * rowHeight + top * 2;
      root.style.height = height + "px";

      const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      svg.setAttribute("viewBox", "0 0 " + width + " " + height);
      svg.setAttribute("width", String(width));
      svg.setAttribute("height", String(height));
      svg.classList.add("edges");
      root.appendChild(svg);

      const eventY = new Map(events.map((event, index) => [event.id, top + index * rowHeight]));
      const fileY = new Map(files.map((file, index) => [file, top + index * rowHeight]));

      for (const event of events) {
        const grouped = new Map();
        for (const call of event.calls) {
          const calls = grouped.get(call.file) || [];
          calls.push(call);
          grouped.set(call.file, calls);
        }
        for (const [file, calls] of grouped) {
          const line = document.createElementNS("http://www.w3.org/2000/svg", "path");
          const y1 = eventY.get(event.id) + 22;
          const y2 = fileY.get(file) + 22;
          const x1 = nodeWidth + 18;
          const x2 = fileX;
          const bend = Math.max(50, (x2 - x1) * 0.45);
          const roles = Array.from(new Set(calls.map((call) => call.role)));
          const role = roles.length === 1 ? roles[0] : "mixed";
          line.setAttribute("d", "M " + x1 + " " + y1 + " C " + (x1 + bend) + " " + y1 + ", " + (x2 - bend) + " " + y2 + ", " + x2 + " " + y2);
          line.setAttribute("class", "edge role-" + role);
          line.setAttribute("stroke-width", String(Math.min(5, 1.5 + calls.length * 0.65)));
          line.addEventListener("click", () => {
            selectedEvent = event.id;
            selectedFile = file;
            render();
          });
          const title = document.createElementNS("http://www.w3.org/2000/svg", "title");
          title.textContent = event.name + " / " + event.bus + " / " + file + " / " + calls.length + " calls";
          line.appendChild(title);
          svg.appendChild(line);
        }
      }

      for (const event of events) {
        const busDetail = [event.bus, event.busFile].filter(Boolean).join(" / ");
        const detail = (busDetail ? busDetail + " / " : "") + event.calls.length + " calls";
        const button = graphNode("event", event.name, detail);
        button.style.left = "18px";
        button.style.top = eventY.get(event.id) + "px";
        button.style.width = nodeWidth + "px";
        button.classList.toggle("selected", selectedEvent === event.id && !selectedFile);
        button.addEventListener("click", () => {
          selectedEvent = event.id;
          selectedFile = null;
          render();
        });
        root.appendChild(button);
      }

      for (const file of files) {
        const count = events.reduce((total, event) => total + event.calls.filter((call) => call.file === file).length, 0);
        const button = graphNode("file", file, count + " calls");
        button.style.left = fileX + "px";
        button.style.top = fileY.get(file) + "px";
        button.style.width = nodeWidth + "px";
        button.classList.toggle("selected", selectedFile === file);
        button.addEventListener("click", () => {
          if (!events.find((event) => event.id === selectedEvent)?.calls.some((call) => call.file === file)) {
            selectedEvent = events.find((event) => event.calls.some((call) => call.file === file))?.id || selectedEvent;
          }
          selectedFile = selectedFile === file ? null : file;
          renderInspector(events);
          for (const candidate of root.querySelectorAll(".node.file")) {
            candidate.classList.toggle("selected", candidate === button && selectedFile === file);
          }
        });
        root.appendChild(button);
      }

      renderInspector(events);
    }

    function graphNode(kind, label, meta) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "node " + kind;
      const name = document.createElement("strong");
      name.textContent = label;
      const detail = document.createElement("span");
      detail.textContent = meta;
      button.append(name, detail);
      return button;
    }

    function renderInspector(events) {
      const event = events.find((candidate) => candidate.id === selectedEvent) || events[0];
      if (!event) {
        inspector.replaceChildren();
        return;
      }
      const calls = selectedFile ? event.calls.filter((call) => call.file === selectedFile) : event.calls;
      inspector.replaceChildren();

      const head = document.createElement("div");
      head.className = "inspector-head";
      const heading = document.createElement("h2");
      heading.textContent = event.name;
      const meta = document.createElement("span");
      meta.textContent = selectedFile || [event.bus, event.busFile].filter(Boolean).join(" / ") || calls.length + " calls";
      head.append(heading, meta);

      const schemaCall = event.calls.find((call) => call.payload !== null);
      if (schemaCall) {
        const schema = document.createElement("code");
        schema.textContent = event.name + "(" + schemaCall.payload.map((parameter) => parameter.name + ": " + parameter.type).join(", ") + ")";
        head.appendChild(schema);
      }
      inspector.appendChild(head);

      const list = document.createElement("div");
      list.className = "call-list";
      for (const call of calls) {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "call-row role-" + call.role;
        const role = document.createElement("span");
        role.className = "role";
        role.textContent = roleLabels[call.role];
        const location = document.createElement("span");
        location.className = "location";
        location.textContent = call.file + ":" + call.line;
        const expression = document.createElement("code");
        expression.textContent = call.receiver + ":" + call.method + '("' + event.name + '")';
        button.append(role, location, expression);
        button.addEventListener("click", () => vscode.postMessage({
          command: "openLocation",
          uri: call.uri,
          range: call.range,
        }));
        list.appendChild(button);
      }
      inspector.appendChild(list);
    }

    function debounce(callback, wait) {
      let timer;
      return () => {
        clearTimeout(timer);
        timer = setTimeout(callback, wait);
      };
    }

    render();`;

  return documentHtml(webview, "Signal Graph", body, script);
}

function documentHtml(webview, title, body, script) {
  const nonce = webviewNonce();

  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}';">
  <title>${escapeHtml(title)}</title>
  <style>
    :root { color-scheme: light dark; font-family: var(--vscode-font-family); font-size: var(--vscode-font-size); }
    * { box-sizing: border-box; }
    body { margin: 0; color: var(--vscode-foreground); background: var(--vscode-editor-background); }
    button, input { font: inherit; color: inherit; letter-spacing: 0; }
    button { cursor: pointer; }
    .toolbar { position: sticky; z-index: 10; top: 0; display: flex; align-items: center; justify-content: space-between; min-height: 58px; padding: 10px 14px; border-bottom: 1px solid var(--vscode-panel-border); background: var(--vscode-editor-background); gap: 16px; }
    .title-block { display: flex; align-items: baseline; min-width: 0; gap: 10px; }
    h1 { margin: 0; font-size: 17px; font-weight: 650; white-space: nowrap; }
    .summary { color: var(--vscode-descriptionForeground); font-size: 12px; white-space: nowrap; }
    .tools { display: flex; align-items: center; justify-content: flex-end; gap: 8px; min-width: 0; }
    .search { display: flex; align-items: center; height: 30px; border: 1px solid var(--vscode-input-border, var(--vscode-panel-border)); background: var(--vscode-input-background); }
    .search span { padding-left: 8px; color: var(--vscode-descriptionForeground); font-size: 11px; text-transform: uppercase; }
    .search input { width: 180px; min-width: 80px; height: 28px; padding: 0 8px; border: 0; outline: 0; background: transparent; }
    .search:focus-within { border-color: var(--vscode-focusBorder); }
    .segments { display: flex; height: 30px; border: 1px solid var(--vscode-panel-border); }
    .segments button { padding: 0 10px; border: 0; border-right: 1px solid var(--vscode-panel-border); background: transparent; }
    .segments button:last-child { border-right: 0; }
    .segments button.active { color: var(--vscode-button-foreground); background: var(--vscode-button-background); }
    .icon-button { display: grid; place-items: center; width: 30px; height: 30px; padding: 0; border: 1px solid var(--vscode-panel-border); background: transparent; font-size: 18px; }
    .segments button:hover:not(.active), .icon-button:hover { background: var(--vscode-toolbar-hoverBackground); }
    .layout { min-width: 0; }
    .graph-band { padding: 12px 14px 18px; border-bottom: 1px solid var(--vscode-panel-border); overflow-x: auto; }
    .column-labels { display: flex; justify-content: space-between; min-width: 720px; padding: 0 18px 8px; color: var(--vscode-descriptionForeground); font-size: 11px; font-weight: 600; text-transform: uppercase; }
    .graph { position: relative; min-width: 720px; border: 1px solid var(--vscode-panel-border); background: var(--vscode-editorWidget-background, var(--vscode-editor-background)); overflow: hidden; }
    .edges { position: absolute; inset: 0; overflow: visible; pointer-events: none; }
    .edge { fill: none; opacity: .58; pointer-events: stroke; cursor: pointer; }
    .edge:hover { opacity: 1; stroke-width: 5; }
    .role-declaration { stroke: var(--vscode-charts-yellow, #c9a227); }
    .role-listener { stroke: var(--vscode-charts-green, #4da36c); }
    .role-emit { stroke: var(--vscode-charts-red, #d35f72); }
    .role-cleanup { stroke: var(--vscode-charts-orange, #c77b30); }
    .role-query { stroke: var(--vscode-charts-blue, #4b91d1); }
    .role-mixed { stroke: var(--vscode-descriptionForeground); }
    .node { position: absolute; z-index: 2; display: flex; align-items: center; justify-content: space-between; height: 44px; min-width: 0; padding: 0 10px; border: 1px solid var(--vscode-panel-border); border-radius: 4px; background: var(--vscode-sideBar-background, var(--vscode-editor-background)); text-align: left; gap: 10px; }
    .node:hover { border-color: var(--vscode-focusBorder); }
    .node.selected { outline: 1px solid var(--vscode-focusBorder); border-color: var(--vscode-focusBorder); }
    .node strong { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
    .node span { flex: 0 0 auto; color: var(--vscode-descriptionForeground); font-size: 11px; }
    .node.event { border-left: 3px solid var(--vscode-charts-purple, #9b70c7); }
    .node.file { border-left: 3px solid var(--vscode-charts-blue, #4b91d1); }
    .inspector { padding: 0 14px 24px; }
    .inspector-head { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 6px 14px; padding: 14px 0 10px; border-bottom: 1px solid var(--vscode-panel-border); }
    .inspector-head h2 { min-width: 0; margin: 0; overflow-wrap: anywhere; font-size: 15px; }
    .inspector-head span { color: var(--vscode-descriptionForeground); font-size: 12px; }
    .inspector-head code { grid-column: 1 / -1; overflow-x: auto; color: var(--vscode-textPreformat-foreground); white-space: nowrap; }
    .call-list { display: grid; }
    .call-row { display: grid; grid-template-columns: 72px minmax(160px, .7fr) minmax(220px, 1fr); align-items: center; min-height: 40px; padding: 6px 8px; border: 0; border-bottom: 1px solid var(--vscode-panel-border); border-left: 3px solid transparent; background: transparent; text-align: left; gap: 12px; }
    .call-row:hover { background: var(--vscode-list-hoverBackground); }
    .call-row .role { font-size: 10px; font-weight: 650; text-transform: uppercase; }
    .call-row .location { overflow: hidden; color: var(--vscode-descriptionForeground); text-overflow: ellipsis; white-space: nowrap; }
    .call-row code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .call-row.role-declaration { border-left-color: var(--vscode-charts-yellow, #c9a227); }
    .call-row.role-listener { border-left-color: var(--vscode-charts-green, #4da36c); }
    .call-row.role-emit { border-left-color: var(--vscode-charts-red, #d35f72); }
    .call-row.role-cleanup { border-left-color: var(--vscode-charts-orange, #c77b30); }
    .call-row.role-query { border-left-color: var(--vscode-charts-blue, #4b91d1); }
    .empty, .state { display: grid; min-height: 240px; place-content: center; justify-items: center; color: var(--vscode-descriptionForeground); gap: 10px; }
    .state.error strong { color: var(--vscode-errorForeground); }
    .state button { min-height: 30px; padding: 0 12px; border: 0; background: var(--vscode-button-background); color: var(--vscode-button-foreground); }
    .spinner { width: 22px; height: 22px; border: 2px solid var(--vscode-panel-border); border-top-color: var(--vscode-progressBar-background); border-radius: 50%; animation: spin .8s linear infinite; }
    @keyframes spin { to { transform: rotate(360deg); } }
    @media (max-width: 760px) {
      .toolbar { align-items: stretch; flex-direction: column; }
      .tools { justify-content: flex-start; flex-wrap: wrap; }
      .search { flex: 1 1 200px; }
      .search input { width: 100%; }
      .call-row { grid-template-columns: 70px minmax(0, 1fr); }
      .call-row code { grid-column: 1 / -1; }
    }
  </style>
</head>
<body>
  ${body}
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    ${script}
  </script>
</body>
</html>`;
}

function jsonForHtml(value) {
  return JSON.stringify(value)
    .replaceAll("&", "\\u0026")
    .replaceAll("<", "\\u003c")
    .replaceAll(">", "\\u003e");
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function webviewNonce() {
  const characters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let nonce = "";

  for (let index = 0; index < 24; index += 1) {
    nonce += characters.charAt(Math.floor(Math.random() * characters.length));
  }

  return nonce;
}

module.exports = {
  registerSignalGraphFeature,
  signalGraphViewModel,
};
