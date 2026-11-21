"use strict";

// Optional additive v1 fields: old captures remain readable and unavailable
// context stays unknown rather than being inferred from profiler availability.
const text = value => typeof value === "string" && value.length > 0 && value.length <= 512 ? value : "unknown";
const count = value => Number.isSafeInteger(value) && value >= 0 ? value : null;
function profileMetadata(value) {
  const source = value && typeof value === "object" ? value : {};
  return {
    mode: source.mode === "profiled" ? "profiled" : "unknown",
    loveVersion: text(source.loveVersion), luaVersion: text(source.luaVersion),
    luaJitVersion: text(source.luaJitVersion),
    jitEnabledStart: typeof source.jitEnabledStart === "boolean" ? source.jitEnabledStart : null,
    jitEnabledEnd: typeof source.jitEnabledEnd === "boolean" ? source.jitEnabledEnd : null,
    target: text(source.target), buildIdentity: text(source.buildIdentity), revision: text(source.revision),
    gcPolicy: ["running", "stopped"].includes(source.gcPolicy) ? source.gcPolicy : "unknown",
    memoryMetric: "sampled Lua heap size (KiB), not cumulative allocation",
    frameMetric: "complete frame wall duration including sleep; update/draw are callback durations",
    gcMetric: "sampled GC VM activity, not exact GC duration",
    benchmark: source.benchmark && typeof source.benchmark === "object" ? {
      workload: text(source.benchmark.workload), revision: text(source.benchmark.revision),
      warmupUpdates: count(source.benchmark.warmupUpdates), repetitions: count(source.benchmark.repetitions),
    } : null,
  };
}
module.exports = { profileMetadata };
