# Inline-completion evaluation

Phase 0 of [`better-autocompletion.md`](../../better-autocompletion.md): an
offline evaluator that measures candidate sources for local inline completion
before any of them ships. It lives behind the `test-fixture` feature in
`tua_core::inline_completion::eval`, with a command-line front end in
`crates/tua-core/examples/inline_eval.rs`.

```sh
make inline-eval            # tuning sets -> target/tua-speed/inline-eval-tune.json
make inline-eval-smoke      # committed fixtures, compared with smoke/expected.json (CI)
make inline-eval-holdout    # held-out gate, checked against thresholds.toml
make inline-eval-external   # fetch pinned third-party projects, then gate on them

# Ad-hoc paths: directories are projects, files stand alone.
cargo run -p tua-core --features test-fixture --example inline_eval -- examples/features --json
```

`--records` keeps every suggestion in the JSON report. `--min-prefix`,
`--min-support`, `--min-full-prefix-support`, `--dominance`,
`--recall-variant unique|common-prefix`, and `--contexts open-paren,comma,...`
configure the candidate `phase1` backend while tuning. The full-prefix support
threshold lets a single matching line offer only its first token while repeated
patterns can offer their whole common continuation. `--no-menu` and
`--no-warmup` skip the analysis-backed measurements for a faster run, and
`--max-positions 0` evaluates every position.

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
- **Results stay local.** Reports go to `target/tua-speed/`; don't commit
  machine-specific numbers.

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
  and sorts differently, so this is a rough model until extension replays
  replace it.

### Backends

- `noop`: never suggests anything; the baseline.
- `semantic`: a statement snippet at the start of a line, or a LÖVE callback
  after `function love.`, each only on a unique match. Callbacks also require
  cached parsed facts for the current source.
- `current-file`: the unique (or dominant) continuation of other lines in the
  same file that start with the normalized line prefix.
- `current-file-prefix`: the part every matching continuation agrees on, cut
  back to a whole token.
- `phase1`: the candidate Phase 1 order, enabled semantic sources first and
  then recall. By default it is the plan's starting hypothesis (`current-file`,
  every context); the thresholds' required `[phase1]` table pins the recall
  variant, context allowlist, callback/snippet sources, and their prefix
  requirements for quality-gate runs.

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
document parsed when the request fired), and source, with latency
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
