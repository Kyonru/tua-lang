"use strict";

// An explicit, versioned store convention. Consume only serialized runtime
// evidence; never infer causes from source text or retain gameplay objects.
const MAX_EVENTS = 128;
const id = value => Number.isSafeInteger(value) && value >= 0;
const text = value => typeof value === "string" ? value.slice(0, 160) : "";
const values = value => value && typeof value === "object" ? Object.values(value) : [];

function causalTraceNodes(state, prefix) {
  if (state?.inspectionKind !== "causal-trace/v1") return null;
  const base = `${prefix}/causal/${id(state.epoch) ? state.epoch : "unknown"}`;
  const sampled = values(state.events);
  const events = sampled.slice(0, MAX_EVENTS).filter(event => event && typeof event === "object"
    && id(event.sequence) && id(event.attackId) && event.attackId > 0
    && id(event.rootId) && event.rootId > 0 && id(event.parentId));
  events.sort((a, b) => a.sequence - b.sequence);
  const groups = new Map();
  for (const event of events) {
    if (!groups.has(event.rootId)) groups.set(event.rootId, new Map());
    const group = groups.get(event.rootId);
    if (!group.has(event.attackId)) group.set(event.attackId, []);
    group.get(event.attackId).push(event);
  }
  const roots = [...groups].sort(([a], [b]) => a - b).map(([rootId, attacks]) => {
    const rootPrefix = `${base}/root/${rootId}`;
    const nodes = new Map();
    const parents = new Map();
    for (const [attackId, evidence] of attacks) {
      const event = evidence[0];
      const consistent = evidence.every(row => row.parentId === event.parentId);
      if (consistent) parents.set(attackId, event.parentId);
      const suppressed = evidence.filter(row => row.phase === "suppressed").length;
      nodes.set(attackId, {
        id: `${rootPrefix}/attack/${attackId}`,
        label: `Attack #${attackId}`,
        description: `${text(event.sourceKind) || "unknown source"}${suppressed ? ` · ${suppressed} suppressed` : ""}`,
        tooltip: `Root #${rootId}\nParent ${consistent ? `#${event.parentId}` : "conflicts in sample"}\nSource: ${text(event.sourceId) || "unknown"}`,
        icon: suppressed ? "warning" : "git-commit",
        children: evidence.map((row, index) => ({
          id: `${rootPrefix}/attack/${attackId}/event/${row.sequence}/${index}`,
          label: `${text(row.phase) || "event"}: ${text(row.reason) || "unspecified"}`,
          description: `#${row.sequence}${row.targetId !== undefined ? ` · target ${text(String(row.targetId))}` : ""}`,
          tooltip: `Observed event #${row.sequence}\n${text(row.reason)}\nValue: ${typeof row.value === "number" && Number.isFinite(row.value) ? row.value : "unknown"}`,
          icon: row.phase === "suppressed" ? "warning" : "circle-small-filled",
        })),
      });
    }
    // Parent chains may be partial, cyclic or contradictory in user-provided
    // data. Bound traversal and keep uncertain nodes at the root, with labels.
    const top = [];
    for (const [attackId, node] of nodes) {
      const parent = parents.get(attackId);
      let cursor = parent;
      const seen = new Set([attackId]);
      let cycle = false;
      while (cursor && nodes.has(cursor)) {
        if (seen.has(cursor) || seen.size >= 16) { cycle = true; break; }
        seen.add(cursor);
        cursor = parents.get(cursor);
      }
      if (!cycle && parent && nodes.has(parent)) {
        nodes.get(parent).children.push(node);
      } else {
        if (cycle) node.description += " · cyclic/deep parent chain";
        else if (parent === undefined) node.description += " · conflicting parents";
        else if (parent) node.description += ` · parent #${parent} outside sample`;
        top.push(node);
      }
    }
    const suppressed = [...attacks.values()].flat().filter(event => event.phase === "suppressed").length;
    return { id: rootPrefix, label: `Root #${rootId}`, icon: "type-hierarchy",
      description: `${attacks.size} attacks · ${suppressed} suppressed`, children: top };
  });
  const omitted = sampled.length - events.length;
  const overwritten = id(state.overwritten) ? state.overwritten : 0;
  const children = roots.length ? roots : [{ id: base + "/empty", label: state.enabled === false ? "Trace disabled in game" : "No causal events sampled", icon: "info" }];
  if (omitted || overwritten) children.unshift({ id: base + "/limits", label: "Partial trace",
    description: `${overwritten} overwritten · ${omitted} omitted/invalid`, icon: "info",
    tooltip: "Sampling and retention can omit parents or entire chains. Missing events are not proof that an effect did not happen." });
  if (values(state.stats).length) children.push({ id: base + "/stats", label: "Stat provenance",
    description: `Root ${id(state.statRootId) && state.statRootId > 0 ? `#${state.statRootId}` : "unknown"} · ${text(state.weaponId) || "unknown source"}`,
    children: values(state.stats).slice(0, 32).map((entry, index) => ({ id: `${base}/stats/${index}`, label: text(entry) })) });
  return { id: base, label: "Causal trace", description: `${events.length} sampled events${state.enabled === false ? " · disabled" : ""}`,
    tooltip: "Sampled runtime evidence grouped by root and parent attack. Expand suppressed events for their reasons.",
    icon: "type-hierarchy", children };
}
module.exports = { causalTraceNodes };
