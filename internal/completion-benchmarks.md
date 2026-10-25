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
| Would local inline completion help? | `make inline-eval` | Offline precision, characters saved, latency, parse cost, and warm-up hit rates for candidate inline sources on the tuning projects. |
| Does real VS Code expose the menu/provider state we modeled? | `make inline-editor-replay` | macOS-only, network-isolated CDP replay of exact tuning edits through the real widget and inline provider, plus provider-timeout cancellation evidence. |
| Do all benchmark lanes still work? | `make benchmark-smoke` | Ranking correctness, bundled-example timing, representative completion freshness, and the inline-evaluation smoke fixtures. |
| Do I need every performance report? | `make profile-all` | General LSP, completion, examples, and representative-scale profiles. |

For a completion implementation change, start with:

```bash
make completion-ranking-smoke
make profile-completion
```

Run `make representative-benchmark-smoke` when the change affects snapshots,
caching, scheduling, project indexing, cancellation, or watched files.

Experimental local inline completion is evaluated offline before its default
or source scope expands. The datasets, the held-out and external gates, and
every metric definition are described in
[`benchmarks/inline-eval/README.md`](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/inline-eval/README.md).

The implemented Phase 1 path is still opt-in. It contains source-only statement
snippets and conservative current-file recall, not the later project corpus or
statistical predictor. Its implementation landing does not mean the held-out,
latency, diagnostics-freshness, real-menu-replay, or parse-count gates have
passed. Run those gates only with the frozen backend configuration described by
the evaluator; do not tune after inspecting held-out results.

### Capture Real VS Code Menu State

The evaluator's default menu model is only an approximation of VS Code's fuzzy
selection and sort order. A developer can explicitly capture the menu state
that VS Code exposes to the inline provider:

```bash
TUA_INLINE_REPLAY_ROOT=/absolute/path/to/dataset/root \
TUA_INLINE_REPLAY_CAPTURE=/absolute/path/to/menu-replay.jsonl \
code --new-window /absolute/path/to/dataset/root
```

Set both variables before starting the VS Code process that hosts the Tua
extension. Capture has no setting or background telemetry path and is entirely
inactive when either variable is absent. It accepts only file documents whose
canonical path is below `TUA_INLINE_REPLAY_ROOT` and appends schema-versioned
JSONL to `TUA_INLINE_REPLAY_CAPTURE`. Also enable
`tua.completion.inline.enabled`; capture observes the same debounced provider
requests as the Phase 1 feature and does not run when that feature is off.

The capture does not store document text, an absolute URI, a wall-clock
timestamp, or inline-completion results. Each event contains the
manifest-root-relative path, SHA-256 source hash, zero-based UTF-16 position,
document version, elapsed time since an observed edit, whether the menu was
open, and the selected completion text/range when present. CRLF is normalized
to LF before hashing so Windows editor documents match the evaluator's
canonical scenarios. The selected text is necessary to reproduce VS Code's
extension rule. The file stays local and Tua never uploads it.

The relative path still reveals part of the dataset layout, and selected
completion text can retain source-derived identifiers or literals. Treat the
JSONL as potentially sensitive local evaluation data: keep it out of version
control and delete it when it is no longer needed.

Replay a tuning capture with:

```bash
cargo run -p tua-core --features test-fixture --example inline_eval -- \
  --manifest benchmarks/inline-eval/manifest.toml --set tune \
  --menu-replay /absolute/path/to/menu-replay.jsonl \
  --out target/tua-speed/inline-eval-menu-replay.json
```

For a deterministic real-editor capture, run `make inline-editor-replay` on
macOS. It opens isolated Code profiles, performs pinned edits in `.tua`
tuning fixtures with the real suggestion widget both open and closed,
triggers the actual Tua inline provider, and requires every capture to match an
exact evaluator scenario. A delayed mock-server session must also observe the
provider request followed by its linked `$/cancelRequest` near the configured
250 ms timeout, measured by the mock server's monotonic high-resolution clock.
For every replay, the harness snapshots the extension's deterministic returned
counters around the explicit request and requires one or more expected-source
increments with no competing local source. VS Code may invoke the provider more
than once for one editor action. Its own suggestion preview can render the same
suffix before Tua answers; that overlap is recorded rather than misreported as provider
rendering. Closed-menu cases with no overlap additionally require the exact
ghost text in the editor DOM.
Code runs under `sandbox-exec`; the extension host itself must
prove loopback access and denial of direct non-loopback outbound access or the
run fails. Results stay under `target/tua-speed/` and are not release evidence
until reviewed and recorded in the Phase 1 gate manifest.
The extension defaults `editor.suggest.preview` to `true` for Tua. A unit test
guards that product default, while the isolated replay workspace pins the same
value for reproducible open-menu evidence. It
does not force `.lua` documents into Tua mode: the production provider selector
is Tua only, while Lua remains part of the separate offline evaluator.
The final report identifies the Code build, platform and architecture, Git
revision and dirty state, and hashes its capture and evaluator inputs. All
outputs are invalidated before prerequisite checks and atomically written mode
`0600`, so a failed rerun cannot leave old passing evidence behind.

For a useful typing replay, work in a tuning fixture without saving changes:
delete the rest of one code line from a sampled cursor, then type that text
back. This produces the evaluator's `edit` scenario; editor-inserted closing
delimiters can produce its `auto-close` scenario. Arbitrary edits are safe to
capture but normally remain unmatched because the evaluator has no known truth
for them.

The evaluator loads the declared dataset and regenerates each known-truth
scenario. A record is applied only when its edited-document hash, UTF-16 cursor
position, and selection range match that exact scenario. Malformed,
outside-dataset, out-of-bounds, and ineligible events are rejected; valid
events that match no scenario are counted as unmatched instead of silently
being treated as menu evidence. Use a fresh output file for a new session
because capture is append-only. A capture is measurement input, not a new
tuning set: do not use held-out or external projects to adjust thresholds.

VS Code does not expose a stable public command for executing an inline
completion provider and inspecting its returned items. The extension-host test
therefore verifies that the real provider sends a version-bound local request;
unit tests cover item and snippet conversion, selected-menu ranges, fallback,
cancellation, and stale responses; and the real-server Rust tests cover the
request/response contract and returned recall text. Automating
`editor.action.inlineSuggest.trigger` followed by a timed UI commit would test
editor focus and animation timing more than the protocol, so it is intentionally
kept out of the release gate.

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
short runs. Set `TUA_COMPLETION_BASELINE_SLACK_MS` to change that slack. The
result is written to the report as `checks.completionBaseline`.

`make profile-completion-phase1-gate` is the Phase 1 version: it requires
`TUA_COMPLETION_BASELINE` (a pre-Phase-1 profile from the same machine), uses
100 repeats and no absolute slack, so 5% means exactly 5%, and writes
`target/tua-speed/completion-phase1-gate.json`. Record the baseline with the
same repeat count.

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
- `project-1000`: a 1,000-file project;
- `graph-chain-256`: a 256-edge runtime dependency chain;
- `graph-diamond-256`: a 16-by-16 diamond with shared-leaf fan-in;
- `graph-cycles-64`: a 32-node runtime cycle paired with mutually recursive
  type-only modules.

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
| Contended | Background inlay requests start, the document changes, then menu completion, deterministic inline completion, and diagnostics are observed. | Cancellation, revision freshness, stale-result rejection, interactive scheduling, inline queue/server cost, and diagnostics freshness. |

Only `main.tua` is opened. Other project files must be discovered through the
normal project model, so the profile exercises realistic indexing rather than
pre-opening every provider.

The harness fails when:

- warm or contended completion p95 exceeds the configured budget;
- successful inline request age, scheduler-dispatch-to-response p95,
  worker-execution p95, or core/backend p95 exceeds its configured budget;
- the first diagnostics publication after an edit exceeds its configured
  budget;
- an inline request fails, its queue/server/core spans do not cover every
  response, or its parse/cache counters are internally inconsistent;
- any stale result is observed after a document change;
- a semantic result validation fails.

Cancellation rate is informational. A low rate can mean background requests
finished before cancellation was necessary; it is not itself a failure.

### Representative Report Schema

Representative reports use schema version 2. The latency fields deliberately
measure different boundaries:

| Field | Boundary |
| --- | --- |
| `inlineCompletion.profile.coreBackend` | Time inside `AnalysisHost::local_inline_completion`: candidate generation and validation only. This is closest to the offline evaluator's backend latency. |
| `inlineCompletion.profile.workerExecution` | The outer `lsp.request` span on the worker: request validation, protocol conversion, the core backend, and response construction. |
| `inlineCompletion.profile.queue` | Time between scheduler dispatch and a worker receiving the request. |
| `inlineCompletion.serverRequest` | Scheduler-dispatch-to-response method latency, measured after the coordinator selects the request. It covers speculative scheduling and result adoption but excludes time already spent waiting in the coordinator's input queue. |
| `requestAge.inlineAfterChange` | Client-observed age from sending a contended request after an edit through receiving a successful response. It includes queueing, server work, and protocol transport. `inlineAllAfterChange` also includes error responses. |
| `diagnostics.firstPublishAfterChange` | Time from `didChange` to a diagnostics publication containing that revision's distinctive probe. Older queued publications cannot satisfy the sample. |

`inlineCompletion.profile` also reports the semantic, recall, and validation
subspans. `inlineCompletion.counters` is the server's cache-stat snapshot,
including requests, exact-source cache peeks, candidate attempts, zero/one/two
parse distribution, returned-after-zero/one/two-parse distribution,
edited-parse sufficiency, and returned candidates. The
profile fails closed when server/request samples and queue, worker, or core
spans do not cover every inline response, when no request reaches the core, or
when the parse distribution does not equal the candidate-attempt count.

This representative profile is one part of Phase 1 acceptance, not a substitute
for the offline quality gates. Overall acceptance also requires the committed
held-out and external precision/utility thresholds, the menu-completion baseline
comparison, real menu-state replay, diagnostics freshness, no semantic-analysis
or snapshot build on the inline path, and an acceptable validation parse
distribution. Passing `make inline-eval-holdout` or
`make inline-eval-external` alone therefore does not pass Phase 1.

The offline production backend intentionally has a cold analysis cache. Its
parse distribution is useful for diagnosing worst-case validation cost, but
it is report-only and does not affect held-out or external quality gates. The aggregate
validation prerequisite uses the representative LSP counters instead. Its
temporarily-invalid edit exercises the baseline comparison and two-parse path;
no candidate may return without parsing, and at least half of returned
candidates must use at most one parse. Zero-parse cancelled or expired attempts
are safe abstentions and remain visible in the report.

`make representative-phase1-gate` runs the full representative fixture matrix
with independent strict 10 ms limits for dispatch-to-response server/request
latency, worker execution, and core/backend work, plus a 1,500 ms p95 ceiling
for client-observed contended request age, and a separate 250 ms delivery
budget for that request age (`TUA_REPRESENTATIVE_INLINE_DELIVERY_P95_BUDGET_MS`),
and writes `target/tua-speed/representative-phase1-gate.json`. The report
groups its failures into named `checks` (`inlineServerPerformance`,
`inlineClientNoStall`, `inlineContendedDelivery`, `inlineCachePeekObserved`,
`validationParseDistribution`, and `general`), each with its own pass state.
The delivery check fails when a result typically arrives after the extension
has already given up on it. It leaves the broad
completion and diagnostics stall ceilings in place. The
1,500 ms request-age bound is a contention stall ceiling, not a claim that
one-second ghost text is acceptable interactive UX. Independently, the VS Code
provider cancels and abstains when its local backend has not answered within
250 ms after the 65 ms debounce; that client responsiveness protection is not
replaced by the synthetic no-stall ceiling. The
first contention round intentionally leaves the document temporarily invalid
to exercise baseline-plus-candidate parsing. Later rounds use valid documents;
only those valid rounds gate semantic menu-completion correctness. The invalid
round's menu result is recorded separately because incomplete syntax can
legitimately suppress semantic candidates.
The aggregate `make inline-eval-phase-gate` target depends on that representative
run, then validates every prerequisite and its evidence in
`benchmarks/inline-eval/phase1-gate.toml`; missing, pending, or evidence-free
items still fail. Every report records the commit and dirty state it was
produced from in a top-level `repository` field, and a passed prerequisite
names its `artifact`, `revision`, and the `check` in that report that decides
it, so one report's unrelated passing checks never stand in for a failing one.
The check rejects evidence from a dirty
tree, from another commit, or from a commit after which shipped code, the
measuring harness, or benchmark inputs changed. Server/request latency starts at scheduler dispatch; the
separate client request-age metric includes coordinator input waiting and
transport, so a fast backend alone cannot pass both gates. Use
`make inline-eval-phase-gate-check` for the cheap
fail-closed status check without refreshing the representative measurements.

Compare first-diagnostics-after-edit latency against a pre-Phase-1 binary on
the same machine with:

```bash
TUA_REPRESENTATIVE_BASELINE_TUA=/tmp/tua-phase1-baseline/target/release/tua \
  make representative-diagnostics-baseline
```

Build that binary from commit `65cb4a6` in a temporary clone or worktree, not
by changing the active checkout. The comparison alternates baseline and
candidate runs, uses the same generated fixtures and content-bound diagnostic
probe for both, and compares pooled p95 values from 100 samples per fixture
(five runs of 20 edits). Internal span profiling is disabled for both binaries
so newer inline-only instrumentation cannot skew the candidate. The old
binary runs in compatibility mode, which omits the request and telemetry that
did not exist before Phase 1. The candidate still enables and issues the local
inline request, so its measurement includes the additional speculative load.
The default per-fixture allowance is the larger of 5% or 10 ms; override the
run count, ratio, or minimum slack with
`TUA_REPRESENTATIVE_DIAGNOSTICS_ROUNDS`,
`TUA_REPRESENTATIVE_DIAGNOSTICS_SAMPLES_PER_ROUND`,
`TUA_REPRESENTATIVE_DIAGNOSTICS_TOLERANCE`, and
`TUA_REPRESENTATIVE_DIAGNOSTICS_MIN_SLACK_MS`. Each run must retain at least
20 edits and the complete comparison must retain at least 100 samples per
fixture. Reports stay under
`target/tua-speed/` and are machine-specific evidence, not committed data.

### Smoke Versus Full Profile

`make representative-benchmark-smoke` deliberately uses only `single-1000` and
`project-100` with reduced repeats and a 3-second completion p95 budget. The
extra shared-runner allowance keeps this a CI reliability gate designed to
catch stalls, stale results, and invalid responses without treating its
two-sample p95 as a microbenchmark.

`make profile-representative` runs every fixture with more samples and a broad
10-second default budget, including the chain, diamond/fan-in, runtime-cycle,
and recursive-type graph shapes. Use its report for scale comparisons and
diagnosis, not as proof that a 9-second completion is acceptable. The
performance-trend workflow raises only this full-profile ceiling to 30 seconds
because shared GitHub runners can take roughly four times longer on the
generated 5,000-line fixture. That runner allowance is not a local target or
baseline.

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
| `TUA_REPRESENTATIVE_INLINE_SERVER_P95_BUDGET_MS` | Scheduler-dispatch-to-response inline p95 budget; defaults to 10,000 ms. It does not include time waiting for coordinator selection. |
| `TUA_REPRESENTATIVE_INLINE_WORKER_P95_BUDGET_MS` | Inline worker-execution p95 budget for the outer `lsp.request` span; defaults to 20 ms. |
| `TUA_REPRESENTATIVE_INLINE_BACKEND_P95_BUDGET_MS` | Inline core/backend p95 budget for `inline.request.core`; defaults to 20 ms. |
| `TUA_REPRESENTATIVE_INLINE_E2E_P95_BUDGET_MS` | Successful contended inline request-age p95 budget, including queue and transport; defaults to 10,000 ms. This broad default catches stalls and is not a UX target. |
| `TUA_REPRESENTATIVE_INLINE_STATISTICAL` | Set to `1` to enable the default-off project statistical source during the representative run and measure its background indexing and contended request cost. |
| `TUA_REPRESENTATIVE_DIAGNOSTICS_P95_BUDGET_MS` | First diagnostics publication after an edit p95 budget; defaults to 10,000 ms. |
| `TUA_REPRESENTATIVE_PROFILE_OUT` | JSON report path. |
| `TUA_LSP_REQUEST_TIMEOUT_MS` | Per-request timeout. |
| `TUA_REPRESENTATIVE_WARM_TIMEOUT_MS` | Maximum wait for every discovered file and the project snapshot to finish warming; defaults to 120 seconds. |

Warm timings begin only after `workspaceWarm` reports that every discovered
file has either finished or failed and the project snapshot is ready. An
incomplete or failed warmup fails the profile instead of silently measuring a
partially indexed workspace.

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

Trusted pushes to `main` and release validation run `make benchmark-smoke`.
CI retains the example and representative smoke reports for 14 days. Open or
untrusted pull requests do not execute repository code in GitHub Actions.

The performance workflow runs `make profile-all` on `main`, weekly, and on
demand. It uploads JSON reports under `target/tua-speed/` for 90 days. Use those
artifacts for trend investigation; use local same-machine baselines for strict
before/after comparisons.

## Trusted performance gates

The representative CI smoke collects 20 warm and contention samples per
fixture, with separate 250 ms warm and 1500 ms contended completion p95 budgets
and a 1024 MiB sampled server RSS ceiling. Missing memory measurements fail the
gate; profiling requires `ps` (Linux/macOS). The trusted-push, weekly and manual
workflow includes 5,000-line and 1,000-file fixtures with 500/3000 ms latency and
4096 MiB RSS ceilings. Reports retain measurements for regression comparison;
these ceilings are qualification limits, not a claim of typical latency.
