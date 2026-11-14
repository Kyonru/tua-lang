"use strict";
const assert = require("node:assert/strict");
const { showProfile } = require("../../src/profiler/view");

async function runProfilerTests() {
  const sourcePath = "/tmp/profile-source.tua";
  const model = {
    reason: "duration", durationMs: 1000, frameCount: 2, retainedFrames: 2,
    p50: 8, p95: 25, worst: 25, budgetMs: 16.67, missedBudget: 1,
    sampleCount: 3, droppedSamples: 0,
    slowFrames: [{ index: 2, wallMs: 25, updateMs: 18, drawMs: 2, drawCalls: 4, gcSamples: 1, luaKb: 1024 }],
    stacks: [{ stack: "build/main.lua:8\n", count: 3, share: 1,
      location: { sourcePath, sourceLine: 4, generatedPath: "/tmp/build/main.lua", generatedLine: 8 } }],
    traces: [{ kind: "ecs", name: "system", calls: 2, totalMs: 6, worstMs: 4,
      source: { sourcePath, sourceLine: 3 } }], recentTraces: [],
    recordedInput: { seed: 42, events: [{ frame: 1, name: "keypressed", args: ["space"] }], dropped: 0 },
  };
  const panel = showProfile(model, () => {});
  try {
    assert.match(panel.webview.html, /LÖVE performance capture/u);
    assert.match(panel.webview.html, /over 16\.67 ms budget/u);
    assert.match(panel.webview.html, /data-source="0"/u);
    assert.match(panel.webview.html, /data-generated="0"/u);
    assert.match(panel.webview.html, /data-trace="0"/u);
    assert.match(panel.webview.html, /id="replay-input"/u);
    assert.equal(panel.title, "Tua Performance Capture");
  } finally { panel.dispose(); }
}

module.exports = { runProfilerTests };
