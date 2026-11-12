"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { signalEventFocus } = require("../../src/signals/focus");

test("live signal focus prefers its source-mapped bus and leaves ambiguous events filtered", () => {
  const events = [
    { id: "one", name: "hit", bus: "enemy", busUri: "file:///enemy.tua" },
    { id: "two", name: "hit", bus: "player", busUri: "file:///player.tua" },
    { id: "three", name: "jump", bus: "player", busUri: "file:///player.tua" },
  ];
  assert.deepEqual(signalEventFocus(events, { eventName: "hit", busUri: "file:///player.tua" }), { focusEventName: "hit", focusEventId: "two" });
  assert.deepEqual(signalEventFocus(events, { eventName: "hit", busName: "enemy" }), { focusEventName: "hit", focusEventId: "one" });
  assert.deepEqual(signalEventFocus(events, { eventName: "hit" }), { focusEventName: "hit", focusEventId: null });
  assert.deepEqual(signalEventFocus(events, { eventName: "jump" }), { focusEventName: "jump", focusEventId: "three" });
  assert.deepEqual(signalEventFocus(events, { eventName: "dynamic" }), { focusEventName: "dynamic", focusEventId: null });
  assert.deepEqual(signalEventFocus(events, {}), {});
});
