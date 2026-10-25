"use strict";

function shortValue(value) {
  if (typeof value === "number" && Number.isFinite(value)) return String(Number.isInteger(value) ? value : Number(value.toPrecision(6)));
  const text = typeof value === "string" ? JSON.stringify(value) : String(value);
  return text.length > 64 ? text.slice(0, 61) + "…" : text;
}
function tableSummary(value) {
  const keys = Object.keys(value);
  if (keys.length >= 2 && keys.length <= 4 && keys.every(key => ["x", "y", "z", "w"].includes(key) && typeof value[key] === "number")) {
    return keys.map(key => `${key} ${shortValue(value[key])}`).join(" · ");
  }
  return `${keys.length} ${keys.length === 1 ? "field" : "fields"}`;
}
function row(prefix, key, label, value, tooltip) {
  return { id: `${prefix}/${key}`, label, description: shortValue(value), tooltip: tooltip || `${label}: ${String(value)}` };
}
function duration(seconds) {
  if (typeof seconds !== "number") return "Never";
  if (seconds < 1) return "Just now";
  if (seconds < 60) return `${Math.floor(seconds)}s ago`;
  return `${Math.floor(seconds / 60)}m ago`;
}

function valueNodes(value, prefix, depth = 0) {
  if (!value || typeof value !== "object" || depth >= 8) return [];
  return Object.keys(value).slice(0, 128).sort((a, b) => a.localeCompare(b, undefined, { numeric: true })).map(key => {
    const child = value[key];
    const nested = child !== null && typeof child === "object";
    return { id: `${prefix}/${encodeURIComponent(key)}`, label: key,
      description: nested ? tableSummary(child) : shortValue(child),
      tooltip: nested ? undefined : `${key}: ${typeof child === "string" ? JSON.stringify(child) : String(child)}`,
      children: nested ? valueNodes(child, `${prefix}/${encodeURIComponent(key)}`, depth + 1) : undefined };
  });
}

// Compare only serialized state, never runtime references or volatile age counters.
function storeNodes(data, prefix, previous) {
  const nodes = valueNodes(data, prefix);
  const state = nodes.find(node => node.label === "state");
  if (state && previous?.state && data.state && typeof data.state === "object") {
    state.children = changedValueNodes(data.state, previous.state, `${prefix}/state`);
  }
  return nodes;
}

function changedValueNodes(current, previous, prefix, depth = 0) {
  if (depth >= 8) return [];
  const keys = [...new Set([...Object.keys(current || {}), ...Object.keys(previous || {})])].slice(0, 128);
  return keys.sort((a, b) => a.localeCompare(b, undefined, { numeric: true })).map(key => {
    const present = Object.hasOwn(current || {}, key), wasPresent = Object.hasOwn(previous || {}, key);
    const value = present ? current[key] : previous[key], old = previous?.[key];
    const nested = value !== null && typeof value === "object";
    const children = nested ? changedValueNodes(present ? value : {}, wasPresent && old && typeof old === "object" ? old : {}, `${prefix}/${encodeURIComponent(key)}`, depth + 1) : undefined;
    const change = !present ? "removed" : !wasPresent ? "added" : nested
      ? (old === null || typeof old !== "object" || children.some(child => child.change) ? "changed" : undefined)
      : !Object.is(value, old) ? "changed" : undefined;
    const summary = nested ? tableSummary(value) : shortValue(value);
    return { id: `${prefix}/${encodeURIComponent(key)}`, label: key, change, children,
      description: `${summary}${change ? ` · ${change}` : ""}`,
      icon: change ? (change === "added" ? "diff-added" : change === "removed" ? "diff-removed" : "diff-modified") : undefined,
      tooltip: change === "changed" && !nested ? `Previous: ${old !== null && typeof old === "object" ? "[table]" : String(old)}\nCurrent: ${String(value)}` : nested ? undefined : `${key}: ${typeof value === "string" ? JSON.stringify(value) : String(value)}` };
  });
}


function presentObject(object, prefix, previous) {
  const data = object.data;
  const details = [];
  let summary, children;
  if (data.unavailable) return { summary: "Unavailable", children: valueNodes(data, prefix), details };
  switch (object.kind) {
    case "stores": {
      const state = storeNodes(data, prefix, previous).find(node => node.label === "state");
      children = state?.children || (Object.hasOwn(data, "state") ? [row(prefix, "state", "Value", data.state)] : []);
      const changed = children.filter(node => node.change).length;
      const fieldCount = data.state && typeof data.state === "object" ? Object.keys(data.state).length : children.length;
      summary = `${fieldCount} ${fieldCount === 1 ? "field" : "fields"}${changed ? ` · ${changed} changed` : ""}`;
      for (const [key, label] of [["updates", "Updates"], ["subscribers", "Subscribers"], ["watchers", "Field watchers"]]) {
        if (data[key] !== undefined) details.push(row(prefix, key, label, data[key]));
      }
      if (data.secondsSinceUpdate !== undefined) details.push({ id: prefix + "/secondsSinceUpdate", label: "Last update", description: duration(data.secondsSinceUpdate), tooltip: `Seconds since successful set/update: ${data.secondsSinceUpdate}` });
      if (data.watcherLimitReached) details.push({ id: prefix + "/watcherLimitReached", label: "Watcher count sampled", description: `${data.watcherKeysSampled} keys`, icon: "info" });
      break;
    }
    case "pools":
      summary = `${data.active ?? "?"} active · ${data.available ?? "?"} available`;
      children = [["active", "Active"], ["available", "Available"], ["capacity", "Capacity"]].filter(([key]) => data[key] !== undefined).map(([key, label]) => row(prefix, key, label, data[key]));
      break;
    case "states":
      summary = String(data.current ?? "Unknown state");
      children = [row(prefix, "current", "Current state", data.current ?? "Unknown")];
      if (data.transition) children.push(row(prefix, "transition", "Pending transition", data.transition));
      if (data.asyncState !== undefined) details.push(row(prefix, "asyncState", "Async phase", data.asyncState));
      break;
    case "signals": {
      const listeners = data.listeners && typeof data.listeners === "object" ? data.listeners : {};
      const count = Object.values(listeners).filter(value => typeof value === "number").reduce((sum, value) => sum + value, 0);
      summary = `${count}${Object.keys(listeners).some(key => key.startsWith("<")) ? "+" : ""} ${count === 1 ? "listener" : "listeners"} · ${data.emitted ?? 0} emitted`;
      const recent = Object.values(data.recent || {}).filter(event => event && typeof event === "object").sort((a, b) => b.sequence - a.sequence).slice(0, 20);
      children = [
        { id: prefix + "/listeners", label: "Listeners", description: String(count), children: valueNodes(listeners, prefix + "/listeners") },
        { id: prefix + "/recent", label: "Recent events", description: String(recent.length), children: recent.map((event, index) => ({ id: `${prefix}/recent/${event.sequence ?? index}`, label: String(event.name ?? "Unnamed event"), description: `#${event.sequence ?? "?"}`, tooltip: `Emission ${event.sequence ?? "?"} · newest first` })) },
      ];
      break;
    }
    case "ecs": {
      const entities = data.entities && typeof data.entities === "object" ? data.entities : {};
      const count = data.sampledEntities ?? Object.keys(entities).length;
      summary = `${count} ${count === 1 ? "entity" : "entities"} sampled`;
      children = valueNodes(entities, prefix + "/entities").map(node => ({ ...node, label: /^\d+$/.test(node.label) ? `Entity ${node.label}` : node.label, description: node.children ? `${node.children.length} ${node.children.length === 1 ? "component" : "components"}` : node.description }));
      break;
    }
    default: children = valueNodes(data, prefix);
  }
  return { summary, children, details };
}
module.exports = { valueNodes, storeNodes, presentObject, shortValue, tableSummary };
