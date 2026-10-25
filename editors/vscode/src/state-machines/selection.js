"use strict";

function restoreGraphState(preview, saved) {
  const empty = { machineIndex: 0, selection: { kind: "machine" }, left: 0, top: 0 };
  if (saved?.source !== preview.source) return empty;
  const machineIndex = preview.machines.findIndex((machine) => machine.name === saved.machine);
  if (machineIndex < 0) return empty;
  const machine = preview.machines[machineIndex];
  let selection = { kind: "machine" };
  const candidate = saved.selection;
  if (candidate?.kind === "state" && machine.states.some((state) => state.name === candidate.name)) {
    selection = { kind: "state", name: candidate.name };
  } else if (candidate?.kind === "transition") {
    const transition = machine.transitions.find((item) =>
      item.event === candidate.event && item.to === candidate.to &&
      (item.from.includes(candidate.source) || (candidate.source === "*" && item.wildcardFrom)));
    if (transition) selection = { kind: "transition", id: transition.id, source: candidate.source };
  }
  return {
    machineIndex,
    selection,
    left: Number.isFinite(saved.left) ? Math.max(0, saved.left) : 0,
    top: Number.isFinite(saved.top) ? Math.max(0, saved.top) : 0,
  };
}

module.exports = { restoreGraphState };
