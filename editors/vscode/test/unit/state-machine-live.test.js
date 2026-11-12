"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const { liveStateMachineSnapshot } = require("../../src/state-machines/live");
const { stateMachinePreviewHtml } = require("../../src/state-machines/template");

const target = { sessionId: "game", objectId: 4 };
const sessions = { entries: new Map([["game", {
  session: { active: true, title: "Game" },
  status: "connected",
  received: 1234,
  snapshot: { objects: [{ id: 4, kind: "states", data: { current: "running", transition: "start" } }] },
}]]) };

test("selected state machine uses only its sampled runtime object", () => {
  assert.deepEqual(liveStateMachineSnapshot(sessions, target), {
    status: "connected", sessionTitle: "Game", state: "running", transition: "start", receivedAt: 1234,
  });
  assert.equal(liveStateMachineSnapshot(sessions, { ...target, objectId: 8 }).status, "missing");
  assert.equal(liveStateMachineSnapshot({ entries: new Map() }, target).status, "unavailable");
  assert.equal(liveStateMachineSnapshot(undefined, target), undefined);
});

test("state machine snapshots distinguish awaiting and ended sessions", () => {
  const entry = sessions.entries.get("game");
  const awaiting = { entries: new Map([["game", { ...entry, snapshot: undefined }]]) };
  assert.equal(liveStateMachineSnapshot(awaiting, target).status, "waiting");
  const ended = { entries: new Map([["game", { ...entry, session: { active: false, title: "Game" } }]]) };
  assert.equal(liveStateMachineSnapshot(ended, target).status, "ended");
});

test("preview marks the runtime sample as an overlay separate from static transitions", () => {
  const range = { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } };
  const html = stateMachinePreviewHtml({ cspSource: "test" }, {
    source: "file:///state.tua", sourceLabel: "state.tua",
    runtime: liveStateMachineSnapshot(sessions, target),
    machines: [{ name: "game", initial: "idle", range, states: [{ name: "idle", range }, { name: "running", range }], transitions: [] }],
  });
  assert.match(html, /id="runtime-status"[^>]*aria-live="polite"/);
  assert.match(html, /tua\/liveStateMachine/);
  assert.match(html, /\.state-node\.live-current/);
  assert.match(html, /"state":"running"/);
});
