"use strict";

const fs = require("fs");
const path = require("path");
const {
  cacheHitRate,
  formatCacheStats,
  profileLspProject,
  profileSpans,
  repositoryState,
  resolveCommand,
  spanStatsByName,
  timingsByLabel,
  statsForValues,
} = require("./lsp-profile-common");

const command = resolveCommand(process.argv[2] || "target/debug/tua");
const root = path.resolve(process.argv[3] || "benchmarks/lsp/medium");
const repeat = Number(process.env.TUA_PROFILE_REPEAT || "20");
const warmup = Number(process.env.TUA_PROFILE_WARMUP || "2");
const outputPath =
  process.env.TUA_COMPLETION_PROFILE_OUT ||
  path.resolve("target", "tua-speed", "completion-profile.json");
const baselinePath = process.env.TUA_COMPLETION_BASELINE;
const baselineTolerance = Number(process.env.TUA_COMPLETION_BASELINE_TOLERANCE || "0.05");
const p95TargetMs = Number(process.env.TUA_COMPLETION_P95_TARGET_MS || "50");
const rankingP95TargetMs = Number(process.env.TUA_COMPLETION_RANKING_P95_TARGET_MS || "5");

(async () => {
  const result = await profileLspProject({
    command,
    root,
    repeat,
    warmup,
    sourceRoot: path.join(root, "src"),
    env: {
      ...process.env,
      TUA_PROFILE: process.env.TUA_PROFILE ?? "1",
    },
  });
  const report = buildReport(result);

  printReport(report);
  writeReport(report);
  compareBaseline(report);
})().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});

function buildReport(result) {
  const completionLatency = completionLatencyStats(result.timings);
  const rankingOverhead = rankingOverheadStats(result.stderr);
  const completionContext = result.stats.completionContext || { hits: 0, misses: 0 };

  return {
    repository: repositoryState(),
    root,
    repeat,
    warmup,
    completionLatency,
    rankingOverhead,
    cacheStats: {
      completionContext: {
        hits: completionContext.hits || 0,
        misses: completionContext.misses || 0,
        hitRate: cacheHitRate(completionContext),
      },
      raw: result.stats,
    },
    modelLane: {
      enabled: false,
      requests: 0,
      fallbacks: 0,
      fallbackRate: 0,
    },
    thresholds: {
      mediumCompletionP95TargetMs: p95TargetMs,
      rankingP95TargetMs,
    },
  };
}

function completionLatencyStats(timings) {
  const metrics = {};

  for (const [label, values] of timingsByLabel(timings)) {
    if (!label.startsWith("completion")) {
      continue;
    }

    metrics[label] = statsForValues(values);
  }

  if (!metrics.completion) {
    throw new Error("completion benchmark did not record textDocument/completion timings");
  }

  return metrics;
}

function rankingOverheadStats(stderr) {
  const spans = profileSpans(stderr, "analysis.completion.ranking");
  const byName = spanStatsByName(spans);

  return {
    spans: byName,
    aggregate: spans.length > 0 ? statsForValues(spans.map((span) => span.elapsedMs)) : null,
  };
}

function printReport(report) {
  console.log(`\n# completion benchmark (${path.basename(report.root)})`);
  console.log(`root ${report.root}`);
  console.log(`repeat ${report.repeat} warmup ${report.warmup}`);

  console.log("\ncompletion latency");
  for (const [label, stats] of Object.entries(report.completionLatency)) {
    console.log(
      `${label.padEnd(24)} p50 ${stats.p50Ms.toFixed(2)}ms  ` +
        `p95 ${stats.p95Ms.toFixed(2)}ms  p99 ${stats.p99Ms.toFixed(2)}ms  ` +
        `avg ${stats.avgMs.toFixed(2)}ms  runs ${stats.runs}`,
    );
  }

  console.log("\ncompletion context cache");
  const context = report.cacheStats.completionContext;
  console.log(
    `hits ${context.hits}  misses ${context.misses}  hitRate ${(context.hitRate * 100).toFixed(1)}%`,
  );
  console.log(formatCacheStats(report.cacheStats.raw));

  console.log("\nranking overhead");
  if (report.rankingOverhead.aggregate) {
    const stats = report.rankingOverhead.aggregate;

    console.log(
      `analysis.completion.ranking p50 ${stats.p50Ms.toFixed(3)}ms  ` +
        `p95 ${stats.p95Ms.toFixed(3)}ms  p99 ${stats.p99Ms.toFixed(3)}ms  runs ${stats.runs}`,
    );
  } else {
    console.log("analysis.completion.ranking unavailable; run with TUA_PROFILE=1");
  }

  console.log("\noptional model lane");
  console.log(
    `enabled ${report.modelLane.enabled}  requests ${report.modelLane.requests}  ` +
      `fallbacks ${report.modelLane.fallbacks}  fallbackRate ${(report.modelLane.fallbackRate * 100).toFixed(1)}%`,
  );

  console.log(
    `\ntargets: medium completion p95 <= ${report.thresholds.mediumCompletionP95TargetMs}ms, ` +
      `ranking p95 <= ${report.thresholds.rankingP95TargetMs}ms`,
  );
}

function writeReport(report) {
  if (!outputPath) {
    return;
  }

  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  console.log(`wrote ${outputPath}`);
}

function compareBaseline(report) {
  if (!baselinePath) {
    return;
  }

  const baseline = JSON.parse(fs.readFileSync(baselinePath, "utf8"));
  const currentP95 = report.completionLatency.completion.p95Ms;
  const baselineP95 = baseline.completionLatency?.completion?.p95Ms;

  if (typeof baselineP95 !== "number") {
    throw new Error(`baseline ${baselinePath} does not include completionLatency.completion.p95Ms`);
  }

  const allowed = Math.max(baselineP95 * (1 + baselineTolerance), baselineP95 + 1);

  if (currentP95 > allowed) {
    throw new Error(
      `completion p95 regressed: current ${currentP95.toFixed(2)}ms, ` +
        `baseline ${baselineP95.toFixed(2)}ms, allowed ${allowed.toFixed(2)}ms`,
    );
  }

  console.log(
    `baseline ok: completion p95 ${currentP95.toFixed(2)}ms <= ${allowed.toFixed(2)}ms`,
  );
}
