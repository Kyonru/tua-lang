---
icon: lucide/gauge
---

# Completion Benchmark Guide

Use Tua's benchmark targets to answer three different questions:

1. Did completion ordering or semantic correctness change?
2. Did ordinary completion become slower?
3. Does completion stay fresh while project analysis is busy?

!!! info "Benchmark data stays local"
    No timing data, source text, ranking statistics, or profile output is sent
    off-device. Reports are written under `target/tua-speed/` unless an output
    path is overridden.

## Choose A Benchmark

| Question | Command | What it checks |
| --- | --- | --- |
| Did ranking order change? | `make completion-ranking-smoke` | Deterministic ordering for representative module fields and typed locals. |
| Is normal completion slower? | `make profile-completion` | End-to-end completion latency, resolve latency, ranking overhead, and cache behavior on the medium fixture. |
| Are results fresh under load? | `make representative-benchmark-smoke` | CI-sized cold, warm, and contended requests with stale-result and semantic validation gates. |
| How does scale affect the LSP? | `make profile-representative` | Full 1,000/5,000-line and 100/1,000-file profiles. |
| Do all benchmark lanes still work? | `make benchmark-smoke` | Ranking correctness, bundled-example timing, and representative completion freshness. |
| Do I need every performance report? | `make profile-all` | General LSP, completion, examples, and representative-scale profiles. |

For a completion implementation change, start with:

```bash
make completion-ranking-smoke
make profile-completion
```

Run `make representative-benchmark-smoke` when the change affects snapshots,
caching, scheduling, project indexing, cancellation, or watched files.

## Prerequisites

Benchmark targets build the required Tua binary automatically. Local runs need:

- Rust and Cargo;
- Node.js;
- enough memory for generated 1,000-file fixtures when running the full
  representative profile.

Generated fixtures and JSON reports live under `target/` and do not modify
checked-in benchmark projects.

## Profile Ordinary Completion

Run:

```bash
make profile-completion
```

The target builds the debug `tua` binary and profiles
`benchmarks/lsp/medium`. It performs unmeasured warmup requests, then repeated
measured requests in one LSP session. The default report is:

```text
target/tua-speed/completion-profile.json
```

### Read The Latency Numbers

Each request label reports:

| Metric | Meaning |
| --- | --- |
| `p50` | Median request. Half of measured requests completed at or below this time. |
| `p95` | Tail latency. Only about 5% of requests were slower. Use this as the primary regression signal. |
| `p99` | Worst-tail signal. Useful for stalls, but noisy with short local runs. |
| `avg` | Arithmetic mean. Sensitive to one unusually slow request. |
| `runs` | Number of measured samples after warmup. |

`completion` is wall-clock time for `textDocument/completion`.
`completionResolve` measures documentation/detail resolution for a selected
completion item. End-to-end completion includes protocol, snapshot, semantic,
candidate, ranking, and response-conversion work.

!!! tip "Prefer p95 over one fast or slow run"
    Compare repeated runs on the same machine, power state, build profile, and
    fixture. A single request or average alone is not a reliable regression
    signal.

### Separate Ranking From Total Time

`analysis.completion.ranking` is an internal profile span inside the full
completion request:

- low ranking p95 with high completion p95 points outside ranking;
- high ranking p95 points to candidate scoring, project usage statistics,
  adaptive data, or sorting;
- no ranking span means internal profiling was disabled.

The printed medium-fixture reference targets are:

- completion p95 at or below 50ms;
- ranking p95 at or below 5ms.

These values are trend targets, not automatic failures for
`make profile-completion`. The command fails on harness errors or when an
explicit baseline comparison exceeds its tolerance.

### Read Cache Statistics

The report includes raw `tua/cacheStats` counters.

| Counter | Healthy interpretation |
| --- | --- |
| `completionContext` | Repeated compatible completion contexts should produce hits. |
| `usageIndex` | Unchanged files should reuse compact per-file usage summaries. |
| `parse`, `hir`, `scope`, `typeEnvironment` | Warm requests should reuse semantic work rather than rebuilding every layer. |
| `invalidations` | Should rise when inputs change, not during identical requests. |
| `workspaceWarm` | Shows background project coverage, queue depth, stale results, and prepared snapshot revision. |

A low hit rate is not automatically a bug on the first request. It becomes
suspicious when repeated identical or prefix-compatible requests continue to
miss without source or configuration changes.

The report's `modelLane` object is currently fixed to disabled with zero
requests and fallbacks. It is reserved telemetry, not evidence that a
network/model provider ran.

## Compare Against A Baseline

Use a baseline from the same machine and keep baseline/current outputs
separate:

```bash
TUA_COMPLETION_PROFILE_OUT=target/tua-speed/completion-baseline.json make profile-completion

TUA_COMPLETION_BASELINE=target/tua-speed/completion-baseline.json \
TUA_COMPLETION_PROFILE_OUT=target/tua-speed/completion-current.json \
make profile-completion
```

!!! warning "Do not reuse the baseline as the current output"
    The profiler writes its current report before loading the baseline. If both
    paths are the same, the current run overwrites the baseline and compares
    against itself.

The comparison uses `completionLatency.completion.p95Ms`. By default, current
p95 may be at most 5% slower than baseline, with at least 1ms of slack for very
short runs.

Set `TUA_COMPLETION_BASELINE_TOLERANCE` to a decimal ratio such as `0.10` for
10%. Prefer increasing repeats before increasing tolerance.

## Check Ranking Correctness

Run:

```bash
make completion-ranking-smoke
```

This is a deterministic correctness test, not a timing benchmark. It uses the
checked-in medium fixture and fails when representative module-field or
typed-local completion items move out of their required order.

Use it whenever candidate sources, scoring, prefix matching, usage statistics,
or tie-breakers change.

## Profile Representative Scale

The representative harness generates deterministic fixtures:

- `single-1000`: one 1,000-line source;
- `single-5000`: one 5,000-line source;
- `project-100`: a 100-file project;
- `project-1000`: a 1,000-file project.

Generate them without profiling:

```bash
make generate-lsp-benchmarks
```

Run the full release-build profile:

```bash
make profile-representative
```

The default report is
`target/tua-speed/representative-profile.json`.

### Cold, Warm, And Contended

| Lane | What happens | What it reveals |
| --- | --- | --- |
| Cold | A fresh server handles one request. | Startup, discovery, initial parsing, and first-query cost. |
| Warm | One server repeats completion, hover, definition, and whole-document inlay hints. | Cache reuse and steady-state latency. |
| Contended | Background inlay requests start, the document changes, then completion is requested immediately. | Cancellation, revision freshness, stale-result rejection, and interactive scheduling. |

Only `main.tua` is opened. Other project files must be discovered through the
normal project model, so the profile exercises realistic indexing rather than
pre-opening every provider.

The harness fails when:

- warm or contended completion p95 exceeds the configured budget;
- any stale result is observed after a document change;
- a semantic result validation fails.

Cancellation rate is informational. A low rate can mean background requests
finished before cancellation was necessary; it is not itself a failure.

### Smoke Versus Full Profile

`make representative-benchmark-smoke` uses only `single-1000` and
`project-100` with reduced repeats and a 3-second completion p95 budget. The
extra shared-runner allowance keeps this a CI reliability gate designed to
catch stalls, stale results, and invalid responses without treating its
two-sample p95 as a microbenchmark.

`make profile-representative` runs every fixture with more samples and a broad
10-second default budget. Use its report for scale comparisons and diagnosis,
not as proof that a 9-second completion is acceptable. The performance-trend
workflow raises only this full-profile ceiling to 30 seconds because shared
GitHub runners can take roughly four times longer on the generated 5,000-line
fixture. That runner allowance is not a local target or baseline.

## Useful Environment Variables

### Completion Profile

| Variable | Default | Purpose |
| --- | --- | --- |
| `TUA_PROFILE_REPEAT` | `20` | Measured request repetitions. |
| `TUA_PROFILE_WARMUP` | `2` | Unmeasured warmup repetitions. |
| `TUA_COMPLETION_PROFILE_OUT` | `target/tua-speed/completion-profile.json` | Current report path. |
| `TUA_COMPLETION_BASELINE` | unset | Prior report to compare against. |
| `TUA_COMPLETION_BASELINE_TOLERANCE` | `0.05` | Allowed p95 regression ratio. |
| `TUA_COMPLETION_P95_TARGET_MS` | `50` | Printed medium completion target. |
| `TUA_COMPLETION_RANKING_P95_TARGET_MS` | `5` | Printed ranking target. |
| `TUA_PROFILE` | `1` in this harness | Enables internal profile spans. |
| `TUA_PROFILE_STREAM` | unset | Streams raw profile spans to stderr. |

### Representative Profile

| Variable | Purpose |
| --- | --- |
| `TUA_REPRESENTATIVE_FIXTURES` | Comma-separated generated fixture names. |
| `TUA_REPRESENTATIVE_COLD_REPEAT` | Fresh-server samples per request. |
| `TUA_REPRESENTATIVE_WARM_REPEAT` | Repeated steady-state samples. |
| `TUA_REPRESENTATIVE_CONTENTION_REPEAT` | Contention rounds. |
| `TUA_REPRESENTATIVE_BACKGROUND_BURST` | Background requests started before each edit. |
| `TUA_REPRESENTATIVE_CONTENTION_DELAY_MS` | Delay before changing the document during contention. |
| `TUA_REPRESENTATIVE_COMPLETION_P95_BUDGET_MS` | Warm and contended completion failure budget; defaults to 10,000 ms, while the shared-runner trend workflow uses 30,000 ms. |
| `TUA_REPRESENTATIVE_PROFILE_OUT` | JSON report path. |
| `TUA_LSP_REQUEST_TIMEOUT_MS` | Per-request timeout. |

Profiling scripts set `TUA_COMPLETION_STATS=0` by default so benchmarks do not
create or mutate fixture-local `.tua/completion-stats.json` files. Set it to
`1` only when intentionally measuring persisted adaptive completion behavior.

## Diagnose A Regression

| Symptom | Likely area | Next check |
| --- | --- | --- |
| Total completion p95 rises, ranking stays flat | Snapshot, semantic query, module graph, or LSP transport | Rerun with `TUA_PROFILE_STREAM=1` and inspect `analysis.query.completion`, `analysis.semantic.*`, and `lsp.completion.*`. |
| Ranking p95 rises | Candidate scoring, usage index, adaptive stats, or sorting | Compare candidate counts and ranking spans before changing thresholds. |
| Repeated contexts keep missing cache | Request key, source fingerprint, revision scope, or prefix reuse | Inspect `completionContext` and `usageIndex` hits/misses across identical requests. |
| Known local or LOVE `.` completion takes seconds | Current-file lane fell back to project snapshot construction | Run `make editor-regression-smoke` and inspect current-file completion routing. |
| Stale result count is non-zero | Cancellation or revision validation | Reproduce with the representative contention profile. Results are stale only when an old request succeeds after the validated new-version completion barrier; do not mask them with a larger latency budget. |
| Semantic validation fails | Result correctness, not performance | Inspect the failing fixture and request before comparing timing. |
| Only p99 moves on a short run | Sample noise or one stall | Increase repeats and rerun on the same machine. |

## CI And Reports

Pull requests run `make benchmark-smoke`. CI retains the example and
representative smoke reports for 14 days.

The performance workflow runs `make profile-all` on `main`, weekly, and on
demand. It uploads JSON reports under `target/tua-speed/` for 90 days. Use those
artifacts for trend investigation; use local same-machine baselines for strict
before/after comparisons.
