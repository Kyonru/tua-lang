"use strict";

const { escapeHtml, jsonForHtml, webviewDocument } = require("../shared/webview");

const GRAPH_LAYOUT = {
  columnWidth: 320,
  nodeHeight: 68,
  nodeWidth: 158,
  paddingX: 88,
  paddingY: 64,
  routeGap: 28,
  rowHeight: 148,
};

function statePastelHue(index) {
  return (210 + index * 137.508) % 360;
}

function connectionRoutes(edges, depth) {
  const routes = edges.map((edge) => {
    const sourceDepth = depth.get(edge.source) || 0;
    const targetDepth = depth.get(edge.target) || 0;
    let kind = "forward";

    if (edge.source === edge.target) {
      kind = "loop";
    } else if (sourceDepth === targetDepth) {
      kind = "vertical";
    } else if (targetDepth < sourceDepth) {
      kind = "backward";
    } else if (targetDepth - sourceDepth > 1) {
      kind = "longForward";
    }

    return {
      kind,
      lane: 0,
      loopIndex: 0,
      offset: 0,
      sourceDepth,
      targetDepth,
    };
  });
  const pairGroups = new Map();
  const loopCounts = new Map();

  for (const [index, edge] of edges.entries()) {
    if (edge.source === edge.target) {
      const loopIndex = loopCounts.get(edge.source) || 0;
      routes[index].loopIndex = loopIndex;
      loopCounts.set(edge.source, loopIndex + 1);
      continue;
    }

    const pair = [edge.source, edge.target].sort();
    const key = pair[0] + "\u0000" + pair[1];
    const group = pairGroups.get(key) || [];
    group.push(index);
    pairGroups.set(key, group);
  }

  for (const group of pairGroups.values()) {
    group.sort((leftIndex, rightIndex) => {
      const left = edges[leftIndex];
      const right = edges[rightIndex];
      const leftDirection = left.source + "\u0000" + left.target;
      const rightDirection = right.source + "\u0000" + right.target;
      return leftDirection.localeCompare(rightDirection) ||
        left.transition.event.localeCompare(right.transition.event) ||
        leftIndex - rightIndex;
    });

    for (const [laneIndex, edgeIndex] of group.entries()) {
      routes[edgeIndex].offset = (laneIndex - (group.length - 1) / 2) * 36;
    }
  }

  for (const kind of ["longForward", "backward"]) {
    const indices = routes
      .map((route, index) => ({ index, route }))
      .filter((entry) => entry.route.kind === kind)
      .map((entry) => entry.index)
      .sort((left, right) => {
        const leftEdge = edges[left];
        const rightEdge = edges[right];
        return leftEdge.source.localeCompare(rightEdge.source) ||
          leftEdge.target.localeCompare(rightEdge.target) ||
          leftEdge.transition.event.localeCompare(rightEdge.transition.event);
      });
    indices.forEach((edgeIndex, lane) => {
      routes[edgeIndex].lane = lane;
    });
  }

  const verticalLanes = new Map();
  for (const [index, route] of routes.entries()) {
    if (route.kind !== "vertical") continue;
    const lane = verticalLanes.get(route.sourceDepth) || 0;
    route.lane = Math.floor(lane / 2);
    route.side = lane % 2 === 0 ? "right" : "left";
    verticalLanes.set(route.sourceDepth, lane + 1);
  }

  return routes;
}

function stateLayout(machine, edges, config) {
  const stateByName = new Map(machine.states.map((state) => [state.name, state]));
  const colorIndexByName = new Map(
    machine.states.map((state, index) => [state.name, index]),
  );
  const names = machine.states.map((state) => state.name);
  const hasWildcard = edges.some((edge) => edge.source === "*");
  if (hasWildcard) names.push("*");

  const depth = new Map([[machine.initial, 0]]);
  const queue = [machine.initial];
  while (queue.length > 0) {
    const source = queue.shift();
    const nextDepth = (depth.get(source) || 0) + 1;
    for (const edge of edges.filter((candidate) => candidate.source === source)) {
      if (!depth.has(edge.target)) {
        depth.set(edge.target, nextDepth);
        queue.push(edge.target);
      }
    }
  }

  const fallbackDepth = Math.max(0, ...depth.values()) + 1;
  for (const name of names) {
    if (name !== "*" && !depth.has(name)) depth.set(name, fallbackDepth);
  }
  if (hasWildcard) {
    for (const name of names) {
      if (name !== "*") depth.set(name, (depth.get(name) || 0) + 1);
    }
    depth.set("*", 0);
  }

  const columns = new Map();
  for (const name of names) {
    const columnDepth = depth.get(name) || 0;
    const column = columns.get(columnDepth) || [];
    column.push(name);
    columns.set(columnDepth, column);
  }

  const rowOrder = new Map();
  for (const [columnDepth, column] of [...columns.entries()].sort((a, b) => a[0] - b[0])) {
    column.sort((left, right) => {
      if (left === "*") return -1;
      if (right === "*") return 1;
      if (left === machine.initial) return -1;
      if (right === machine.initial) return 1;

      const barycenter = (name) => {
        const incoming = edges
          .filter(
            (edge) =>
              edge.target === name &&
              (depth.get(edge.source) || 0) < columnDepth &&
              rowOrder.has(edge.source),
          )
          .map((edge) => rowOrder.get(edge.source));
        return incoming.length > 0
          ? incoming.reduce((total, value) => total + value, 0) / incoming.length
          : Number.POSITIVE_INFINITY;
      };
      return barycenter(left) - barycenter(right) || left.localeCompare(right);
    });
    column.forEach((name, row) => rowOrder.set(name, row));
  }

  const routes = connectionRoutes(edges, depth);
  const topLaneCount = routes.filter((route) => route.kind === "longForward").length;
  const bottomLaneCount = routes.filter((route) => route.kind === "backward").length;
  const topReserve = topLaneCount > 0 ? 24 + topLaneCount * config.routeGap : 0;
  const bottomReserve =
    bottomLaneCount > 0 ? 24 + bottomLaneCount * config.routeGap : 0;
  const maxRows = Math.max(1, ...Array.from(columns.values(), (column) => column.length));
  const maxDepth = Math.max(0, ...columns.keys());
  const stateBandHeight = Math.max(
    300,
    (maxRows - 1) * config.rowHeight + config.nodeHeight,
  );
  const height =
    topReserve + config.paddingY + stateBandHeight + config.paddingY + bottomReserve;
  const width = Math.max(
    700,
    config.paddingX * 2 + maxDepth * config.columnWidth + config.nodeWidth,
  );
  const states = [];

  for (const [columnDepth, column] of [...columns.entries()].sort((a, b) => a[0] - b[0])) {
    const columnHeight = (column.length - 1) * config.rowHeight + config.nodeHeight;
    const startY =
      topReserve + config.paddingY + Math.max(0, (stateBandHeight - columnHeight) / 2);

    for (const [row, name] of column.entries()) {
      const source = stateByName.get(name);
      states.push({
        name,
        colorIndex: colorIndexByName.get(name),
        depth: columnDepth,
        initial: name === machine.initial,
        wildcard: name === "*",
        range: source?.range,
        incoming: edges.filter((edge) => edge.target === name).length,
        outgoing: edges.filter((edge) => edge.source === name).length,
        x: config.paddingX + columnDepth * config.columnWidth,
        y: startY + row * config.rowHeight,
      });
    }
  }

  return {
    bottomReserve,
    height,
    maxDepth,
    routes,
    states,
    topReserve,
    width,
  };
}

function orthogonalPath(points, radius = 10) {
  if (points.length < 2) return "";
  let path = "M " + points[0].x + " " + points[0].y;

  for (let index = 1; index < points.length - 1; index += 1) {
    const previous = points[index - 1];
    const corner = points[index];
    const next = points[index + 1];
    const incoming = Math.hypot(corner.x - previous.x, corner.y - previous.y);
    const outgoing = Math.hypot(next.x - corner.x, next.y - corner.y);
    const bend = Math.min(radius, incoming / 2, outgoing / 2);
    const before = {
      x: corner.x - Math.sign(corner.x - previous.x) * bend,
      y: corner.y - Math.sign(corner.y - previous.y) * bend,
    };
    const after = {
      x: corner.x + Math.sign(next.x - corner.x) * bend,
      y: corner.y + Math.sign(next.y - corner.y) * bend,
    };
    path += " L " + before.x + " " + before.y;
    path += " Q " + corner.x + " " + corner.y + " " + after.x + " " + after.y;
  }

  const last = points.at(-1);
  return path + " L " + last.x + " " + last.y;
}

function connectionGeometry(edge, route, source, target, layout, canvas, config) {
  const anchorOffset = Math.max(-20, Math.min(20, route.offset * 0.4));
  let labelX;
  let labelY;
  let pathData;
  let points;

  if (route.kind === "loop") {
    const side = route.sourceDepth <= layout.maxDepth / 2 ? "left" : "right";
    const direction = side === "left" ? -1 : 1;
    const edgeX = side === "left" ? source.left : source.right;
    const outerX = edgeX + direction * (48 + route.loopIndex * 18);
    const anchorSpread = Math.min(14, route.loopIndex * 4);
    const startY = source.centerY - 16 - anchorSpread;
    const endY = source.centerY + 16 + anchorSpread;
    points = [
      { x: edgeX, y: startY },
      { x: outerX, y: startY },
      { x: outerX, y: endY },
      { x: edgeX, y: endY },
    ];
    pathData = orthogonalPath(points);
    labelX = edgeX + direction * 110;
    labelY = source.top + 12 + route.loopIndex * 26;
  } else if (route.kind === "longForward") {
    const corridorY = 24 + route.lane * config.routeGap;
    const start = { x: source.right, y: source.centerY + anchorOffset };
    const end = { x: target.left, y: target.centerY + anchorOffset };
    points = [
      start,
      { x: start.x + 32, y: start.y },
      { x: start.x + 32, y: corridorY },
      { x: end.x - 32, y: corridorY },
      { x: end.x - 32, y: end.y },
      end,
    ];
    pathData = orthogonalPath(points);
    labelX = (start.x + end.x) / 2;
    labelY = corridorY;
  } else if (route.kind === "backward") {
    const corridorY = canvas.height - 24 - route.lane * config.routeGap;
    const start = { x: source.left, y: source.centerY + anchorOffset };
    const end = { x: target.right, y: target.centerY + anchorOffset };
    points = [
      start,
      { x: start.x - 32, y: start.y },
      { x: start.x - 32, y: corridorY },
      { x: end.x + 32, y: corridorY },
      { x: end.x + 32, y: end.y },
      end,
    ];
    pathData = orthogonalPath(points);
    labelX = (start.x + end.x) / 2;
    labelY = corridorY;
  } else if (route.kind === "vertical") {
    const side = route.side || "right";
    const direction = side === "left" ? -1 : 1;
    const startX = side === "left" ? source.left : source.right;
    const endX = side === "left" ? target.left : target.right;
    const corridorX =
      (side === "left"
        ? Math.min(source.left, target.left)
        : Math.max(source.right, target.right)) +
      direction * (36 + route.lane * 18);
    const start = { x: startX, y: source.centerY + anchorOffset };
    const end = { x: endX, y: target.centerY + anchorOffset };
    points = [
      start,
      { x: corridorX, y: start.y },
      { x: corridorX, y: end.y },
      end,
    ];
    pathData = orthogonalPath(points);
    labelX = corridorX;
    labelY = (start.y + end.y) / 2;
  } else {
    const offset = route.offset;
    const x1 = source.right;
    const y1 = source.centerY + anchorOffset;
    const x2 = target.left;
    const y2 = target.centerY + anchorOffset;
    const bend = Math.max(56, Math.abs(x2 - x1) * 0.44);
    pathData = "M " + x1 + " " + y1 + " C " + (x1 + bend) + " " +
      (y1 + offset) + ", " + (x2 - bend) + " " + (y2 + offset) +
      ", " + x2 + " " + y2;
    labelX = (x1 + x2) / 2;
    labelY = (y1 + y2) / 2 + offset * 0.72;
  }

  return { labelX, labelY, pathData, points };
}

function stateMachinePreviewLoadingHtml(webview, source) {
  return previewDocument(
    webview,
    `<main class="message-state"><div class="spinner" aria-hidden="true"></div><strong>Reading state machines…</strong><span>${escapeHtml(source)}</span></main>`,
    "",
  );
}

function stateMachinePreviewErrorHtml(webview, source, message) {
  return previewDocument(
    webview,
    `<main class="message-state error"><strong>State-machine preview unavailable</strong><span>${escapeHtml(message)}</span><small>${escapeHtml(source)}</small><button id="reload" type="button">Reload</button></main>`,
    `document.getElementById("reload").addEventListener("click", () => vscode.postMessage({ command: "reload" }));`,
  );
}

function stateMachinePreviewHtml(webview, viewModel) {
  const data = jsonForHtml(viewModel);
  const body = `
    <header class="toolbar">
      <div class="title-block">
        <h1>State Machine</h1>
        <span class="source">${escapeHtml(viewModel.sourceLabel)}</span>
      </div>
      <div class="toolbar-actions">
        <label class="machine-picker">
          <span>Machine</span>
          <select id="machine-picker"></select>
        </label>
        <span id="summary" class="summary"></span>
        <button id="reload" class="icon-button" type="button" title="Reload graph" aria-label="Reload graph">&#8635;</button>
      </div>
    </header>
    <main class="workspace">
      <section class="graph-panel" aria-label="State transition graph">
        <div class="graph-legend">
          <span><i class="legend-dot initial"></i>Initial state</span>
          <span><i class="legend-line"></i>Transition event</span>
          <span><i class="legend-dot wildcard"></i>Any state</span>
        </div>
        <div id="graph-scroll" class="graph-scroll">
          <div id="graph" class="graph">
            <svg id="connections" class="connections" aria-hidden="true">
              <defs>
                <marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto" markerUnits="strokeWidth">
                  <path d="M 0 0 L 8 4 L 0 8 z"></path>
                </marker>
              </defs>
            </svg>
            <div id="nodes" class="nodes"></div>
            <div id="edge-labels" class="edge-labels"></div>
          </div>
        </div>
      </section>
      <aside id="inspector" class="inspector" aria-live="polite"></aside>
    </main>`;

  const script = `
    const preview = ${data};
    const picker = document.getElementById("machine-picker");
    const summary = document.getElementById("summary");
    const graph = document.getElementById("graph");
    const graphScroll = document.getElementById("graph-scroll");
    const nodes = document.getElementById("nodes");
    const connections = document.getElementById("connections");
    const edgeLabels = document.getElementById("edge-labels");
    const inspector = document.getElementById("inspector");
    let machineIndex = 0;
    let selection = { kind: "machine" };
    let positionedNodes = new Map();
    let renderedEdges = [];
    let renderedLayout;

    const graphLayoutConfig = ${jsonForHtml(GRAPH_LAYOUT)};
    ${statePastelHue.toString()}
    ${connectionRoutes.toString()}
    ${stateLayout.toString()}
    ${orthogonalPath.toString()}
    ${connectionGeometry.toString()}

    for (const [index, machine] of preview.machines.entries()) {
      const option = document.createElement("option");
      option.value = String(index);
      option.textContent = machine.name;
      picker.append(option);
    }
    picker.hidden = preview.machines.length < 2;
    picker.parentElement.hidden = preview.machines.length < 2;
    picker.addEventListener("change", () => {
      machineIndex = Number(picker.value) || 0;
      selection = { kind: "machine" };
      render();
    });
    document.getElementById("reload").addEventListener("click", () => {
      vscode.postMessage({ command: "reload" });
    });
    window.addEventListener("resize", debounce(() => requestAnimationFrame(drawConnections), 80));

    render();

    function currentMachine() {
      return preview.machines[machineIndex];
    }

    function render() {
      const machine = currentMachine();
      picker.value = String(machineIndex);
      summary.textContent = machine.states.length + " states / " +
        machine.transitions.length + " transitions";
      renderGraph(machine);
      renderInspector(machine);
      requestAnimationFrame(drawConnections);
    }

    function renderGraph(machine) {
      nodes.replaceChildren();
      edgeLabels.replaceChildren();
      clearConnectionPaths();
      positionedNodes = new Map();
      renderedEdges = expandedEdges(machine);

      const layout = stateLayout(machine, renderedEdges, graphLayoutConfig);
      renderedLayout = layout;
      graph.style.width = layout.width + "px";
      graph.style.height = layout.height + "px";

      for (const state of layout.states) {
        const button = document.createElement("button");
        const selected = selection.kind === "state" && selection.name === state.name;
        button.type = "button";
        button.className = "state-node" +
          (state.initial ? " initial" : "") +
          (state.wildcard ? " wildcard" : "") +
          (selected ? " selected" : "");
        button.style.left = state.x + "px";
        button.style.top = state.y + "px";
        if (!state.wildcard) {
          button.style.setProperty("--state-hue", statePastelHue(state.colorIndex) + "deg");
        }
        button.dataset.state = state.name;
        button.title = state.wildcard ? "Wildcard transition source" : "State " + state.name;

        const name = document.createElement("span");
        name.className = "state-name";
        name.textContent = state.wildcard ? "Any state" : state.name;
        button.append(name);

        const meta = document.createElement("span");
        meta.className = "state-meta";
        meta.textContent = state.initial ? "initial" : state.wildcard ? "*" : state.incoming +
          " in / " + state.outgoing + " out";
        button.append(meta);

        button.addEventListener("click", () => {
          selection = { kind: "state", name: state.name };
          render();
        });
        if (!state.wildcard) {
          button.addEventListener("dblclick", () => openLocation(state.range));
        }

        positionedNodes.set(state.name, { element: button, state });
        nodes.append(button);
      }
    }

    function expandedEdges(machine) {
      const edges = [];
      for (const transition of machine.transitions) {
        for (const source of transition.from) {
          edges.push({ source, target: transition.to, transition });
        }
        if (transition.wildcardFrom) {
          edges.push({ source: "*", target: transition.to, transition, wildcard: true });
        }
      }
      return edges;
    }

    function drawConnections() {
      clearConnectionPaths();
      edgeLabels.replaceChildren();
      const graphRect = graph.getBoundingClientRect();
      connections.setAttribute("viewBox", "0 0 " + graph.clientWidth + " " + graph.clientHeight);
      connections.setAttribute("width", graph.clientWidth);
      connections.setAttribute("height", graph.clientHeight);

      for (const [edgeIndex, edge] of renderedEdges.entries()) {
        const sourceEntry = positionedNodes.get(edge.source);
        const targetEntry = positionedNodes.get(edge.target);
        if (!sourceEntry || !targetEntry) continue;

        const sourceRect = sourceEntry.element.getBoundingClientRect();
        const targetRect = targetEntry.element.getBoundingClientRect();
        const route = renderedLayout.routes[edgeIndex];
        const source = relativeRect(sourceRect, graphRect);
        const target = relativeRect(targetRect, graphRect);
        const selected = selection.kind === "transition" &&
          selection.id === edge.transition.id &&
          selection.source === edge.source;
        const geometry = connectionGeometry(
          edge,
          route,
          source,
          target,
          renderedLayout,
          { height: graph.clientHeight, width: graph.clientWidth },
          graphLayoutConfig,
        );

        const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
        path.setAttribute("d", geometry.pathData);
        path.setAttribute(
          "class",
          "connection" +
            (edge.wildcard ? " wildcard" : "") +
            (selected ? " selected" : ""),
        );
        path.setAttribute("marker-end", "url(#arrow)");
        connections.append(path);

        const label = document.createElement("button");
        label.type = "button";
        label.className = "edge-label" +
          (route.kind === "loop" ? " loop" : "") +
          (selected ? " selected" : "");
        label.textContent = edge.transition.event;
        label.style.left = geometry.labelX + "px";
        label.style.top = geometry.labelY + "px";
        label.title = edge.source + " → " + edge.target;
        label.addEventListener("click", () => {
          selection = {
            kind: "transition",
            id: edge.transition.id,
            source: edge.source,
          };
          render();
        });
        label.addEventListener("dblclick", () => openLocation(edge.transition.selectionRange));
        label.addEventListener("mouseenter", () => path.classList.add("hovered"));
        label.addEventListener("mouseleave", () => path.classList.remove("hovered"));
        edgeLabels.append(label);
      }
    }

    function relativeRect(rect, graphRect) {
      const left = rect.left - graphRect.left;
      const top = rect.top - graphRect.top;
      return {
        bottom: rect.bottom - graphRect.top,
        centerX: left + rect.width / 2,
        centerY: top + rect.height / 2,
        left,
        right: rect.right - graphRect.left,
        top,
      };
    }

    function clearConnectionPaths() {
      for (const path of connections.querySelectorAll("path.connection")) {
        path.remove();
      }
    }

    function renderInspector(machine) {
      inspector.replaceChildren();
      const header = element("header", "inspector-header");
      const title = element("button", "source-link");
      title.type = "button";
      title.textContent = machine.name;
      title.title = "Open state-machine declaration";
      title.addEventListener("click", () => openLocation(machine.selectionRange));
      header.append(title, text(machine.initial + " is initial", "inspector-subtitle"));
      inspector.append(header);

      const content = element("div", "inspector-content");
      if (selection.kind === "state") {
        renderStateInspector(content, machine, selection.name);
      } else if (selection.kind === "transition") {
        renderTransitionInspector(content, machine, selection);
      } else {
        renderMachineInspector(content, machine);
      }
      inspector.append(content);
    }

    function renderMachineInspector(root, machine) {
      root.append(sectionLabel("Topology"));
      root.append(metricRow([
        [machine.states.length, "states"],
        [machine.transitions.length, "events"],
        [expandedEdges(machine).length, "routes"],
      ]));
      root.append(sectionLabel("States"));
      const list = element("div", "item-list");
      for (const state of machine.states) {
        const button = element("button", "list-item");
        button.type = "button";
        button.append(text(state.name, "item-name"));
        if (state.initial) button.append(text("initial", "tag"));
        button.addEventListener("click", () => {
          selection = { kind: "state", name: state.name };
          render();
        });
        list.append(button);
      }
      root.append(list);
    }

    function renderStateInspector(root, machine, name) {
      const state = machine.states.find(candidate => candidate.name === name);
      if (!state) return renderMachineInspector(root, machine);
      const incoming = expandedEdges(machine).filter(edge => edge.target === name);
      const outgoing = expandedEdges(machine).filter(edge => edge.source === name);
      root.append(sectionLabel(state.initial ? "Initial state" : "State"));
      root.append(text(name, "selection-title"));
      root.append(metricRow([
        [incoming.length, "incoming"],
        [outgoing.length, "outgoing"],
      ]));
      root.append(sourceButton("Open state source", state.range));
      root.append(sectionLabel("Outgoing events"));
      root.append(transitionList(outgoing, "No events leave this state."));
      root.append(sectionLabel("Incoming events"));
      root.append(transitionList(incoming, "No events enter this state."));
    }

    function renderTransitionInspector(root, machine, selected) {
      const edge = expandedEdges(machine).find(candidate =>
        candidate.transition.id === selected.id && candidate.source === selected.source
      );
      if (!edge) return renderMachineInspector(root, machine);
      root.append(sectionLabel("Transition event"));
      root.append(text(edge.transition.event, "selection-title"));
      const route = element("div", "route");
      route.append(text(edge.source === "*" ? "Any state" : edge.source, "route-state"));
      route.append(text("→", "route-arrow"));
      route.append(text(edge.target, "route-state"));
      root.append(route);
      if (edge.wildcard) {
        root.append(text("Fires from any state without a more specific route.", "note"));
      }
      root.append(sourceButton("Open event source", edge.transition.selectionRange));
    }

    function transitionList(edges, emptyMessage) {
      const list = element("div", "item-list");
      if (edges.length === 0) {
        list.append(text(emptyMessage, "empty"));
        return list;
      }
      for (const edge of edges) {
        const button = element("button", "list-item transition-item");
        button.type = "button";
        button.append(text(edge.transition.event, "item-name"));
        button.append(text(edge.source + " → " + edge.target, "item-meta"));
        button.addEventListener("click", () => {
          selection = {
            kind: "transition",
            id: edge.transition.id,
            source: edge.source,
          };
          render();
        });
        list.append(button);
      }
      return list;
    }

    function sourceButton(label, range) {
      const button = element("button", "source-button");
      button.type = "button";
      button.textContent = label;
      button.addEventListener("click", () => openLocation(range));
      return button;
    }

    function metricRow(entries) {
      const row = element("div", "metrics");
      for (const [value, label] of entries) {
        const metric = element("div", "metric");
        metric.append(text(String(value), "metric-value"), text(label, "metric-label"));
        row.append(metric);
      }
      return row;
    }

    function sectionLabel(value) {
      return text(value, "section-label");
    }

    function openLocation(range) {
      if (!range) return;
      vscode.postMessage({
        command: "openLocation",
        uri: preview.source,
        range,
      });
    }

    function element(tag, className) {
      const node = document.createElement(tag);
      node.className = className;
      return node;
    }

    function text(value, className) {
      const node = element("span", className);
      node.textContent = value;
      return node;
    }

    function debounce(callback, delay) {
      let timer;
      return () => {
        clearTimeout(timer);
        timer = setTimeout(callback, delay);
      };
    }
  `;

  return previewDocument(webview, body, script);
}

function previewDocument(webview, body, script) {
  return webviewDocument(webview, {
    title: "Tua State Machine Preview",
    body,
    styles: STYLES,
    script,
  });
}

const STYLES = `
  :root {
    color-scheme: light dark;
    --canvas: var(--vscode-editor-background);
    --surface: var(--vscode-sideBar-background, var(--vscode-editor-background));
    --raised: var(--vscode-editorWidget-background, var(--vscode-sideBar-background));
    --border: color-mix(in srgb, var(--vscode-panel-border) 82%, transparent);
    --text: var(--vscode-foreground);
    --muted: var(--vscode-descriptionForeground);
    --accent: var(--vscode-focusBorder, var(--vscode-button-background));
    --accent-soft: color-mix(in srgb, var(--accent) 16%, transparent);
    --input: var(--vscode-input-background);
    --input-border: var(--vscode-input-border, var(--border));
    --button: var(--vscode-button-secondaryBackground, var(--vscode-button-background));
    --button-text: var(--vscode-button-secondaryForeground, var(--vscode-button-foreground));
    --button-hover: var(--vscode-button-secondaryHoverBackground, var(--vscode-button-hoverBackground));
    --danger: var(--vscode-errorForeground);
    --node-width: 158px;
    --node-height: 68px;
    font: 13px/1.45 var(--vscode-font-family);
  }

  * { box-sizing: border-box; }

  body {
    margin: 0;
    color: var(--text);
    background: var(--canvas);
    -webkit-font-smoothing: antialiased;
  }

  button, select { font: inherit; }
  button:focus-visible, select:focus-visible {
    outline: 1px solid var(--accent);
    outline-offset: 2px;
  }

  .toolbar {
    min-height: 58px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--canvas);
  }

  .title-block { min-width: 0; }
  h1 { margin: 0; font-size: 16px; line-height: 1.2; font-weight: 600; }
  .source {
    display: block;
    max-width: 70ch;
    margin-top: 2px;
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .toolbar-actions {
    display: flex;
    align-items: end;
    justify-content: end;
    gap: 10px;
  }

  .machine-picker {
    display: grid;
    gap: 2px;
    color: var(--muted);
    font-size: 11px;
    font-weight: 500;
  }

  select {
    min-width: 150px;
    height: 28px;
    padding: 2px 26px 2px 8px;
    color: var(--text);
    background: var(--input);
    border: 1px solid var(--input-border);
    border-radius: 4px;
  }

  .summary {
    min-height: 28px;
    display: inline-flex;
    align-items: center;
    color: var(--muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .icon-button {
    width: 32px;
    height: 32px;
    padding: 0;
    color: var(--button-text);
    background: var(--button);
    border: 0;
    border-radius: 4px;
    cursor: pointer;
  }
  .icon-button:hover { background: var(--button-hover); }
  .icon-button:active { transform: scale(.97); }

  .workspace {
    min-height: calc(100vh - 59px);
    display: grid;
    grid-template-columns: minmax(0, 1fr) 290px;
  }

  .graph-panel { min-width: 0; display: grid; grid-template-rows: auto 1fr; }
  .graph-legend {
    min-height: 38px;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 6px 16px;
    color: var(--muted);
    border-bottom: 1px solid var(--border);
    font-size: 11px;
  }
  .graph-legend span { display: inline-flex; align-items: center; gap: 6px; }
  .legend-dot {
    width: 9px;
    height: 9px;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--surface);
  }
  .legend-dot.initial { border-color: var(--accent); background: var(--accent-soft); }
  .legend-dot.wildcard { border-style: dashed; }
  .legend-line { width: 18px; height: 1px; background: var(--muted); }

  .graph-scroll {
    min-height: 420px;
    overflow: auto;
    background: var(--canvas);
  }
  .graph { position: relative; min-width: 100%; min-height: 100%; }
  .connections, .nodes, .edge-labels { position: absolute; inset: 0; }
  .connections { overflow: visible; pointer-events: none; }
  .connection {
    fill: none;
    stroke: color-mix(in srgb, var(--muted) 72%, transparent);
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
  }
  .connection.wildcard { stroke-dasharray: 5 4; }
  .connection.hovered, .connection.selected {
    stroke: var(--accent);
    stroke-width: 2;
  }
  #arrow path {
    fill: color-mix(in srgb, var(--muted) 72%, transparent);
    fill: context-stroke;
  }

  .state-node {
    --state-pastel: hsl(var(--state-hue, 210deg) 72% 82%);
    --state-pastel-border: hsl(var(--state-hue, 210deg) 54% 62%);
    position: absolute;
    width: var(--node-width);
    min-height: var(--node-height);
    display: grid;
    align-content: center;
    gap: 3px;
    padding: 10px 12px;
    color: var(--text);
    text-align: left;
    background: color-mix(in srgb, var(--state-pastel) 42%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--state-pastel-border) 58%, var(--border));
    border-radius: 6px;
    cursor: pointer;
  }
  .state-node:hover { border-color: color-mix(in srgb, var(--accent) 55%, var(--border)); }
  .state-node.initial {
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .state-node.wildcard {
    color: var(--muted);
    background: var(--surface);
    border-color: var(--border);
    border-style: dashed;
  }
  .state-node.selected {
    outline: 1px solid var(--accent);
    outline-offset: 1px;
  }
  .state-name {
    font-size: 14px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state-meta { color: var(--muted); font-size: 11px; font-variant-numeric: tabular-nums; }

  .edge-label {
    position: absolute;
    z-index: 2;
    min-height: 24px;
    max-width: 150px;
    transform: translate(-50%, -50%);
    padding: 2px 7px;
    color: var(--text);
    background: var(--raised);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 11px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }
  .edge-label:hover, .edge-label.selected { border-color: var(--accent); }
  .edge-label.selected { background: var(--accent-soft); }
  .edge-label.loop { max-width: 96px; }

  .inspector {
    min-width: 0;
    border-left: 1px solid var(--border);
    background: var(--surface);
  }
  .inspector-header {
    display: grid;
    gap: 3px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }
  .source-link {
    width: fit-content;
    max-width: 100%;
    padding: 0;
    color: var(--text);
    background: transparent;
    border: 0;
    font-size: 15px;
    font-weight: 600;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }
  .source-link:hover { color: var(--accent); }
  .inspector-subtitle, .note, .empty { color: var(--muted); font-size: 12px; }
  .inspector-content { display: grid; gap: 12px; padding: 16px; }
  .section-label {
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: .05em;
    text-transform: uppercase;
  }
  .selection-title { font-size: 20px; line-height: 1.2; font-weight: 600; }
  .metrics { display: grid; grid-template-columns: repeat(auto-fit, minmax(64px, 1fr)); gap: 8px; }
  .metric {
    display: grid;
    gap: 2px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 5px;
  }
  .metric-value { font-size: 18px; font-weight: 600; font-variant-numeric: tabular-nums; }
  .metric-label { color: var(--muted); font-size: 11px; }
  .item-list { display: grid; gap: 4px; }
  .list-item {
    min-height: 34px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 8px;
    color: var(--text);
    text-align: left;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
    cursor: pointer;
  }
  .list-item:hover { background: var(--raised); border-color: var(--border); }
  .item-name { min-width: 0; font-weight: 500; overflow: hidden; text-overflow: ellipsis; }
  .item-meta { color: var(--muted); font-size: 11px; }
  .tag {
    padding: 1px 6px;
    color: var(--accent);
    background: var(--accent-soft);
    border-radius: 999px;
    font-size: 10px;
  }
  .route {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    gap: 8px;
    align-items: center;
  }
  .route-state {
    padding: 7px 8px;
    text-align: center;
    background: var(--raised);
    border: 1px solid var(--border);
    border-radius: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .route-arrow { color: var(--muted); }
  .source-button {
    min-height: 32px;
    padding: 5px 10px;
    color: var(--button-text);
    background: var(--button);
    border: 0;
    border-radius: 4px;
    cursor: pointer;
  }
  .source-button:hover { background: var(--button-hover); }
  .source-button:active { transform: scale(.98); }

  .message-state {
    min-height: 100vh;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 8px;
    padding: 24px;
    color: var(--muted);
    text-align: center;
  }
  .message-state strong { color: var(--text); font-size: 15px; }
  .message-state.error strong { color: var(--danger); }
  .message-state button {
    min-height: 32px;
    margin-top: 6px;
    padding: 5px 12px;
    color: var(--button-text);
    background: var(--button);
    border: 0;
    border-radius: 4px;
    cursor: pointer;
  }
  .spinner {
    width: 20px;
    height: 20px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin .8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  @media (max-width: 760px) {
    .toolbar { align-items: start; }
    .toolbar-actions { flex-wrap: wrap; }
    .workspace { grid-template-columns: 1fr; }
    .inspector { border-left: 0; border-top: 1px solid var(--border); }
    .graph-scroll { min-height: 360px; }
  }

  @media (prefers-color-scheme: dark) {
    .state-node {
      background: color-mix(in srgb, var(--state-pastel) 26%, var(--surface));
      border-color: color-mix(in srgb, var(--state-pastel-border) 48%, var(--border));
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner { animation: none; }
  }
`;

module.exports = {
  GRAPH_LAYOUT,
  connectionGeometry,
  connectionRoutes,
  orthogonalPath,
  statePastelHue,
  stateLayout,
  stateMachinePreviewErrorHtml,
  stateMachinePreviewHtml,
  stateMachinePreviewLoadingHtml,
};
