"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");
const { displayUriPath, openLocationMessage } = require("../shared/locations");
const { escapeHtml, jsonForHtml, webviewDocument } = require("../shared/webview");

let graphPanel;
let latestGraphViewModel;

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

  if (message?.command === "createTypedListener") {
    await createTypedListener(message);
    return;
  }

  if (message?.command === "createDeclarationFromEmit") {
    await createDeclarationFromEmit(message);
    return;
  }

  await openLocationMessage(message);
}

async function renderSignalGraph(panel, client) {
  panel.webview.html = signalGraphLoadingHtml(panel.webview);

  try {
    const graph = await client.sendRequest("tua/signalGraph", null);
    const viewModel = signalGraphViewModel(graph);
    latestGraphViewModel = viewModel;

    panel.webview.html = signalGraphHtml(panel.webview, viewModel);
  } catch (error) {
    latestGraphViewModel = undefined;
    panel.webview.html = signalGraphErrorHtml(panel.webview, error);
  }
}

function signalGraphViewModel(graph) {
  const events = Array.isArray(graph?.events) ? graph.events : [];

  return {
    events: events
      .filter((event) => typeof event?.name === "string")
      .map((event, index) => {
        const id =
          typeof event.id === "string" && event.id
            ? event.id
            : `${event.name}:${index}`;
        const busUri = typeof event.busUri === "string" ? event.busUri : "";

        return {
          id,
          name: event.name,
          bus: typeof event.bus === "string" ? event.bus : "",
          busUri,
          busFile: busUri ? displayUriPath(busUri) : "",
          resolvedPayload: normalizePayload(event.resolvedPayload),
          schemaConflict: event.schemaConflict === true,
          calls: (Array.isArray(event.calls) ? event.calls : [])
            .filter((call) => typeof call?.uri === "string" && call?.range)
            .map((call, callIndex) => {
              const role = knownRole(call.role);
              const line = Number(call.range?.start?.line) || 0;
              const character = Number(call.range?.start?.character) || 0;

              return {
                id:
                  typeof call.id === "string" && call.id
                    ? call.id
                    : `${id}:${call.uri}:${role}:${line}:${character}:${callIndex}`,
                uri: call.uri,
                file: displayUriPath(call.uri),
                receiver: typeof call.receiver === "string" ? call.receiver : "",
                method: typeof call.method === "string" ? call.method : "",
                role,
                range: call.range,
                callRange: call.callRange || call.range,
                insertionPosition: normalizePosition(call.insertionPosition, call.range?.start),
                indentation: typeof call.indentation === "string" ? call.indentation : "",
                line: line + 1,
                payload: normalizePayload(call.payload),
                inferredPayload: normalizePayload(call.inferredPayload),
              };
            }),
        };
      }),
  };
}

function normalizePayload(payload) {
  if (!Array.isArray(payload)) {
    return null;
  }

  return payload.map((parameter, index) => ({
    name:
      typeof parameter?.name === "string" && parameter.name
        ? parameter.name
        : `value${index + 1}`,
    type:
      typeof parameter?.type === "string" && parameter.type
        ? parameter.type
        : "any",
  }));
}

function normalizePosition(position, fallback) {
  const candidate = position && typeof position === "object" ? position : fallback;

  return {
    line: Math.max(0, Number(candidate?.line) || 0),
    character: Math.max(0, Number(candidate?.character) || 0),
  };
}

function knownRole(role) {
  return ["declaration", "listener", "emit", "cleanup", "query"].includes(role)
    ? role
    : "query";
}

async function createTypedListener(message) {
  const { event, call } = graphActionTarget(message, (event) =>
    event.calls.find((candidate) => candidate.id === message.callId && candidate.receiver) ||
    event.calls.find((candidate) => candidate.receiver),
  );

  if (!event || !call) {
    vscode.window.showWarningMessage("Signal event is no longer available. Reload the graph.");
    return;
  }

  if (event.schemaConflict || !Array.isArray(event.resolvedPayload)) {
    vscode.window.showWarningMessage(
      "Cannot create a typed listener until this event has one resolved payload schema.",
    );
    return;
  }

  const text = typedListenerText(event, call);
  await insertSignalText(call, text);
}

async function createDeclarationFromEmit(message) {
  const { event, call } = graphActionTarget(message, (event) =>
    event.calls.find(
      (candidate) =>
        candidate.id === message.callId &&
        candidate.role === "emit" &&
        Array.isArray(candidate.inferredPayload),
    ) ||
    event.calls.find(
      (candidate) => candidate.role === "emit" && Array.isArray(candidate.inferredPayload),
    ),
  );

  if (!event || !call) {
    vscode.window.showWarningMessage("Signal emit is no longer available. Reload the graph.");
    return;
  }

  if (event.calls.some((candidate) => candidate.role === "declaration")) {
    vscode.window.showWarningMessage("This signal event already has a declaration.");
    return;
  }

  const text = declarationText(event, call);
  await insertSignalText(call, text);
}

function graphActionTarget(message, pickCall) {
  const event = latestGraphViewModel?.events.find((candidate) => candidate.id === message?.eventId);
  const call = event ? pickCall(event) : undefined;

  return { event, call };
}

async function insertSignalText(call, text) {
  if (!call.uri || !call.receiver || !call.insertionPosition) {
    vscode.window.showWarningMessage("Signal graph insertion target is incomplete.");
    return;
  }

  const uri = vscode.Uri.parse(call.uri);
  const position = new vscode.Position(
    call.insertionPosition.line,
    call.insertionPosition.character,
  );
  const edit = new vscode.WorkspaceEdit();

  edit.insert(uri, position, text);

  if (!(await vscode.workspace.applyEdit(edit))) {
    vscode.window.showWarningMessage("Could not apply the SignalBus edit.");
    return;
  }

  await vscode.commands.executeCommand("vscode.open", uri, {
    selection: new vscode.Range(position, position),
  });
}

function typedListenerText(event, call) {
  const indent = call.indentation || "";
  const parameters = event.resolvedPayload.map(formatTypedParameter).join(", ");

  return `${indent}${call.receiver}:on("${escapeLuaString(event.name)}", function(${parameters})\n${indent}end)\n`;
}

function declarationText(event, call) {
  const indent = call.indentation || "";
  const payload = Array.isArray(call.inferredPayload) ? call.inferredPayload : [];
  const parameters = payload.map(formatTypedParameter).join(", ");

  return `${indent}${call.receiver}:declare("${escapeLuaString(event.name)}", function(${parameters}) end)\n`;
}

function formatTypedParameter(parameter, index) {
  return `${luaIdentifier(parameter.name, index)}: ${typeLabel(parameter.type)}`;
}

function luaIdentifier(value, index) {
  const candidate = typeof value === "string" ? value.trim() : "";

  if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(candidate)) {
    return candidate;
  }

  return `value${index + 1}`;
}

function typeLabel(value) {
  const candidate = typeof value === "string" ? value.replace(/\s+/g, " ").trim() : "";

  return candidate || "any";
}

function escapeLuaString(value) {
  return String(value)
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\n/g, "\\n")
    .replace(/\r/g, "\\r");
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
        <label class="search"><span>Event</span><input id="event-filter" type="search" placeholder="player.hit" autocomplete="off"></label>
        <label class="select-filter"><span>Bus</span><select id="bus-filter"></select></label>
        <label class="select-filter"><span>File</span><select id="file-filter"></select></label>
        <div class="segments" role="group" aria-label="Signal roles">
          <button type="button" data-mode="all" class="active">All</button>
          <button type="button" data-mode="listeners">Listeners</button>
          <button type="button" data-mode="emitters">Emitters</button>
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
    const eventFilterInput = document.getElementById("event-filter");
    const busFilterSelect = document.getElementById("bus-filter");
    const fileFilterSelect = document.getElementById("file-filter");
    const segmentButtons = Array.from(document.querySelectorAll("[data-mode]"));
    let mode = "all";
    let eventQuery = "";
    let busFilter = "";
    let fileFilter = "";
    let selectedEvent = graphData.events[0]?.id || null;
    let selectedFile = null;

    const roleLabels = {
      declaration: "declare",
      listener: "listen",
      emit: "emit",
      cleanup: "cleanup",
      query: "query",
    };

    populateSelect(busFilterSelect, Array.from(new Set(graphData.events.map((event) => event.bus).filter(Boolean))).sort(), "All buses");
    populateSelect(fileFilterSelect, Array.from(new Set(graphData.events.flatMap((event) => event.calls.map((call) => call.file)))).sort(), "All files");

    document.getElementById("reload").addEventListener("click", () => {
      vscode.postMessage({ command: "reload" });
    });
    eventFilterInput.addEventListener("input", () => {
      eventQuery = eventFilterInput.value.trim().toLowerCase();
      selectedFile = null;
      render();
    });
    busFilterSelect.addEventListener("change", () => {
      busFilter = busFilterSelect.value;
      selectedFile = null;
      render();
    });
    fileFilterSelect.addEventListener("change", () => {
      fileFilter = fileFilterSelect.value;
      selectedFile = fileFilter || null;
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

    function populateSelect(select, values, allLabel) {
      const all = document.createElement("option");
      all.value = "";
      all.textContent = allLabel;
      select.appendChild(all);

      for (const value of values) {
        const option = document.createElement("option");
        option.value = value;
        option.textContent = value;
        select.appendChild(option);
      }
    }

    function visibleRole(role) {
      if (mode === "listeners") {
        return role === "declaration" || role === "listener";
      }
      if (mode === "emitters") {
        return role === "emit";
      }
      return true;
    }

    function visibleEvents() {
      return graphData.events
        .filter((event) => !eventQuery || event.name.toLowerCase().includes(eventQuery))
        .filter((event) => !busFilter || event.bus === busFilter)
        .map((event) => ({
          ...event,
          calls: event.calls.filter((call) => {
            if (!visibleRole(call.role)) {
              return false;
            }
            if (fileFilter && call.file !== fileFilter) {
              return false;
            }
            return true;
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
        const detailPrefix = event.schemaConflict ? "schema conflict / " : "";
        const detail = detailPrefix + (busDetail ? busDetail + " / " : "") + event.calls.length + " calls";
        const button = graphNode("event", event.name, detail);
        button.style.left = "18px";
        button.style.top = eventY.get(event.id) + "px";
        button.style.width = nodeWidth + "px";
        button.classList.toggle("conflict", event.schemaConflict);
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
      const listenerTarget = calls.find((call) => call.receiver) || event.calls.find((call) => call.receiver);
      const declarationTarget =
        calls.find((call) => call.role === "emit" && call.inferredPayload !== null) ||
        event.calls.find((call) => call.role === "emit" && call.inferredPayload !== null);
      inspector.replaceChildren();

      const head = document.createElement("div");
      head.className = "inspector-head";
      const heading = document.createElement("h2");
      heading.textContent = event.name;
      const meta = document.createElement("span");
      meta.textContent = selectedFile || [event.bus, event.busFile].filter(Boolean).join(" / ") || calls.length + " calls";
      head.append(heading, meta);

      if (event.resolvedPayload !== null && !event.schemaConflict) {
        const schema = document.createElement("code");
        schema.textContent = event.name + "(" + payloadSignature(event.resolvedPayload) + ")";
        head.appendChild(schema);
      }
      inspector.appendChild(head);

      const actions = document.createElement("div");
      actions.className = "inspector-actions";
      actions.append(
        actionButton("Create typed listener", !event.schemaConflict && event.resolvedPayload !== null && listenerTarget, () => {
          vscode.postMessage({
            command: "createTypedListener",
            eventId: event.id,
            callId: listenerTarget?.id,
          });
        }),
        actionButton("Create declaration from emit", !hasDeclaration(event) && declarationTarget, () => {
          vscode.postMessage({
            command: "createDeclarationFromEmit",
            eventId: event.id,
            callId: declarationTarget?.id,
          });
        }),
      );
      inspector.appendChild(actions);

      if (event.schemaConflict) {
        const conflict = document.createElement("div");
        conflict.className = "conflict-panel";
        const title = document.createElement("strong");
        title.textContent = "Conflicting payload schemas";
        const details = document.createElement("div");
        details.className = "conflict-list";

        for (const call of event.calls.filter((call) => call.payload !== null)) {
          const row = document.createElement("button");
          row.type = "button";
          row.className = "conflict-row role-" + call.role;
          const where = document.createElement("span");
          where.textContent = roleLabels[call.role] + " / " + call.file + ":" + call.line;
          const signature = document.createElement("code");
          signature.textContent = event.name + "(" + payloadSignature(call.payload) + ")";
          row.append(where, signature);
          row.addEventListener("click", () => openCall(call));
          details.appendChild(row);
        }

        conflict.append(title, details);
        inspector.appendChild(conflict);
      }

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
        const payload = call.payload || call.inferredPayload;
        const signature = document.createElement("span");
        signature.className = "payload";
        signature.textContent = payload ? payloadSignature(payload) : "";
        button.append(role, location, expression, signature);
        button.addEventListener("click", () => openCall(call));
        list.appendChild(button);
      }
      inspector.appendChild(list);
    }

    function actionButton(label, enabled, onClick) {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = label;
      button.disabled = !enabled;
      if (enabled) {
        button.addEventListener("click", onClick);
      }
      return button;
    }

    function hasDeclaration(event) {
      return event.calls.some((call) => call.role === "declaration");
    }

    function payloadSignature(payload) {
      return payload.map((parameter) => parameter.name + ": " + parameter.type).join(", ");
    }

    function openCall(call) {
      vscode.postMessage({
        command: "openLocation",
        uri: call.uri,
        range: call.range,
      });
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
  return webviewDocument(webview, { title, body, script, styles: `
    :root { color-scheme: light dark; font-family: var(--vscode-font-family); font-size: var(--vscode-font-size); }
    * { box-sizing: border-box; }
    body { margin: 0; color: var(--vscode-foreground); background: var(--vscode-editor-background); }
    button, input, select { font: inherit; color: inherit; letter-spacing: 0; }
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
    .select-filter { display: flex; align-items: center; height: 30px; border: 1px solid var(--vscode-input-border, var(--vscode-panel-border)); background: var(--vscode-input-background); }
    .select-filter span { padding-left: 8px; color: var(--vscode-descriptionForeground); font-size: 11px; text-transform: uppercase; }
    .select-filter select { max-width: 180px; min-width: 96px; height: 28px; padding: 0 7px; border: 0; outline: 0; background: transparent; }
    .select-filter:focus-within { border-color: var(--vscode-focusBorder); }
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
    .node.conflict { border-left-color: var(--vscode-editorWarning-foreground, var(--vscode-charts-orange, #c77b30)); }
    .inspector { padding: 0 14px 24px; }
    .inspector-head { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 6px 14px; padding: 14px 0 10px; border-bottom: 1px solid var(--vscode-panel-border); }
    .inspector-head h2 { min-width: 0; margin: 0; overflow-wrap: anywhere; font-size: 15px; }
    .inspector-head span { color: var(--vscode-descriptionForeground); font-size: 12px; }
    .inspector-head code { grid-column: 1 / -1; overflow-x: auto; color: var(--vscode-textPreformat-foreground); white-space: nowrap; }
    .inspector-actions { display: flex; flex-wrap: wrap; gap: 8px; padding: 12px 0; border-bottom: 1px solid var(--vscode-panel-border); }
    .inspector-actions button { min-height: 30px; padding: 0 10px; border: 1px solid var(--vscode-panel-border); background: var(--vscode-button-secondaryBackground, transparent); color: var(--vscode-button-secondaryForeground, var(--vscode-foreground)); }
    .inspector-actions button:hover:not(:disabled) { background: var(--vscode-toolbar-hoverBackground); }
    .inspector-actions button:disabled { cursor: default; opacity: .48; }
    .conflict-panel { display: grid; gap: 8px; padding: 12px 0; border-bottom: 1px solid var(--vscode-panel-border); }
    .conflict-panel strong { color: var(--vscode-editorWarning-foreground, var(--vscode-warningForeground)); font-size: 12px; }
    .conflict-list { display: grid; gap: 6px; }
    .conflict-row { display: grid; grid-template-columns: minmax(160px, .55fr) minmax(220px, 1fr); align-items: center; min-height: 34px; padding: 6px 8px; border: 1px solid var(--vscode-panel-border); border-left: 3px solid transparent; background: transparent; text-align: left; gap: 10px; }
    .conflict-row:hover { background: var(--vscode-list-hoverBackground); }
    .conflict-row span { overflow: hidden; color: var(--vscode-descriptionForeground); text-overflow: ellipsis; white-space: nowrap; }
    .conflict-row code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .conflict-row.role-declaration { border-left-color: var(--vscode-charts-yellow, #c9a227); }
    .conflict-row.role-listener { border-left-color: var(--vscode-charts-green, #4da36c); }
    .call-list { display: grid; }
    .call-row { display: grid; grid-template-columns: 72px minmax(150px, .65fr) minmax(190px, 1fr) minmax(160px, .8fr); align-items: center; min-height: 40px; padding: 6px 8px; border: 0; border-bottom: 1px solid var(--vscode-panel-border); border-left: 3px solid transparent; background: transparent; text-align: left; gap: 12px; }
    .call-row:hover { background: var(--vscode-list-hoverBackground); }
    .call-row .role { font-size: 10px; font-weight: 650; text-transform: uppercase; }
    .call-row .location { overflow: hidden; color: var(--vscode-descriptionForeground); text-overflow: ellipsis; white-space: nowrap; }
    .call-row code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .call-row .payload { overflow: hidden; color: var(--vscode-descriptionForeground); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
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
      .select-filter { flex: 1 1 150px; }
      .select-filter select { width: 100%; max-width: none; }
      .call-row { grid-template-columns: 70px minmax(0, 1fr); }
      .call-row code, .call-row .payload { grid-column: 1 / -1; }
      .conflict-row { grid-template-columns: minmax(0, 1fr); }
    }
  ` });
}

module.exports = {
  registerSignalGraphFeature,
  signalGraphViewModel,
};
