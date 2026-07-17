"use strict";

function serverHealthViewModel(raw) {
  const health = raw && typeof raw === "object" ? raw : {};
  const warm = objectValue(health.workspaceWarm);
  const queue = objectValue(health.queue);
  const cache = objectValue(health.cache);
  const stale = objectValue(health.staleResults);
  const latency = objectValue(health.latency);

  return {
    raw: health,
    refreshedAt: new Date().toISOString(),
    revision: numberValue(health.revision),
    documents: {
      open: numberValue(objectValue(health.documents).open),
      recentChanges: numberValue(objectValue(health.documents).recentChanges),
    },
    warm: {
      enabled: Boolean(warm.enabled),
      discovered: numberValue(warm.discovered),
      warmed: numberValue(warm.warmed),
      failed: numberValue(warm.failed),
      queued: numberValue(warm.queued),
      queuedHigh: numberValue(warm.queuedHigh),
      queuedLow: numberValue(warm.queuedLow),
      coveragePercent: numberValue(warm.coveragePercent),
      snapshotReadyRevision: optionalNumber(warm.snapshotReadyRevision),
      snapshotFailedRevision: optionalNumber(warm.snapshotFailedRevision),
      staleResults: numberValue(warm.staleResults),
    },
    queue: {
      total: numberValue(queue.total),
      coordinator: numberValue(queue.coordinator),
      inFlight: numberValue(queue.inFlight),
      workspaceWarmHigh: numberValue(queue.workspaceWarmHigh),
      workspaceWarmLow: numberValue(queue.workspaceWarmLow),
    },
    cache: {
      hits: numberValue(cache.hits),
      misses: numberValue(cache.misses),
      hitRate: numberValue(cache.hitRate),
      rows: cacheRows(cache),
    },
    stale: {
      total: numberValue(stale.total),
      requests: numberValue(stale.requests),
      workspaceWarm: numberValue(stale.workspaceWarm),
    },
    latency: {
      recentCount: numberValue(latency.recentCount),
      p50Ms: numberValue(latency.p50Ms),
      p95Ms: numberValue(latency.p95Ms),
      p99Ms: numberValue(latency.p99Ms),
      maxMs: numberValue(latency.maxMs),
      methods: Array.isArray(latency.methods) ? latency.methods.map(methodLatencyRow) : [],
    },
  };
}

function cacheRows(cache) {
  return [
    ["parse", "Parse"],
    ["hir", "HIR"],
    ["scope", "Scope"],
    ["typeEnvironment", "Types"],
    ["diagnostics", "Diagnostics"],
    ["moduleExports", "Module exports"],
    ["requireEdges", "Require edges"],
    ["symbolIndex", "Symbols"],
    ["usageIndex", "Usage"],
    ["completionContext", "Completion context"],
  ].map(([key, label]) => {
    const value = objectValue(cache[key]);

    return {
      key,
      label,
      hits: numberValue(value.hits),
      misses: numberValue(value.misses),
      hitRate: numberValue(value.hitRate),
    };
  });
}

function methodLatencyRow(value) {
  const row = objectValue(value);

  return {
    method: String(row.method || "unknown"),
    count: numberValue(row.count),
    p95Ms: numberValue(row.p95Ms),
    maxMs: numberValue(row.maxMs),
  };
}

function objectValue(value) {
  return value && typeof value === "object" ? value : {};
}

function numberValue(value) {
  return Number.isFinite(value) ? Number(value) : 0;
}

function optionalNumber(value) {
  return Number.isFinite(value) ? Number(value) : null;
}

module.exports = {
  serverHealthViewModel,
};
