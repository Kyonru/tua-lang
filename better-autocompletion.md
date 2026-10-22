# Local Pattern-Based Inline Completion for Tua

## Context

Tua's menu completion already covers scope, types, LÖVE APIs, paths, snippets,
and adaptive ranking. The implemented first phase adds opt-in deterministic
ghost text from source-only statement snippets and current-file recall; Ollama
remains the optional model fallback. `multitoken.rs` still contributes a small
number of hardcoded multi-token menu items.

The goal is deterministic inline completion that uses existing semantic
knowledge and patterns from the user's code, with no AI service required.

The implementation expands only when evaluation supports it:

```text
semantic reuse
    -> current-file recall
    -> immutable project retrieval
    -> n-gram experiment
    -> multiline completion
```

Personal learning across projects remains a separate, later proposal.

## Goal

Provide deterministic inline suggestions from:

1. Existing semantic snippets
2. Repeated patterns in the current file
3. Repeated patterns in the current project
4. A small bundled Lua/LÖVE corpus
5. Ollama, only when explicitly enabled and all local sources return nothing

The Phase 0 evaluation decides the source order, which can differ by context.
The first release prioritizes precision and responsiveness, not general code
generation.

## Non-goals

- Building semantic analysis or a project snapshot for every inline request
- Full type-checking of suggestions
- Any observable behavior change to `textDocument/completion`. Shared
  primitives may be extracted and tests may be added, but existing completion
  results, ordering, and performance gates must stay unchanged.
- Persisting the project retrieval index
- Reading VS Code user snippets inside `tua-core`
- Replacing `multitoken.rs` before measured parity
- Cross-project personal learning in the initial implementation

## Architectural rules

### Ownership

- `tua-core` owns candidate generation, normalization, retrieval, ranking, and
  validation.
- `tua-lsp` owns scheduling, configuration, protocol conversion, source
  discovery and eligibility, and adoption of background jobs.
- The extension owns the VS Code provider, user snippets, presentation,
  workspace-trust state, and Ollama fallback.

### Performance

- An inline query never builds semantic analysis, project indexes, or an
  `AnalysisSnapshot`.
- Phase 1 does not consult an `AnalysisSnapshot`, even opportunistically.
  Project-wide facts arrive only through the immutable, version-checked corpus
  introduced in Phase 2, so results never depend on whether a **project**
  snapshot happens to be cached.
- Phase 1 candidate generation is source-only, so statement snippets and
  current-file recall do not depend on cache warm-up timing. Parse-only
  validation may reuse an exactly matching cached parse as its error baseline;
  a miss never causes semantic analysis or a snapshot build. Any future
  candidate that needs parsed facts must be justified by measured cache-hit
  behavior and remain safe when the lookup misses.
- It may perform a bounded source-kind parse for validation. This builds the
  complete syntax model but not the checker, linter, type environment, or
  project indexes. The common case is a single parse.
- Cached semantic facts are used only when they already match the current VFS
  source.
- No request-time disk or database reads.
- A cancelled or expired request returns nothing.
- Menu completion and diagnostics retain priority over speculative inline work.

### Consistency

- No mutable pattern index is shared through `AnalysisSnapshot`.
- Project corpora are immutable values assembled from per-file tables, each
  tagged with the VFS version of the file it came from.
- Before a base hit is returned, its VFS version is compared with the file's
  current version. This is an integer comparison, never a hash of the source.
- Existing semantic snippets and LÖVE catalogs are shared rather than
  duplicated.

---

## Phase 0: Measurement and evaluation

### Performance baselines

Record baselines on a clean `main`:

- Menu completion p50/p95 using `make profile-completion`
- Representative contention using `make profile-representative`, including
  inline queue, worker-execution, scheduler-dispatch-to-response server/request,
  core-backend, client-observed request-age, parse-counter, and
  first-diagnostics-after-edit measurements
- Diagnostics freshness using `make editor-regression-smoke`
- A same-machine pre-Phase-1 diagnostics comparison using
  `make representative-diagnostics-baseline`. Build commit `65cb4a6` in a
  temporary checkout and pass its binary through
  `TUA_REPRESENTATIVE_BASELINE_TUA`. The harness alternates five runs, checks
  identical fixture hashes, and compares pooled first-publication p95 values
  across 100 samples per fixture. Internal profiling is disabled symmetrically;
  the compatibility baseline omits unsupported inline requests while the
  candidate retains the enabled inline request.
- Large-source behavior using `make profile-large-tables`
- Complete syntax-parse throughput and p95 latency for each source kind
  (`parse_tua_source`, `parse_lua_source`)
- Memory before and after representative project loading

Save local reports under `target/tua-speed/`. Do not commit machine-specific
results.

Base the decision to attempt a parse on both:

- A conservative maximum file size
- Measured p95 throughput with a safety margin

Do not rely solely on average bytes per millisecond.

### Eligibility

Inline completion is eligible only when all of these hold:

- The cursor is outside comments and all string forms.
- The line prefix is non-empty, except for later statement-start experiments.
- Text after the cursor is empty or whitespace, or consists of closing
  delimiters that agree with the candidate.
- The candidate does not duplicate existing suffix text.

When the completion menu is closed, normal inline candidates are allowed in
contexts such as these:

- `(`
- `,`
- `=`
- An operator
- Whitespace
- A statement prefix

When `selectedCompletionInfo` is present, only return a candidate that extends
that selected item. Otherwise return nothing. VS Code requires the returned
item's range to start where the selected item's range starts, and its text to
begin with the selected text. The server must therefore return a range anchored
at `selectedCompletion.range.start`, not at the cursor.

Measure the menu-open and menu-closed cases separately. Don't assume either
is the common one until the extension replay results exist.

### Evaluation implementation

Add an evaluator behind the existing `test-fixture` feature:

```text
tua_core::inline_completion::eval
crates/tua-core/examples/inline_eval.rs
```

Invocation:

```sh
cargo run -p tua-core \
  --features test-fixture \
  --example inline_eval -- <paths> --json
```

#### Evaluation targets

- `make inline-eval`
  - Uses the Tua tuning projects and the Lua tuning data while developing and
    tuning thresholds.
- `make inline-eval-smoke`
  - Uses small, committed, deterministic fixtures.
  - Runs under `benchmark-smoke` and CI.
  - Requires no network access.
- `make representative-phase1-gate`
  - Runs the full representative fixture matrix with independent strict 10 ms
    budgets for scheduler-dispatch-to-response latency, worker execution,
    and core/backend work, plus a 1,500 ms p95 ceiling for client-observed
    contended request age.
  - Writes `target/tua-speed/representative-phase1-gate.json`.
- `make inline-eval-phase-gate`
  - Runs `representative-phase1-gate`, then checks the fail-closed aggregate
    artifact covering every Phase 1 prerequisite.
- `make inline-eval-phase-gate-check`
  - Checks the recorded aggregate artifact without rerunning the expensive
    representative fixture matrix.
- `make inline-eval-external`
  - Downloads pinned third-party projects into `target/`.
  - Verifies commit IDs and checksums.
  - Records project URLs and licenses in a manifest.
  - Runs manually or in a dedicated network-enabled evaluation job.

The external dataset and the held-out Tua sets are the quality gates for phase
approval. Ordinary CI never downloads anything.

Passing those dataset gates is necessary but not sufficient for Phase 1
acceptance. `benchmarks/inline-eval/phase1-gate.toml` is the fail-closed
aggregate decision: held-out and external quality, representative menu replay,
completion performance, internal inline performance, client no-stall and
responsiveness evidence, diagnostics freshness, real-scheduler cache behavior,
and the validation parse distribution must each be passed with evidence. Run
`make inline-eval-phase-gate`; a missing, pending, or evidence-free prerequisite
fails.

The evaluator's production backend deliberately starts cold, so its
zero/one/two-parse distribution is a report-only diagnostic worst case; it is
not part of held-out or external quality acceptance. The aggregate validation
prerequisite instead uses the representative LSP run to measure the production
request under real scheduling. It may be
marked passed only when counters cover every attempt and returned candidate,
no candidate returns without a successful parse, a temporarily-invalid edit
exercises the two-parse baseline-comparison path, and at least half of returned
candidates use at most one parse. A zero-parse cancellation or deadline
abstention is safe and is reported rather than treated as a returned
completion.

### Scenarios

Evaluate:

- Current file revealed only up to the cut point
- Active line suffix withheld from the index
- Target file held out from project retrieval
- Target line held out while other lines remain available
- Edits earlier in an existing file
- Existing text after the cursor
- Temporarily invalid documents
- Cold, partial, and complete indexes
- Menu open and closed
- Tua and Lua sources separately
- In Phase 2, a callback already declared in another project file
  (duplicate suppression)

Remove duplicate files before splitting the dataset.

### Datasets

No suitable independent Tua corpus has been identified yet, so the initial Tua
sets must be built deliberately:

- **Split `examples/` by project, never by file.** For example:
  - Tune on `love-full`, `features`, and `project-model`.
  - Hold out `love-tua` and `audio-system`.

  Record the split in the dataset manifest before tuning starts.
- **Add frozen Tua holdout fixtures:** a small set of Tua files written and
  committed before any threshold tuning, and never used for tuning. These
  protect against tuning leakage but do not establish external validity.
- **Tua precision gate:** Tua precision is gated on the held-out Tua projects
  plus the frozen fixtures, independently of the Lua results. Treat this as a
  provisional product-quality gate until independent Tua projects are
  available.
- **Lua test set:** pinned third-party LÖVE projects form the external Lua/LÖVE
  test set.
- **Generated benchmarks** are for performance only.
- **Reporting:** report Tua and Lua results separately. Third-party Lua results
  must not hide weak Tua coverage.

Commit thresholds and acceptance criteria before looking at any held-out
results. Thresholds are scoped to dataset and language: the holdout gate names
Tua limits, while the external gate names Lua limits. A dataset must fail when
one of its configured languages is absent, but it must not require a language
that dataset was never intended to contain. The committed Phase 1 table also
pins semantic-source switches and the minimum snippet prefix, not only recall
settings.

### Metrics

Report:

- Suggestions per eligible position
- Exact full-suggestion precision
- First-token precision
- First-three-token precision
- Characters saved per eligible position, excluding whitespace-only literals
- Incorrect characters offered per eligible position
- Precision/coverage curves
- p50/p95/p99 backend-only latency (candidate generation/validation, excluding
  evaluator setup, menu filtering, and scoring)
- Index build time
- Memory in bytes
- Parses per request (target: one in the common case)
- How often the exact-current-source cache peek hits at request time, during replayed
  continuous typing and after pauses of 150 ms, 300 ms, and 1 s. This decides
  whether candidates that need parsed facts are worth shipping in Phase 1.
- Results by source
- Results by language
- Results by menu state
- Results by cold/partial/warm index state

The harness must compare:

1. No-op backend
2. Semantic-only backend
3. Current-file recall
4. Later project and statistical backends

---

## Phase 1: Minimal vertical slice

> **Implementation status (2026-10-21): implemented, experimental, gate
> pending only on the frozen quality gates.** The core host query, source-only statement snippets,
> tuning-selected common-prefix current-file recall, parse-only validation,
> custom LSP request, newest-wins scheduling, configuration, unified VS Code
> provider, and focused tests are present in the working implementation.
> Following the Phase 0 evidence, LÖVE callback ghost text was not included;
> snippets require three typed characters, assignment and member contexts are
> excluded from recall, a one-line match is limited to its first token, and a
> full common prefix requires two supporting lines. The feature remains off by
> default. Menu-completion performance, the strict inline server gate,
> real-scheduler cache behavior, and validation parse distribution pass. The
> held-out and external quality gates have not been run. The real menu-state
> replay and linked client-timeout cancellation checks pass. The same-machine,
> 100-sample-per-fixture diagnostics
> comparison against pre-Phase-1 commit `65cb4a6` passes all seven fixtures.
> Representative synthetic contention reaches 869.41 ms, but the separate
> real-editor replay proves the provider cancels local work at its 250 ms
> timeout and renders no late result.
> Phase 2 must not ship merely because the implementation exists.

The representative profiler now emits schema-version-2 queue, worker,
scheduler-dispatch-to-response server/request, core, end-to-end request-age,
inline-counter, and diagnostics-freshness measurements. That instrumentation
makes the remaining performance gates measurable. The strict seven-fixture
server/worker/backend and client no-stall gates pass; the aggregate Phase 1
gate remains pending only on the frozen held-out and external quality checks.

Implement only:

- Narrow semantic snippets
- Current-file, single-line recall

Existing deterministic semantic snippets may retain their authored multiline
body so the Phase 1 semantic source is useful. This is a narrow exception for
the shared snippet catalog, not general multiline generation: recalled or
learned candidates remain single-line, and Phase 1 does not expose a
`maxLines` setting. General multiline completion remains gated in Phase 4.

No project index, bundled corpus, n-grams, database, or general multiline
completion.

### Core API

Add:

```rust
AnalysisHost::local_inline_completion(
    path,
    position,
    options: &InlineCompletionOptions,
    budget: &InlineCompletionBudget,
    cancelled: &AtomicBool,
) -> Vec<InlineCompletionCandidate>
```

Use a small budget type instead of passing raw `Instant` values around. It
should allow deterministic testing and deadline checks between stages.

Candidate shape:

```rust
struct InlineCompletionCandidate {
    insert_text: String,
    range: SourceSpan,
    is_snippet: bool,
    source: InlineCompletionSource,
    score: InlineCompletionScore,
    pattern_id: Option<PatternId>,
}
```

Use "score," not "confidence," until calibration exists.

### Cached lookup that never builds

Use narrow internal lookups that never build:

```rust
AnalysisHost::current_analysis_cache_matches(path)
AnalysisHost::peek_current_parse_errors(path)
```

Its contract:

- Read the current VFS source.
- Look in `AnalysisQueryCache::files`.
- Report a hit, or clone only the cached parse-error vector, when path, kind,
  and source match. Do not clone the full `AnalysisFile` on the request path.
- Never call `cached_file`.
- Never parse, rebuild, or update cache statistics.
- Remain crate-private.

The inline request can always use source-only behavior. Behavior that needs
parsed facts runs only when a matching current-file cache entry exists. Phase
1 does not consume project-wide semantic facts. Phase 2 obtains project facts
only from its immutable corpus after version and eligibility checks; neither
phase adds an API that exposes a stale snapshot as current.

### Semantic candidates

Extract only narrow primitives shared by menu and inline completion.

#### Source-only statement snippets

Reuse `syntax_snippet_completions` without requiring cached semantic analysis.
Take `include_ecs` from the host's project config.

Offer one only when:

- The cursor is at a valid statement position.
- The typed prefix has a unique snippet match.
- The completion menu is not already presenting equivalent text.

#### LÖVE callbacks

Do not treat `missing_core_declarations` as the complete callback catalog. It
covers only `CORE_CALLBACKS` (`load`, `update`, `draw`).

Instead:

- Reuse the existing LÖVE callback catalog (`love_callback::items`).
- Reuse `declaration_suffix` to build the callback text.
- Extract a shared `declared_love_callbacks` query over the parsed declaration
  facts. It returns canonical callback IDs and is used by both menu and inline
  completion; do not infer declarations from workspace-symbol labels or fuzzy
  symbol-index matches.
- Suppress callbacks already declared in the active file using a narrow cached
  declaration-facts lookup (Phase 1 ultimately dropped this source).
- Require a unique prefix match after `function love.`.
- If the current-file lookup misses, return nothing, because even active-file
  duplicates can't be ruled out safely.

Phase 1 intentionally performs active-file suppression only. Reading
project-wide declarations from an opportunistically available snapshot would
make identical source produce different results depending on whether a project
snapshot happened to be cached.

Callback candidates need parsed facts, so they appear only when the
current-file lookup hits (see "Performance"). If the Phase 0 hit rate during
typing is too low for callback ghost text to add measurable value, drop
callbacks from Phase 1. Ship statement snippets and recall alone rather than a
feature that rarely appears.

Fully reliable project-wide suppression arrives in Phase 2, which adds a
versioned per-file callback-declaration index and excludes a callback declared
in any current project file. Per-file corpus builds populate that index through
the same `declared_love_callbacks` query, not through a second detection path.

`apply_love_callback_snippets` continues to decorate menu candidates using the
same shared callback-building primitive.

Member and field completion remain out of Phase 1.

### Current-file recall

Match the normalized prefix of the current line against other lines in the
same source.

Rules:

- Exclude the active line.
- Require a meaningful prefix, initially around six non-whitespace characters.
- Normalize indentation and insignificant spacing for lookup. Map the matched
  normalized prefix back to a byte offset in the original line before taking
  the continuation.
- Return the original source continuation, so quote style, spacing, and
  formatting are preserved.
- Stop at the end of the line.
- Prefer unique continuations.
- Suppress ambiguous matches unless one continuation clearly dominates.
- Tune the dominance ratio and minimum supporting-line count on the tuning
  set.
- Skip files exceeding the Phase 0 byte or line cap.

#### Caching

Start with a bounded scan. Current-file recall doesn't get its own cache until
profiling shows it needs one.

If needed, add a shared bounded memoization cache:

```rust
(path, vfs_version) -> Arc<FileRecallTable>
```

Requirements:

- Immutable table values
- Explicit byte cap
- Not stored inside `AnalysisQueryCache`, which is deep-cloned per job
- Not included in `AnalysisSnapshot`
- Shared across host clones, like `completion_cache`
- Version checked before reuse
- Safe eviction that never affects correctness

### Combining sources

Automatic requests return one item.

Candidates are selected in this order:

1. Context-specific source priority
2. Score within that source

Scores from different sources are not compared directly.

Starting hypothesis:

- Unique semantic callbacks and statement snippets win in their exact contexts.
- Current-file recall wins for expression and argument continuations.
- A real continuation after `love.graphics.setColor(` should beat a generic
  snippet with placeholders.

Phase 0 may change that order.

### Parse-only validation

Validation must not construct a snapshot or run type checking.

#### Stage 1: structural checks

Reject candidates that:

- Start or end inside a token
- Contain unresolved placeholders
- Produce incompatible bracket or delimiter structure
- Conflict with the existing line suffix
- Exceed one line, unless they are deterministic snippets from the existing
  semantic snippet catalog

#### Stage 2: deciding whether to parse

Before any parse:

- Check cancellation.
- Check remaining budget.
- Enforce the conservative byte cap.
- Apply the measured p95 parsing estimate with a safety margin.

If the parse can't safely fit, return nothing. A full_moon parse can't be
interrupted, so this check must happen before the parse starts.

#### Stage 3: parse the edited text first

Use the normal source-kind parse entry points: `parse_tua_source` for Tua and
`parse_lua_source` for Lua. These build the complete syntax model, including
Tua class, sprite, declaration-file, and syntax-model diagnostics, but do not
run the checker or build project analysis.

1. Parse the **edited** source.
2. If it has no errors, accept the candidate. No baseline is needed. This is
   the common case while typing, because the document is usually invalid until
   the current line is finished.
3. If it has errors, get the **baseline** (the errors before the edit):
   - Use the narrow cached parse-error vector when the exact-source peek hits.
   - Otherwise do one source-kind baseline parse, if Stage 2 still allows it.
4. Compare the two with the diagnostic multiset and position-mapping helpers
   from model completion, made `pub(crate)`.
5. Reject candidates that add or worsen an error attributable to the
   insertion.

The baseline and candidate therefore use the same complete syntax contract.
Neither includes checker, linter, or project-semantic diagnostics.

Do not call:

- `snapshot()`
- Full diagnostics
- The checker
- The linter
- Project analysis
- `validate_model_completion`

Try at most one shorter variant, cut at a balanced delimiter boundary. It goes
through the same parse decision and budget.

### LSP request

Add:

```text
tua/localInlineCompletion
```

Parameters:

```json
{
  "textDocument": { "uri": "..." },
  "position": { "line": 0, "character": 0 },
  "documentVersion": 1,
  "selectedCompletion": {
    "text": "...",
    "range": {}
  }
}
```

`selectedCompletion` is optional. When it is present in Phase 1, each returned
item uses `selectedCompletion.range` as its replacement range and its
`insertText` starts with `selectedCompletion.text`. A later suffix-replacement
feature may widen the range only after dedicated compatibility tests.

Response:

```json
{
  "documentVersion": 1,
  "items": [
    {
      "insertText": "...",
      "isSnippet": false,
      "range": {},
      "source": "currentFile",
      "score": 0,
      "patternId": null
    }
  ]
}
```

The custom request is used at first because:

- The extension coordinates local and Ollama sources in one provider.
- Internal source and pattern metadata are useful.
- It avoids tying the first release to language-client support for LSP 3.18.

Core types stay protocol-neutral, so standard `textDocument/inlineCompletion`
can be added later.

### Scheduling

Use one dedicated speculative worker. Representative contention measurements
showed that sharing the foreground workers kept core/backend work below 2 ms
but raised dispatch-to-response latency above the 10 ms Phase 1 gate.

- Give local inline completion lower coordinator priority than menu completion.
- Mark it non-interactive for capacity accounting.
- Implement newest-wins cancellation per URI.
- Keep at most one queued request per URI in the speculative lane; replace a
  queued superseded request instead of accumulating cancelled work.
- Prepare an owned source query on the coordinator without parsing or building
  analysis, and run all generation and validation on the speculative worker.
- Give core candidate generation and validation a 10 ms deadline. Check the
  remaining budget before each non-interruptible parse and abstain when there
  is not enough time. Queue, dispatch-to-response request, and client-observed
  end-to-end latency remain separate measured budgets. During the speculative
  worker's bounded priority window, only completed workspace-warm adoption is
  deferred; edits, cancellation, menu completion, and ordinary completed
  requests bypass it.
- Pass the scheduler's cancellation flag into the core query.
- Check cancellation between stages and inside long scans.
- Keep cancelling requests for documents that have been edited.

Newest-wins is distinct from the existing coalescing: a newer request cancels
the older job instead of joining it as another waiter.

### Extension

Refactor `ollama-completion.js` into one `inline-completion.js` controller.

The provider:

- Debounces approximately 50-75 ms.
- After the debounce, bounds the local backend request to 250 ms with a linked
  cancellation token. Timeout is a transient abstention and never starts the
  Ollama fallback.
- Cancels on edits and cursor movement.
- Rejects stale document versions.
- Sends the selected completion's text and range when present.
- Tries the local backend first.
- Calls Ollama only when local completion returns no candidate or the server
  does not support the local request, and model integration is enabled.
- Uses `SnippetString` for snippet candidates.
- Treats cancellation, `ContentModified`, and capacity errors as transient
  abstentions that do not start Ollama work.
- Deduplicates candidates equivalent to the selected menu item.
- Preserves current Ollama status and statistics behavior.

User-defined VS Code snippets remain extension-owned. They are not parsed or
persisted by core.

### Phase 1 settings

Expose only settings that Phase 1 implements:

```text
tua.completion.inline.enabled = false
tua.completion.inline.sources.semantic = true
tua.completion.inline.sources.currentFile = true
```

Do not expose `maxLines`, project, or bundled-corpus settings yet.

### Carried over from Phase 0

The Phase 0 evaluator (`benchmarks/inline-eval/`) shipped without these
measurements. Each one must exist before its gate is checked.

- **Menu-state replays in the extension.** Phase 0 simulates the suggestion
  menu in core: it counts the menu as open after a word or a `.`/`:` trigger
  when a menu item continues the typed word, and selects the first such item.
  Record real typing sessions in the extension instead (local only, opt-in,
  never uploaded), capturing when the menu is open and what it has selected at
  each eligible position. Replay them through the evaluator to confirm or
  correct the simulation, and report menu-open and menu-closed results from
  the replays. Required before the Phase 1 gate, because those results are
  reported separately.
  The developer-only local JSONL capture and evaluator ingestion are now
  implemented behind `TUA_INLINE_REPLAY_ROOT` plus
  `TUA_INLINE_REPLAY_CAPTURE`; `--menu-replay` applies a record only when its
  edited-document hash, cursor, and selection range match a generated scenario
  exactly, then validates core eligibility. Records retain the dataset-relative
  path and selected completion text/range when present; selected text can carry
  source-derived names, so capture files are potentially sensitive local
  evaluation data and must not be committed. Capture normalizes CRLF to the
  evaluator's canonical LF before hashing. Valid but unrelated edits are
  reported as unmatched. The committed clean-tree tuning replay described
  below satisfies this prerequisite; it does not substitute for the still
  unspent held-out and external quality checks.
  `make inline-editor-replay` now automates that evidence on macOS: it performs
  hash-pinned `.tua` tuning edits one character at a time through VS Code's
  editor API, uses CDP to observe the real suggestion widget in matched
  open/closed cases, triggers the actual provider,
  requires one or more expected deterministic returned-counter increments with
  no return from a competing local source, and replays one exact final capture
  per scenario through the evaluator. Transitional invocations remain counted
  separately. The counter proof keeps
  VS Code's own matching suggestion-preview text from being mistaken for Tua
  rendering; native preview overlap is retained in the report. It also verifies
  the provider's 250 ms timeout with a delayed mock server. Code runs in an isolated
  profile under `sandbox-exec`, and each extension-host process must actively
  prove loopback works while direct non-loopback outbound access is denied.
  The target fails on missing widget, capture, evaluator, cancellation, or
  network-isolation evidence. A clean-tree run on commit `95c9000` matched all
  four curated captures and cancelled delayed local work after 251 ms; the menu
  replay and client-responsiveness prerequisites are recorded as passing.
  Evidence is invalidated before prerequisites, written atomically with mode
  `0600`, bound by content hashes and a run ID, and tagged with the Code build,
  host platform/architecture, and repository revision.
  The extension now defaults `editor.suggest.preview` to `true` for Tua. A unit
  test asserts that product default, while the replay workspace pins it for the
  `selectedCompletionInfo` path. It does not relabel `.lua` documents as Tua:
  that would not represent the production provider's current Tua-only selector,
  and Lua stays covered by offline evaluation.
- **Parses per request.** There is nothing to count until parse-only
  validation exists. Once it does, the evaluator reports parses per request
  and how often the edited text alone was enough. The Phase 1 gate's "common-case
  validation needs one parse" is checked against this number.
- **Warm-up hit rate under real scheduling.** Phase 0 compares each warm-up's
  duration with the request delay. That's an upper bound: it assumes the
  warm-up starts at the keystroke and never waits behind other work. Measure
  exact-current-source cache-peek hits inside the language server, with real
  scheduling and the newest-wins cancellation, before relying on any
  candidate that needs parsed facts.
- **Cold, partial, and warm index states.** The Phase 2 evaluator now reports
  cold, partial, and complete project indexes separately. They remain tuning
  evidence only until a Phase 2 gate is frozen and run.
- **Phase 2 evaluator work.** The experimental backend now reports separate
  index-construction and near-duplicate clustering costs, removes the target
  file before clustering and partial-index selection, supports target-file and
  target-line holdouts, and clusters near-identical tutorial checkpoints with
  deterministic representatives. Callback suppression from another project
  file remains deferred with callback generation. The evaluator must still
  gate on per-project (macro) results as well as pooled ones when Phase 2's
  thresholds are frozen.
- **Completion-latency comparisons.** `make profile-completion` accepts a
  regression of the larger of 5% and 1 ms, which on a 5 ms baseline is about
  18%. A single run's p95 is also noisy. Check the Phase 1 gate's 5% tolerance
  by alternating the baseline and candidate binaries over several rounds on the
  same fixture and comparing medians.

### Findings from the Phase 0 tuning run

The Phase 0 evaluator's first runs on the tuning set suggest these Phase 1
design changes. Check them again on the tuning set before freezing
`benchmarks/inline-eval/thresholds.toml`.

- **Drop LÖVE callback suggestions from Phase 1.** A half-typed
  `function love.upd` line never parses and has no recovered parse, so the
  "cached facts only" rule never offers a callback. Most requests arrive while
  the document doesn't parse at all.
- **Require a few typed characters for statement snippets.** Snippets offered
  after one typed character are rarely right; after three, they usually are.
- **Prefer common-prefix recall.** Offering only what every matching line
  agrees on is right as far as it goes far more often than offering a unique
  full continuation, and it saves more typing.
- **Gate on correct-prefix precision and the wrong-to-saved ratio,** not exact
  match, and consider a context allowlist: recall after `=` and after `.`/`:`
  is much weaker than elsewhere.

The first pinned candidate (common-prefix recall, no callback source, a
three-character snippet minimum, and the initial context allowlist) improved
the tuning-set wrong-to-saved ratio from 3.53 to 2.17 and correct-prefix
precision from 0.28 to 0.53. It still misses the provisional 1.0 and 0.60
limits, so this is evidence to improve or narrow the backend—not permission to
weaken the gates or inspect held-out results.

Requiring two supporting lines makes long recall precise, but leaves the Tua
slice below its coverage floor. The current candidate keeps full common-prefix
continuations at support two or higher and limits a one-line match to its
first token. After the evaluator began sampling the configured whitespace
context and stopped crediting blank snippet literals, the corrected tuning set
reaches 0.728 correct-prefix precision overall, with 0.395 wrong characters per
character saved. Split by language, Lua reaches 0.771 precision, 0.261
wrong/saved, and 0.304 characters saved per eligible position; Tua reaches
0.695, 0.554, and 0.093 respectively. Broadening
assignment/member recall, allowing full one-line continuations, two-letter
snippet exceptions, and ambiguous snippet common-prefix fallback were all
tested only on tuning data and rejected. The candidate remains below the
provisional Tua usefulness floor, so Phase 1 is quality-blocked and no
held-out or external quality gate has been run.

### Phase 1 gate

Proceed only if:

- Menu completion stays within its existing 5% tolerance.
- Diagnostics freshness meets the representative profile's configured
  first-publication-after-edit budget.
- Offline core/backend p95 meets the committed evaluator threshold (10 ms in
  the frozen candidate), and representative inline core/backend,
  worker-execution, scheduler-dispatch-to-response server/request, and
  client-observed end-to-end p95 meet their independently configured budgets. The
  representative defaults are 20 ms each for worker and core/backend work and
  broad 10,000 ms stall ceilings for server/request and end-to-end request
  age; those broad ceilings are not UX targets. `make
  representative-phase1-gate` tightens the dispatch-to-response, worker, and
  core/backend limits to 10 ms and client-observed contended request age to
  a 1,500 ms stall ceiling before the aggregate phase-gate check runs. That
  request-age ceiling catches coordinator/transport stalls; it is not UX
  acceptance for a suggestion that arrives that late. The extension's separate
  250 ms local-request timeout protects interactive responsiveness after its
  65 ms debounce; it is not replaced by the synthetic 1,500 ms gate.
- Current-file recall adds measurable characters saved per eligible position.
- Held-out precision meets the committed thresholds, separately for:
  - the Tua held-out projects plus the frozen holdout fixtures
  - the external Lua set
- Source-only snippets continue to work when cached semantic analysis is
  absent.
- No inline query builds semantic analysis or a snapshot.
- Common-case validation needs one parse.
- The representative gate includes a temporarily-invalid edit: it must exercise
  a successful two-parse baseline comparison, no candidate may return after
  zero parses, and at least half of returned candidates must use at most one
  parse.
- Callback candidates ship only if the measured lookup hit rate during typing
  lets them add measurable characters saved. Otherwise Phase 1 ships without
  them.

This is an aggregate gate. The held-out evaluator checks quality and
backend-only latency; it cannot by itself pass the menu-completion,
representative server/end-to-end latency, diagnostics-freshness, real-menu
replay, cache-hit, or parse-distribution prerequisites above.

---

## Phase 2: Immutable project retrieval

Project-wide exact retrieval may be implemented and measured as experimental
gate-development when Phase 1 evidence shows that current-file-only recall is
the limiting source. It must remain disabled and must not consume the frozen
held-out sets or be described as an accepted phase until the aggregate Phase 1
gate passes. This exception lets the tuning set test whether project evidence
actually closes the known Tua coverage gap without weakening any release gate.

> **Evaluator status (2026-10-22): tuning only.** Exact retrieval now reports
> same-directory and rest-of-project evidence separately. The pinned
> `[phase2_tuning]` policy in `benchmarks/inline-eval/thresholds.toml` adds one
> explicitly named combined candidate. That candidate now runs the production
> `AnalysisHost::local_inline_completion` path against an adopted immutable
> corpus, including production source order, parse-delta validation, request
> budget, cancellation, and VFS-version rejection. Its conservative project
> fallback is limited to comma, operator, and `other` contexts, excludes
> same-directory and bundled evidence, and requires six non-whitespace prefix
> characters and two supporting matches before returning their full common
> prefix. This policy may be
> measured on tune and smoke only until it is frozen for a Phase 2 gate. The
> production path implements this policy behind a hidden, default-off server
> switch so scheduling, invalidation, and contention can be tested, but the
> extension does not expose it and its presence is not evidence that Phase 2
> passes.
> On the corrected tuning set it adds no suggestion over Phase 1: both save
> `0.149` characters per eligible position, have `0.728` correct-prefix
> precision, and offer `0.395` wrong characters per character saved. The
> architecture remains useful groundwork, but there is currently no tuning
> evidence that project retrieval improves completion quality. No held-out or
> external data has been inspected.

> **Implementation status (2026-10-21): integrated but not approved.** The VFS
> lineage/version mechanism, immutable base-plus-overlay corpus, background
> build/adopt/remerge lifecycle, bounded request query, versioned callback
> declaration index, and `tua/cacheStats`/`tua/health` counters are implemented.
> Callback generation remains deferred until it is evaluated, so the index is
> foundation for later duplicate suppression rather than a production source.
> The project source remains unreachable from the extension's contributed
> settings; only an explicit raw `sources.project` configuration can enable it
> for integration testing. The bundled corpus remains evaluator-only. Phase 1's
> aggregate gate is still incomplete, and neither the Phase 2 held-out nor
> external data has been inspected.

### Prerequisite: per-file VFS versions

The implementation now stores an opaque version with every effective
`VfsFileSource`, avoiding a full-source hash for each request-time hit check.

The implemented per-file version contract is:

- Track a version for each path's **effective visible source and kind**, with
  accessors on both `VirtualFileSystem` and `VfsSnapshot`. The effective value
  is the open overlay when present and the disk value otherwise.
- Draw versions from a monotonic allocator owned by one VFS lineage and shared
  by all of its clones through `Arc<AtomicU64>`. The VFS is cloned for every
  worker job and by `detached_clone_for_ephemeral_analysis`; copying the
  counter value would let two clones hand out the same next version.
  `validate_model_completion` already calls `open_document` on such a clone.
  Sharing the allocator makes versions unique within that lineage without
  introducing process-global state or coupling independent hosts and tests.
- **Detached clones keep the allocator.** `detached_clone_for_ephemeral_analysis`
  deliberately replaces the host's shared `Arc` caches (`completion_cache`,
  `definition_cache`, `snapshot_cache`, and others) with fresh ones. The VFS
  version allocator must **not** be added to that list, or the collision comes
  back. Put a comment at that function saying so, and cover it with a test.
- Give each allocator an opaque lineage ID recorded in every corpus, so
  adoption and queries reject versions produced by another VFS lineage.
- A bare version is meaningful only inside its VFS lineage. Corpus adoption
  and lookup compare it together with the corpus/eligibility generation; a
  version from one independent host is never compared with another host's.
- Preserve a version when an operation leaves the effective `(source, kind)`
  unchanged. Increment it when the effective source or kind changes.
- A path that is removed and re-added always receives a new version.
- Disk changes hidden by an open overlay do not change the effective version
  until closing the overlay changes the visible source or kind.
- Cover it with dedicated VFS tests:
  - updates that change nothing
  - a disk update while an overlay is open
  - opening and closing an overlay identical to disk
  - open/close overlay transitions
  - source-kind changes
  - exclusion and reinclusion
  - remove and re-add
  - cloned VFS and snapshot behavior
  - a detached clone and the coordinator both assigning versions never
    produce the same number for different content
  - `detached_clone_for_ephemeral_analysis` keeps the shared allocator while
    resetting the caches
  - version reported in snapshots

### Corpus structure

Use an immutable base with a small immutable overlay:

```rust
struct ProjectPatternCorpus {
    base: Arc<PatternBase>,
    overlay: Arc<BTreeMap<CorpusFileId, OverlayEntry>>,
    files: Arc<BTreeMap<CorpusFileId, CorpusFile>>,
    eligibility_generation: u64,
    approximate_bytes: usize,
}

enum OverlayEntry {
    Table(Arc<FilePatternTable>),
    Tombstone,
}
```

Use corpus-local numeric `CorpusFileId` values rather than the existing
path-backed `VfsFileId`, so full paths are not stored in every hit.

Each file record contains:

- Normalized path identity
- The VFS version it was built from
- Source kind
- Compact retrieval table
- The lexical module-specifier list the table was resolved from
- Resolved require targets, produced during the background build from those
  specifiers plus the already-loaded in-memory project model
- Declared LÖVE callbacks

Eligibility (ignored, generated, vendored, oversized, unsupported) is applied
by the LSP when it publishes a corpus. A settings or project-model change
increments `eligibility_generation` and immediately invalidates the old corpus
for queries. Background work then publishes a corpus for the new generation.
Core does not reinterpret the project model on the request path.

### Query consistency

Before reading hits, require the corpus's `eligibility_generation` to match the
current project inputs. Otherwise return no project candidate. For every
matching base hit:

1. Resolve its `CorpusFileId`.
2. Check whether the overlay replaces or removes it.
3. Compare the hit's VFS version with the file's current VFS version.
4. Exclude the active line.
5. Return only current hits.

Stale hits disappear immediately:

```text
didChange
  -> VFS version increments
  -> stored base version no longer matches
  -> stale base hit is rejected
  -> rebuilt overlay becomes available later
```

`didChange` never rebuilds the corpus synchronously.

### Background building and adoption

- Workspace discovery schedules background construction in chunks.
- Each per-file table is built from the worker's VFS snapshot and records the
  VFS version it was built from.
- The worker returns tables plus their versions to the coordinator. This
  follows the existing warm-up pattern (`warm_result` → `adopt_warmed_file`).
- The coordinator adopts a per-file table only if its version still matches
  the current one and the build's eligibility generation still matches.
- Changed files stay absent or tombstoned until they're rebuilt.
- Valid tables from the same background batch can still be adopted.
- A new immutable corpus value is then published.

Edits:

1. Mark the path dirty.
2. Coalesce rebuild work after an idle delay.
3. Build from the VFS overlay, never from disk.
4. Adopt only if that file's version still matches.

Rebuild the base while idle once the overlay exceeds a measured file or byte
threshold. Until then, overlay entries replace base entries, so nothing ever
has to be subtracted from an aggregate.

### Retrieval index

Store:

- Exact normalized line prefixes
- Collision-checked keys
- Original continuations
- Corpus file ID
- Line identity
- The file's VFS version
- Frequency and tier metadata

Hashes may speed up lookup but never establish identity on their own.

Tiers remain distinct so evaluation can enable them independently:

1. Current file, excluding the active line
2. Required modules
3. Same-directory files
4. Rest of the project
5. Bundled corpus

The current tune-only production policy holds out the active file, excludes
same-directory and bundled results, and considers required-module and other
project files together when forming the supported common prefix. That policy
is an integration candidate, not a shipped default.

Required-module relationships for tier 2 come from each file's own table, but
they are never discovered or resolved on the request path:

- **Lexical extraction already exists.** `module_specifier_literals`
  (`module_link/mod.rs`, already `pub(crate)`) and `scan_type_imports`
  extract raw module specifiers from Tua or Lua source without loading a
  `ProjectModel` or touching the filesystem. The refactor splits
  `module_links_tua_source` into this lexical step plus a resolution step that
  takes a supplied model. No new extractor is written.
- **Resolution happens in the background.** Background corpus construction
  resolves those specifiers through the same module-resolution implementation
  used by dependency analysis (`ProjectModel::resolve_required_module`,
  `resolve_imported_module`, and the type-import equivalent). The LSP's
  already-loaded immutable `ProjectModel` is supplied explicitly as build
  input. Do not duplicate resolution rules in the inline-completion module.
- **Resolved targets are stored per file:** in that file's table, next to the
  specifier list they came from.

#### Reusing resolved targets while the active file changes

Tier 2 is defined relative to the active file, and the active file's VFS
version changes on every keystroke. If its resolved targets were tagged only
with that version, tier 2 would be unavailable almost every time an inline
request fires, 50–75 ms after a keystroke. Resolution depends on the
specifiers, the project model, and the file layout. It does not depend on the
rest of the source. So:

1. At request time, extract the active file's specifiers lexically with
   `module_specifier_literals`. This is source-only and bounded by the Phase 0
   size cap. It involves no parse, no model, no resolution, and no disk.
2. Take the most recently adopted table for that file, even if its VFS version
   is stale.
3. If that table's specifier list is identical to the current one, **and** the
   corpus's eligibility generation still matches, reuse its resolved targets
   for tier-2 weighting.
4. Otherwise, required-module weighting for the active file is unavailable
   until its rebuilt table is adopted. Same-directory weighting is still
   available from paths alone.

Only an edit to a `require` or `import` specifier invalidates the targets. A
module file created since the last rebuild is weighted as tier 3 until the
corpus catches up, which is acceptable. A stale table's *patterns* still never
participate. Only its resolved targets are reused, and only under the
specifier-identity check.

For files other than the active one, resolved targets stay tied to each file's
VFS version, so they become ineligible together with that file's patterns.

The existing `require_targets_for_source`/`module_links_tua_source` path is not
used by inline requests. It calls `ProjectModel::load_for_path`, which walks
parent directories and loads configuration from disk, uncached, once per
specifier, so it doesn't satisfy the request-time contract. If retained for
other callers, it should delegate to the shared lexical step and the shared
resolver, after obtaining its model through its existing compatibility path.

The corpus also stores a versioned per-file set of declared LÖVE callbacks,
but current completion does not consume it. If callback generation is
reevaluated and passes its own gate, it must use this index to suppress
callbacks already declared outside the active file; tables with stale VFS
versions must never participate.

### Memory

Track approximate bytes for:

- Strings
- Map and vector capacity
- Interned paths
- Per-file tables
- Hit lists
- Overlay tables

Apply:

- Total corpus byte cap
- Per-file byte cap
- Maximum source size
- Maximum continuation length
- Deduplication of identical continuations

Skip, at the point of publishing:

- Ignored files
- Generated output
- Vendored dependencies
- Unsupported source kinds
- Oversized files

### Bundled corpus

A small, hand-written Lua/LÖVE corpus is built once with `OnceLock`.

It should cover only high-value, stable idioms such as:

- `pairs` and `ipairs` loops
- `table.insert` and `table.remove`
- `love.graphics` draw and color patterns
- `dt`-scaled movement
- Common callback shapes
- Basic Tua class and state patterns

Treat it as another retrieval source, not as training data merged into the
project corpus. The corpus and its evaluator backend exist, but production
completion does not query it because tuning did not support enabling it.

### Phase 2 settings

The server currently recognizes the following hidden source switch solely for
integration testing:

```text
tua.completion.inline.sources.project = false
```

It is not contributed by the VS Code extension and is not part of the public
configuration contract. Only expose approved settings if the Phase 2 gate
passes:

```text
tua.completion.inline.sources.project = true
tua.completion.inline.sources.bundled = true
```

### Phase 2 gate

Project retrieval must improve held-out characters saved per eligible
position over Phase 1, for Tua and Lua separately, without violating:

- Precision threshold
- Incorrect-character threshold
- Inline latency budget
- Completion contention budget
- Corpus memory cap
- Diagnostics freshness

---

## Phase 3: Statistical predictor experiment

N-grams start as an offline backend in the evaluator. They don't ship until
they beat exact retrieval.

### Normalization

- Keywords and punctuation remain concrete.
- Known globals, fields, methods, and APIs remain concrete.
- Locals and parameters become slots.
- Slots are numbered by first appearance, separately for each context order.
- `-1` follows the lexer's actual two-token representation.
- A small evaluated set of numbers may remain concrete.
- Other numbers become `$N`.
- Sensitive or uncommon strings become `$S`.
- Unknown placeholders terminate decoding unless bound from current context.

Each n-gram order owns its own slot-renaming context.

### Keys and counts

- Use exact interned sequences or collision-checked hashes.
- Never generate code based solely on a hash match.
- Use immutable per-file tables tagged with VFS versions.
- Rebuild aggregates instead of subtracting from pruned mutable counts.
- Enforce explicit byte caps.

### Prediction

Evaluate:

- Maximum order around six
- Jelinek-Mercer interpolation
- Small beam width
- Mean-surprise stopping
- Strict token limits
- Single-line output initially
- Source-tier weighting
- Cancellation and deadline checks between beam steps
- Termination on unresolved slots or literals

### Rendering

N-gram output needs a proper token renderer, because it can combine pieces of
different examples.

Test:

- Unary versus binary operators
- Calls and indexing
- Dots and colons
- Commas
- Table constructors
- String quoting
- Tua-specific tokens
- Comments and newlines
- Round-trip tokenization

### Shipping gate

The statistical backend ships only if it beats semantic-plus-retrieval on the
frozen held-out sets (Tua and Lua separately), in characters saved per
eligible position, without exceeding:

- Incorrect-output threshold
- Latency budget
- Memory budget
- Contention budget

If exact retrieval wins, stop here and do not ship the n-gram engine.

### Phase 3 implementation and tuning evidence

The experiment now exists only behind tua-core's `test-fixture` feature. It
uses collision-free exact token-sequence keys, per-order slot renaming,
Jelinek-Mercer interpolation through order six, a three-wide beam, mean
surprise stopping, a 12-token single-line limit, and a renderer covered for
calls, indexing, member access, unary and binary operators, commas, and table
constructors. Unbound slots, uncommon numbers, and strings terminate decoding.
Per-file immutable tables make target-file holdout structural rather than a
post-query filter. Source, context, model-memory, file-count, deadline, and
cancellation limits all fail closed.

`make inline-eval-phase3` and `make inline-eval-phase3-smoke` are the only
entry points. The flag hard-refuses held-out and external roles in both the CLI
and core API. No statistical code is connected to AnalysisHost, the LSP, or the
extension.

Tune-only results show why the raw model must not ship: it reaches only 0.100
correct-prefix precision and offers 26.14 wrong characters for each character
saved. A pinned filter (assignment and `other` contexts, score at least 0.85)
is safer but extremely sparse. Used only after the Phase 2 candidate abstains,
it raises characters saved per eligible position from 0.149 to 0.162, raises
correct-prefix precision from 0.728 to 0.745, and lowers wrong-to-saved from
0.395 to 0.369. The filtered model itself suggests at only 0.005 eligible
positions, saves 0.013 characters per eligible position, and has 0.913
correct-prefix precision.

These figures supersede earlier tune reports that accidentally included
configured output roots. Manifest entries name workspace directories, but the
evaluator had used a file-oriented project lookup that started at each
directory's parent. Correct workspace loading now honors `tua.toml` source and
output roots; the regression is covered by a configured `src`/`build` fixture.
With that correction, Phase 2 adds no suggestions over Phase 1 on the tuning
set, so project retrieval has no current quality evidence despite its completed
production-path architecture.

The smoke fixture produced no filtered suggestions, so it corroborates only
execution, not quality. The small tuning gain may be overfit and is not an
approval to ship. The Phase 3 configuration must be reviewed and frozen before
either gate dataset is used; until that happens, the production decision
remains unchanged and exact retrieval is the last implemented source.

---

## Phase 4: Product hardening

Only after preceding gates:

> **Safe DX preparation (2026-10-21): implemented without changing candidate
> behavior.** The extension exposes a workspace-scoped toggle for the existing
> default-off deterministic backend, reports `Local`, `Ollama`, both, or off in
> its language status, and records source-free returned/accepted counters for
> semantic and current-file candidates. The counters are observational only:
> they neither rank nor generate suggestions. Project, corpus, n-gram, and
> multiline settings remain unexposed.

- Evaluate multiline block completion and expose `maxLines`.
- Evaluate empty-statement completion.
- Require balanced `end` and delimiter structure for multiline output.
- Increase the required score per additional line.
- Calibrate scores before exposing them as confidence.
- Add acceptance-based ranking only after enough exposures.
- Record counts as "returned" and "accepted," never "shown."
- Use a neutral prior or log-odds adjustment.
- Do not multiply directly by `(accepted + 1) / (returned + 2)`.
- Add standard `textDocument/inlineCompletion` support.
- Revisit speculative-lane sizing only if later contention measurements show
  the single-worker/latest-queued design is insufficient.
- Feed pattern information into menu ranking only if menu benchmarks improve.
- Keep the language-scoped `editor.suggest.preview` default as a separate UX
  choice, measured by the real-editor replay rather than treated as engine behavior.
- Deprecate old multi-token settings only after replacement stability.
- Remove hardcoded multi-token rules only after measured parity or improvement.

---

## Later proposal: personal history across projects

Personal history remains outside the main implementation sequence.

### Initial behavior

It ranks candidates produced by semantic, project, and bundled sources. It does
not generate new text.

### Persisted data

Store only:

- Locally salted structural-key hashes
- Returned counts
- Accepted counts
- Last-seen time
- Distinct-project support
- Schema and tokenizer versions

Do not store:

- Source lines
- Paths
- String literals
- Module paths
- Project names
- Candidate insert text

To count each project once when tracking how many projects support a pattern,
store:

- A locally salted project fingerprint
- Salted `(project, structural pattern)` membership

Alternatively, use an approximate distinct-project sketch if exact deletion is
not needed.

### Storage

Begin with a versioned compact file in Tua's OS-specific user-data directory.

Move to SQLite only when requirements such as these justify it:

- Per-project deletion
- Transactions
- Large-scale eviction
- Schema migrations
- Efficient inspection

Database or file contents are loaded into an immutable in-memory snapshot.
Inline requests never read persistent storage.

### Privacy and trust

- Disabled by default
- Dedicated reset command
- Location and size visible to the user
- Workspace trust supplied by the extension
- No learning from untrusted workspaces
- No telemetry or upload
- `TUA_INLINE_COMPLETION_HISTORY=0` for benchmarks and reproducible tests
- Chronological cross-project evaluation before release

---

## Critical files

### New core files

```text
crates/tua-core/src/inline_completion/
  mod.rs
  eligibility.rs
  recall.rs
  semantic.rs
  validate.rs
  retrieval.rs
  eval/
  tests.rs

crates/tua-core/examples/inline_eval.rs
```

Phase 2 adds:

```text
crates/tua-core/src/inline_completion/corpus.rs
crates/tua-core/src/inline_completion/corpus/*
crates/tua-core/src/inline_completion/project_corpus.rs
```

Phase 3 adds an evaluator-only tokenizer, model, predictor, and renderer under
`inline_completion/eval/ngram.rs`. It remains excluded from production unless
its frozen gate passes.

### Core modifications

- `analysis/host.rs`
  - `local_inline_completion`
  - narrow exact-source cache match and parse-error peeks (never build and do
    not clone the full analysis file)
- `analysis/database.rs`
  - Crate-private peek into `files`
  - (Phase 2) Split `module_links_tua_source` (`module_link/mod.rs`) into the
    existing lexical `module_specifier_literals` step plus resolution against
    an already-loaded `ProjectModel`; do not expose the disk-loading
    compatibility wrapper to the inline request path
- `analysis/host.rs` (Phase 2)
  - Comment and test: `detached_clone_for_ephemeral_analysis` keeps the shared
    VFS version allocator
- `completion/mod.rs`
  - Narrow shared statement/callback primitives, with no behavior change
- `love_callback.rs`
  - Shared callback catalog, declared-callback lookup, and declaration helpers
- `model_completion.rs`
  - Expose only reusable diagnostic mapping helpers
- `vfs/mod.rs` (Phase 2)
  - Per-path effective-source version, drawn from a per-lineage
    `Arc<AtomicU64>` allocator so it's unique across related clones, and bumped
    only when the visible source or kind changes
- `lib.rs`
  - Protocol-neutral public result types where necessary

### LSP modifications

- `handlers.rs`
- `request_registry.rs`
- `scheduler.rs`
- `mod.rs`
- `configuration.rs`
- `tests/inline_completion.rs`
- `tests/scheduling.rs`

Phase 2 also needs a result type for background corpus builds and their
adoption, modelled on `WorkspaceWarmResult`.

### Extension modifications

- Replace `ollama-completion.js` with `inline-completion.js`
- Update `editor/index.js`
- Update `package.json`
- Add unit tests
- Add real-server tests

### Build and evaluation

- `Makefile`
- `scripts/fetch-inline-eval-projects.*`
- External dataset manifest with URLs, commits, checksums, and licenses
- Tua dataset manifest recording the per-project tuning/held-out split
- Committed frozen Tua holdout fixtures
- Committed CI smoke fixtures

---

## Verification

### Core

Test:

- Active-line exclusion
- Original-text preservation and mapping offsets back to the original line
- Ambiguity suppression
- String and comment rejection
- Agreement with existing text after the cursor
- Source-only snippets when the semantic cache misses
- Phase 1 never produces callback ghost text and source-only candidates are
  identical whether the current-file cache hits or misses
- (Phase 2 foundation) The versioned callback declaration index uses the shared
  exact declaration query, not fuzzy or display-name symbol lookup
- (Phase 2 gate, deferred) If callback generation is reintroduced, prove that
  it consumes that index for duplicate suppression
- Inline queries build nothing: rebuild counters (`record_file_rebuild`) are
  unchanged and the snapshot cache revision doesn't move
- Size check before parsing
- Edited text parsed first: a clean edit is accepted with exactly one parse
- Complete source-kind parse comparison before and after the edit
- Errors reported after the insertion point
- Tua model errors from classes, sprites, declaration files, and typed syntax
  reject invalid candidates
- Cancellation and deadline behavior
- Recall-cache byte eviction, if caching is added
- (Phase 2) VFS effective-source versions: unchanged updates, hidden disk
  updates, overlay transitions, kind changes, exclusion/reinclusion, cloned
  VFS state, and remove/re-add
- (Phase 2) Versions stay unique when a detached clone and the coordinator
  both assign them, and `detached_clone_for_ephemeral_analysis` keeps the
  shared allocator
- (Phase 2) While the active file changes without touching its `require` or
  `import` lines, tier 2 reuses the previously resolved targets
- (Phase 2) Editing a specifier, or changing the eligibility generation, stops
  that reuse immediately
- (Phase 2) A stale table's patterns never participate, even when its resolved
  targets are reused
- (Phase 2) Tier 2 uses require targets resolved during background construction
  from lexical specifiers and the already-loaded project model
- (Phase 2) A file whose specifiers changed loses its old require targets
  immediately. No request-time parse, project-model load, module resolution,
  or disk read is used before its replacement table is adopted. Lexical
  specifier extraction is the only request-time work.
- (Phase 2) The shared module resolver gives dependency analysis and corpus
  construction identical results for configured roots and relative imports
- (Phase 2) Eligibility-generation changes reject the old corpus immediately
- (Phase 2 gate, deferred) If callback generation is reintroduced, prove that
  declarations in another current project file suppress duplicates and stale
  declaration tables do not participate

### LSP

Test:

- Stale versions return empty
- Newest request cancels the previous request for the same URI
- Disabled mode queues no work
- Capacity errors are handled
- Cancellation flags reach core
- Inline work does not noticeably delay menu completion
- With `selectedCompletion`, Phase 1 items use the selected replacement range
- (Phase 2) Background corpus adoption rejects per-file tables with stale
  versions
- (Phase 2) Base hits are rejected immediately after a VFS change

### Extension

Test:

- Local backend before Ollama
- Ollama fallback
- Debounce
- Edit and cursor cancellation
- Filtering to candidates that extend the selected menu item
- Stale-version rejection
- Silent capacity and cancellation handling
- Snippet insertion
- Disabled mode sends no request

### Evaluation and performance

Run:

```sh
make inline-eval
make inline-eval-smoke
make inline-eval-external

make profile-completion
make profile-representative
make editor-regression-smoke
make profile-large-tables

make verify
make ci-check
```

`inline-eval-external` is not part of ordinary offline CI.

### Manual verification

In `examples/love-tua`:

- Recall after `(`, `,`, operators, whitespace, statement prefixes, and
  identifiers
- No recall in the intentionally excluded assignment and member-access
  contexts
- Extend a selected menu item
- After callback generation independently passes its gate, suggest missing
  callbacks and suppress callbacks declared in any current project file
- No suggestions in comments or strings
- No duplicated closing delimiters
- Correct behavior during invalid edits
- Ollama only after local sources return nothing

---

## Related follow-up (outside this plan)

`changed_file_dependency_invalidation` (`analysis/database.rs`) calls
`require_targets_for_source` while handling an edit, for any file that other
files import. That path resolves every specifier through
`ProjectModel::load_for_path`, so today each such edit does a directory walk
and a configuration load per `require`.

The Phase 2 resolver refactor (lexical extraction plus resolution against a
supplied model) would let dependency invalidation stop doing that too. Track
it as a separate, measured issue (`make editor-regression-smoke`,
`make profile-representative`), not as part of inline completion.

## Realistic scope

- **Phase 0:** 1.5–2 weeks. It covers:
  - menu-state replays in the extension
  - the external dataset fetcher with checksums and license manifest
  - the Tua project split and frozen holdout fixtures
  - scenarios and result breakdowns
  - baselines
- **Phase 1:** about two more weeks.
- **Phase 2:** another two to three weeks, including the VFS version change.
- **Phase 3:** two to four weeks of experimentation, which may be stopped.
- **Phase 4:** separate hardening work.
- **Personal history:** a separate proposal and evaluation.

The shippable MVP is Phases 0–1, roughly 3.5–4 weeks. Phase 2 is the first
genuinely project-aware expansion. N-grams, multiline generation, and
persistent personal learning go ahead only when the earlier measurements show
enough value to justify them.
