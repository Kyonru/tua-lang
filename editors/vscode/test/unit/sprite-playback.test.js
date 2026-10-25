"use strict";

const assert = require("assert/strict");
const test = require("node:test");
const { advancePlayback, expandAnimationFrames } = require("../../src/love/sprite-preview/playback");

test("sprite ranges support multiple rows and descending frames", () => {
  assert.deepEqual(expandAnimationFrames({ frames: [{ from: 2, to: 0, row: 1 }, { from: 0, to: 1, row: 2 }] }), [
    { column: 2, row: 1 }, { column: 1, row: 1 }, { column: 0, row: 1 },
    { column: 0, row: 2 }, { column: 1, row: 2 },
  ]);
  assert.throws(() => expandAnimationFrames({ from: 0, to: 1e9, row: 0 }), /2048 frames/);
  assert.throws(() => expandAnimationFrames({ from: -1, to: 0, row: 0 }), /non-negative integers/);
});

test("playback honors per-frame durations and pauses at the final frame", () => {
  const start = { offset: 0, elapsed: 0, playing: true };
  const next = advancePlayback(start, [0.1, 0.2, 0.1], false, 0.15);
  assert.equal(next.offset, 1);
  assert(Math.abs(next.elapsed - 0.05) < 1e-9);
  const end = advancePlayback({ offset: 2, elapsed: 0, playing: true }, [0.1, 0.2, 0.1], false, 0.2);
  assert.deepEqual(end, { offset: 2, elapsed: 0, playing: false });
  assert.equal(advancePlayback(end, [0.1, 0.2, 0.1], false, 0.2), end);
});

test("playback bounds background delays and skips tiny-duration cycles", () => {
  const state = { offset: 0, elapsed: 0, playing: true };
  assert.deepEqual(advancePlayback(state, [1, 1], true, 3600), { offset: 0, elapsed: 0.25, playing: true });
  const fast = advancePlayback(state, [1e-12, 1e-12], true, 0.25);
  assert(fast.offset >= 0 && fast.offset < 2);
  assert(fast.elapsed >= 0 && fast.elapsed < 1e-12);
  assert.deepEqual(advancePlayback(state, [0.1, 0.1], true, -1), state);
});

