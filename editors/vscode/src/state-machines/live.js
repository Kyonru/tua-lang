"use strict";

function liveStateMachineSnapshot(sessions, target) {
  if (!sessions || !target) return undefined;
  const entry = sessions.entries.get(target.sessionId);
  if (!entry) return { status: "unavailable" };
  const base = {
    status: entry.session.active ? entry.status : "ended",
    sessionTitle: entry.session.title,
  };
  const object = entry.snapshot?.objects.find(
    candidate => candidate.id === target.objectId && candidate.kind === "states",
  );
  if (!object) {
    return { ...base, status: entry.snapshot ? "missing" : base.status === "connected" ? "waiting" : base.status };
  }
  return {
    ...base,
    state: typeof object.data?.current === "string" ? object.data.current : undefined,
    transition: typeof object.data?.transition === "string" ? object.data.transition : undefined,
    receivedAt: entry.received,
  };
}

module.exports = { liveStateMachineSnapshot };
