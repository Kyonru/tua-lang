# Inline-completion evaluation

Phase 0 of [`better-autocompletion.md`](../../better-autocompletion.md): an
offline evaluator that measures candidate sources for local inline completion
before their default or source scope expands. It lives behind the
`test-fixture` feature in
`tua_core::inline_completion::eval`, with a command-line front end in
`crates/tua-core/examples/inline_eval.rs`.

```sh
make inline-eval            # tuning sets -> target/tua-speed/inline-eval-tune.json
make inline-eval-smoke      # committed fixtures, compared with smoke/expected.json (CI)
make inline-eval-holdout    # held-out gate, checked against thresholds.toml
make inline-eval-external   # fetch pinned third-party projects, then gate on them
make inline-eval-phase-gate # aggregate decision; fails while any prerequisite is pending

# Ad-hoc paths: directories are projects, files stand alone.
cargo run -p tua-core --features test-fixture --example inline_eval -- examples/features --json
```

`make representative-phase1-gate` first records the full seven-fixture
scheduler/server matrix with independent strict 10 ms limits for
dispatch-to-response latency, worker execution, and core/backend work, plus a
1,500 ms p95 ceiling for client-observed contended request age, in
`target/tua-speed/representative-phase1-gate.json`. It is a dependency of
`make inline-eval-phase-gate`, which then checks every prerequisite and its
evidence in `phase1-gate.toml`; a passing quality dataset alone is insufficient.

`--records` keeps every suggestion in the JSON report. `--min-prefix`,
`--min-support`, `--min-full-prefix-support`, `--dominance`,
`--recall-variant unique|common-prefix`, and `--contexts open-paren,comma,...`
configure the exploratory `phase1-prototype` backend while tuning. The shipping
`phase1` backend always runs the exact host query and its fixed production
policy. The full-prefix support
threshold lets a single matching line offer only its first token while repeated
patterns can offer their whole common continuation. `--no-menu` and
`--no-warmup` skip the analysis-backed measurements for a faster run, and
`--max-positions 0` evaluates every position.

`--menu-replay FILE` replaces simulated menu selection with explicitly enabled
local JSONL captured from the VS Code provider. Start the extension host with both
`TUA_INLINE_REPLAY_ROOT` set to the manifest's dataset root and
`TUA_INLINE_REPLAY_CAPTURE` set to a fresh local JSONL path. Capture is inactive
when either variable is absent, ignores files outside the canonical root, and
never uploads data. Enable `tua.completion.inline.enabled` as well: capture
observes the Phase 1 provider after its normal debounce and stays idle when the
feature is off. It stores a relative path, SHA-256 hash of the edited in-memory
document (with CRLF normalized to LF, matching evaluator scenarios), zero-based
UTF-16 position/range, document version, elapsed time
since an observed edit, menu-open state, and the selected completion text when
present; it does not store the document body, absolute URI, wall-clock time, or
inline result.

The relative path reveals part of the dataset layout, and selected completion
text can contain source-derived identifiers or literals. Treat the capture as
potentially sensitive local evaluation data: keep it out of version control and
delete it when the replay is no longer needed. Tua does not upload it.

To replay real typing against known truth, leave the fixture unsaved, delete
the rest of a code line from a sampled cursor, and type it back. The resulting
in-memory document matches the evaluator's `edit` scenario; editor-inserted
closing delimiters may match `auto-close`. The evaluator applies a record only
when its edited-document hash, cursor, and selection range match a generated
scenario exactly. It rejects malformed entries, unknown files, invalid UTF-16
positions, and positions that fail core eligibility, and counts valid records
that match no scenario as unmatched. Matched events use the real selected item
instead of `menu_selection`; reports retain separate `replay-open` and
`replay-closed` results. Use replay capture only with tuning data while choosing
behavior. It does not authorize inspecting a gate set before its configuration
and thresholds are frozen.

## Datasets

`manifest.toml` records the split. It is by project, never by file. Paths are
relative to its `root`, itself relative to the manifest's directory, and are
canonicalized, so `./examples/foo`, `examples/../examples/foo`, and a symbolic
link to it are the same path. No path may appear twice or contain another, in
one set or across sets. Before evaluation, files with identical content (after
normalizing line endings and surrounding whitespace) are kept only in the set
with the highest-priority role: holdout, then external, smoke, and tune.

| Set | Role | Contents |
| --- | --- | --- |
| `tune` | Tuning | `love-full`, `features`, `project-model`, `love-minimal`, and the bundled runtime `.lua` files |
| `holdout` | Gate | `love-tua`, `audio-system` (minus generated output), and the frozen fixtures in `holdout/` |
| `smoke` | CI | `smoke/`, pinned in `smoke/expected.json` |
| `external` | Gate | Pinned MIT-licensed LÖVE projects from `external.toml` |

Rules that keep the gates honest:

- **Never tune on a gate set.** Tune recall settings, source order, and
  thresholds on `tune` only.
- **`holdout/` is frozen.** Its files were written before any tuning. Don't
  edit them or add files to make a number move.
- **Thresholds come first.** `thresholds.toml` holds the Phase 1 acceptance
  criteria and, in its `[phase1]` table, the exact configuration to evaluate.
  Choose both on `tune`, commit them, then run each gate once. The evaluator
  refuses to run a gate set without `--thresholds`, and a gate fails when it
  can't be checked: no gate set evaluated, no thresholds for that set, a
  missing backend or configured language, no eligible requests, too few
  suggestions, or a language with no limits. Holdout names only Tua limits;
  external names only Lua limits.
- **Report languages separately.** Tua and Lua results are summarized and
  gated independently, so the larger Lua corpus can't hide weak Tua results.
- **A quality gate is not phase acceptance.** `phase1-gate.toml` records the
  separate held-out/external quality, real menu replay, menu-completion
  performance, strict inline-server performance, client no-stall and
  responsiveness evidence, diagnostics freshness, scheduler cache-hit, and
  parse-distribution evidence. The 1,500 ms no-stall ceiling does not satisfy
  the pending interactive-responsiveness prerequisite. The extension separately
  cancels and abstains after a local backend waits 250 ms beyond its 65 ms
  debounce; that client protection and the synthetic contention gate measure
  different boundaries.
  Its checker fails on missing, pending, or evidence-free prerequisites.
- **Results stay local.** Reports go to `target/tua-speed/`; don't commit
  machine-specific numbers.

The offline `phase1` backend is intentionally cold. Its validation
zero/one/two-parse distribution diagnoses worst-case cost and is report-only;
`check_gates` does not mix it into held-out or external quality acceptance.
The aggregate validation prerequisite is decided from the representative LSP
run under real scheduling. That prerequisite requires complete counter
coverage, no two-parse scheduled attempts, and at most one parse for every
returned candidate. A zero-parse cancellation or deadline expiration is a safe
abstention, not a returned completion. Candidate-attempt and returned-candidate
parse distributions are reported separately; returned counts use request-local
parse counts, so concurrent requests cannot distort them.

`scripts/fetch-inline-eval-projects.js` checks out each external project at its
pinned commit into `target/inline-eval/external/`, then verifies the commit ID,
the license file, and a SHA-256 digest over the evaluated `.lua` files. It
needs network access and never runs in ordinary CI.

## What is measured

At each sampled position, the evaluator hides what the author wrote after the
cursor on that line and asks every backend for one suggestion.

- **Positions** are on code lines, outside strings, comments, and leading
  indentation:
  - after each whole token (`token-end`);
  - inside each word of two or more characters, after its first one, two, and
    three characters (`partial-1` to `partial-3`) and one character before its
    end (`partial-near`), where a user is still typing.

  At most 40 per file are kept (`--max-positions`), evenly spaced from the
  first to the last, so runs are deterministic. Positions at the end of a
  line, where nothing is left to type, are reported separately as
  *abstention* requests: the right answer there is no suggestion, so they
  don't share a denominator with the rest.
- **Scenarios** describe the document when the request fires. Each one is
  opened, warmed, and queried on its own, so it only knows its own text:
  - `cut`: new code at the end of a file; nothing after the cursor exists yet.
  - `edit`: an existing file with the rest of the active line withheld.
  - `auto-close`: like `edit`, with the closers for the line's still-open
    brackets already after the cursor, as an editor with auto-closing pairs
    leaves them. It applies only where the author's line ends with those
    closers. A suggestion that closes none of those brackets keeps them after
    it; one that does must end with them, and they are dropped from it;
    anything else would leave extra closers and is rejected.
- **Menu state** is simulated with real menu completion. After a word or a
  `.`/`:` trigger, the menu counts as open when an item continues the typed
  word, and the first such item is the selection. With the menu open, a
  suggestion counts only if the typed word plus the suggestion starts with the
  selected item's text, as VS Code requires. Real VS Code matching is fuzzy
  and sorts differently, so this remains a rough model when no validated
  `--menu-replay` capture is supplied.

### Backends

- `noop`: never suggests anything; the baseline.
- `semantic`: a statement snippet at the start of a line, or a LÖVE callback
  after `function love.`, each only on a unique match. Callbacks also require
  cached parsed facts for the current source.
- `current-file`: the unique (or dominant) continuation of other lines in the
  same file that start with the normalized line prefix.
- `current-file-prefix`: the part every matching continuation agrees on, cut
  back to a whole token.
- `phase1-prototype`: configurable source ordering and recall policy for tuning.
- `phase1`: the exact shipping host query, including budget, cancellation,
  suffix handling, parse-delta validation, and the fixed production policy.
  A gate rejects a `[phase1]` table that does not match that policy.

### Metrics

For a suggestion `s` and the author's actual continuation `t`:

- **Offered text** is what a user keeps without editing: the first line of
  `s`, and for a snippet only its literal text before the first placeholder.
  VS Code inserts the whole snippet, but its placeholder defaults are selected
  for the user to overwrite.
- **Exact**: the first line of `s`, with snippet placeholders filled by their
  defaults, equals `t`.
- **Template match**: for a snippet, `t` fits its first line with each
  placeholder standing for some non-empty text. For other suggestions it is
  the same as exact.
- **Correct prefix**: the offered text has a non-whitespace character and `t`
  starts with it, so accepting it is never wrong. This is the precision to
  gate partial suggestions on; exact match penalizes a suggestion for
  stopping early.
- **Characters saved**: the offered length when `t` starts with it, otherwise
  0.
- **Incorrect characters**: offered characters past the common prefix with
  `t`, which a user would delete. The **wrong-to-saved ratio** divides them by
  the characters saved.
- **First-token** precision compares the first tokens; **first-three-token**
  precision requires at least three offered tokens, all matching.

Rates ending in `/el` divide by eligible requests and precisions divide by
suggestions. Per-source rates also divide by every eligible request, so they
add up to the backend's. Every summary is broken down by language, scenario,
menu state, cursor context, prefix kind, project, document state (whether the
document parsed when the request fired), and source, with backend-only latency
percentiles and a precision/coverage curve over each backend's scores.

The report also measures:

- **Complete-parse cost** per language: after one untimed warm-up parse,
  every timed parse is kept (five per file by default), with latency
  percentiles overall and by file size, and throughput at the median and the
  slowest 5% of samples. The source copy is outside the timed section. The
  recommended byte cap is the largest document a 4 ms parse budget covers at
  the p5 throughput with a 2x margin, capped at 256 KiB. With few files per
  size bucket, treat it as a rough figure.
- **Warm-up hit rate**: each edited revision is warmed for real, and its
  duration is compared with request delays of 60 ms (the inline debounce during
  continuous typing), 150 ms, 300 ms, and 1 s. This assumes a warm-up starts at
  the keystroke and isn't queued behind other work, so it's an upper bound for
  the language server. `parsed facts` is the share of warmed revisions whose
  cache entry had declaration facts.
- **Memory**: heap bytes requested by setting each project's files and by
  building a full snapshot, as deltas counted by the example's allocator. This
  includes reserved but untouched capacity, so it can exceed the process's
  peak RSS. Compare with `make profile-large-tables` for RSS.

## Smoke expectations

`smoke/expected.json` pins the timing-independent counts for the smoke set.
After an intended change to the evaluator or a backend, run
`make inline-eval-smoke-update` and review the diff.
