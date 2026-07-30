"use strict";

function stateMachinePreviewViewModel(document, inspection) {
  const machines = inspection.items
    .filter((item) => item?.kind === "stateMachine" && typeof item.name === "string")
    .map(machineViewModel);

  return {
    analysisRevision: finiteInteger(inspection.analysisRevision),
    documentVersion: finiteInteger(inspection.documentVersion),
    source: document.uri.toString(),
    sourceLabel: document.uri.fsPath || document.uri.toString(),
    machines,
    summary: {
      machineCount: machines.length,
      stateCount: machines.reduce((count, machine) => count + machine.states.length, 0),
      transitionCount: machines.reduce(
        (count, machine) => count + machine.transitions.length,
        0,
      ),
    },
  };
}

function machineViewModel(machine) {
  const states = (Array.isArray(machine.states) ? machine.states : [])
    .filter((state) => typeof state?.name === "string" && state?.range)
    .map((state) => ({
      name: state.name,
      initial: state.initial === true || state.name === machine.initial,
      range: state.range,
    }));
  const stateNames = new Set(states.map((state) => state.name));
  const transitions = (Array.isArray(machine.transitions) ? machine.transitions : [])
    .filter(
      (transition) =>
        typeof transition?.event === "string" &&
        typeof transition?.to === "string" &&
        transition?.range,
    )
    .map((transition, index) => ({
      id: `${machine.name}:${transition.event}:${index}`,
      event: transition.event,
      from: (Array.isArray(transition.from) ? transition.from : []).filter(
        (state) => typeof state === "string" && stateNames.has(state),
      ),
      wildcardFrom: transition.wildcardFrom === true,
      to: transition.to,
      range: transition.range,
      selectionRange: transition.selectionRange || transition.range,
    }));

  return {
    name: machine.name,
    initial: typeof machine.initial === "string" ? machine.initial : "none",
    range: machine.range,
    selectionRange: machine.selectionRange || machine.range,
    states,
    transitions,
  };
}

function finiteInteger(value) {
  const number = Number(value);
  return Number.isInteger(number) && number >= 0 ? number : undefined;
}

module.exports = {
  stateMachinePreviewViewModel,
};
