"use strict";

function signalEventFocus(events, selection) {
  if (!selection || typeof selection.eventName !== "string" || !selection.eventName) return {};
  const named = events.filter(event => event.name === selection.eventName);
  let candidates = named;
  if (selection.busUri) {
    const sourced = candidates.filter(event => event.busUri === selection.busUri);
    if (sourced.length) candidates = sourced;
  }
  if (selection.busName) {
    const namedBus = candidates.filter(event => event.bus === selection.busName);
    if (namedBus.length) candidates = namedBus;
  }
  return { focusEventName: selection.eventName, focusEventId: candidates.length === 1 ? candidates[0].id : null };
}

module.exports = { signalEventFocus };
