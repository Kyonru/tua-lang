---
icon: lucide/gauge
---

# Completion Benchmarks

Tua keeps completion benchmarking local. No timing data, source text, ranking
statistics, or profile output is sent off-device.

## Commands

Run the fixed Phase 14 completion profile against the medium LSP fixture:

```bash
make profile-completion
```

The profile writes `target/tua-speed/completion-profile.json` by default and
prints:

- `completion` and `completionResolve` latency as p50, p95, p99, average, and
  run count;
- `completionContext` cache hits, misses, and hit rate from `tua/cacheStats`;
- `usageIndex` per-file summary cache hits and misses from `tua/cacheStats`;
- `analysis.completion.ranking` overhead from `TUA_PROFILE=1` profile spans;
- `analysis.index.usage.file` and `analysis.index.usage.project` index build
  costs when profiling is enabled;
- optional model-lane fallback frequency. The model lane is currently disabled,
  so the expected baseline is zero requests and zero fallbacks.

Project-aware completion does not collect or reparse project source during an
editor request. The snapshot reuses content-keyed per-file summaries, builds a
compact revisioned project usage index, and scopes completion cache entries to
that revision. A rising `usageIndex.misses` count without corresponding file
changes indicates unexpected invalidation.

Run the ranking-order regression smoke:

```bash
make completion-ranking-smoke
```

That smoke uses `benchmarks/lsp/medium` and checks representative module-field
and typed-local field completion ordering.

LSP smoke and profiling scripts set `TUA_COMPLETION_STATS=0` by default so
verification does not create or mutate fixture-local `.tua/completion-stats.json`
files. Set `TUA_COMPLETION_STATS=1` when you intentionally want a benchmark run
to include persisted adaptive completion stats.

## Contextual Sequence Ranking

The optional contextual sequence ranking lane is disabled by default. Enable it
with `tua.completion.sequenceRanking.enabled` to let the local sequence ranker
add score hints on top of compiler-generated candidates. It never creates new
completion candidates.

The ranker protocol is:

- input: normalized context before the cursor, normalized current-line prefix,
  receiver parts when available, typed prefix, and a bounded compiler candidate
  list;
- output: per-candidate confidence scores plus optional rationale tokens;
- fallback: timeout, provider error, or disabled config returns zero score
  hints and preserves the pre-sequence ordering.

The first provider is local-only and uses normalized token frequency near the
cursor. No network calls are made. Tune the bounded request with
`tua.completion.sequenceRanking.timeoutMs` and
`tua.completion.sequenceRanking.maxCandidates`.

## Multi-Token Completion

The optional multi-token completion lane is also disabled by default. Enable it
with `tua.completion.multiToken.enabled`; set
`tua.completion.multiToken.advanced` when you want suggestions even when normal
compiler completion has ordinary confidence.

The first provider is local-only and covers narrow contexts:

- assignment right-hand sides, such as vector-like locals;
- expression tails, such as fallback expressions after `return value`;
- method-chain endings for obvious audio/source-shaped expressions.

Every proposed insertion is applied to the current source and parsed before it
is converted to an LSP completion item. Unparsable insertions are rejected. The
`tua.completion.multiToken.allowModelIntegration` setting is an explicit policy
gate for future model-backed providers; no model or network provider is called
by the current implementation.

## Targets

The fixed Phase 14 success targets are:

- medium fixture `completion` p95 stays at or below 50ms after warmup;
- `analysis.completion.ranking` p95 stays at or below 5ms;
- completion-context cache hit rate is reported for every run and should improve
  after repeated same-context requests;
- optional model-lane fallback rate remains 0% while the model lane is disabled.

Machine speed varies, so local regression gates should compare against a local
baseline rather than a checked-in absolute JSON file:

```bash
make profile-completion
TUA_COMPLETION_BASELINE=target/tua-speed/completion-profile.json make profile-completion
```

Use `TUA_COMPLETION_BASELINE_TOLERANCE` to tune the allowed p95 drift. The
default tolerance is 5%, with a 1ms minimum slack for short local runs.

## Knobs

- `TUA_PROFILE_REPEAT`: measured repeat count. Defaults to `20`.
- `TUA_PROFILE_WARMUP`: unmeasured warmup count. Defaults to `2`.
- `TUA_COMPLETION_PROFILE_OUT`: JSON output path.
- `TUA_COMPLETION_BASELINE`: previous JSON report to compare against.
- `TUA_COMPLETION_P95_TARGET_MS`: printed target for medium completion p95.
- `TUA_COMPLETION_RANKING_P95_TARGET_MS`: printed target for ranking p95.
- `TUA_PROFILE=0`: disables internal profile spans if only wall-clock request
  timing is needed.
- `TUA_PROFILE_STREAM=1`: streams raw profile spans while the benchmark runs.

## Failure Modes

- A high completion p95 with low ranking overhead usually points to snapshot,
  semantic fact, module graph, or LSP transport work. Rerun with
  `TUA_PROFILE_STREAM=1` and inspect `analysis.query.completion`,
  `analysis.semantic.*`, and `lsp.completion.*` spans.
- High `analysis.completion.ranking` p95 means ranking work is too expensive for
  the candidate set. Check project usage scanning, adaptive stats lookups, and
  sort tie-breakers before adding new ranking features.
- Low completion-context hit rate after repeated requests means prefix-stable or
  exact completion cache reuse regressed. Inspect the completion request key and
  source fingerprint path.
- Non-zero model-lane fallback counts should only appear after an optional model
  ranking lane is implemented and explicitly enabled.
