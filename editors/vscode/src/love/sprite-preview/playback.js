"use strict";

// Bound preview work even when incomplete source contains an enormous range.
function expandAnimationFrames(animation, limit = 2048) {
  const ranges = animation.frames?.length
    ? animation.frames
    : [{ from: animation.from, to: animation.to, row: animation.row }];
  let count = 0;
  for (const range of ranges) {
    if (![range.from, range.to, range.row].every(Number.isSafeInteger) ||
        Math.min(range.from, range.to, range.row) < 0) {
      throw new Error("Frame coordinates must be non-negative integers.");
    }
    count += Math.abs(range.to - range.from) + 1;
    if (count > limit) throw new Error(`Animation preview supports up to ${limit} frames.`);
  }
  const frames = [];
  for (const range of ranges) {
    const direction = range.to >= range.from ? 1 : -1;
    for (let column = range.from; ; column += direction) {
      frames.push({ column, row: range.row });
      if (column === range.to) break;
    }
  }
  return frames;
}

function advancePlayback(state, durations, loop, deltaSeconds) {
  if (!state.playing || durations.length === 0) return state;
  const times = durations.map((duration) => Number.isFinite(duration) && duration > 0 ? duration : 1);
  let offset = Math.max(0, Math.min(times.length - 1, state.offset));
  let elapsed = state.elapsed + Math.max(0, Math.min(0.25, deltaSeconds));
  let playing = true;
  if (loop) {
    // Skip full cycles arithmetically, including very short frame durations.
    elapsed %= times.reduce((total, duration) => total + duration, 0);
  }
  for (let steps = 0; elapsed >= times[offset] && steps <= times.length; steps += 1) {
    elapsed -= times[offset];
    if (offset < times.length - 1) offset += 1;
    else if (loop) offset = 0;
    else {
      elapsed = 0;
      playing = false;
      break;
    }
  }
  return { offset, elapsed, playing };
}

module.exports = { advancePlayback, expandAnimationFrames };
