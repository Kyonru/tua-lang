# Independent integration review — 2026-11-07

Scope: integrated feature-audit fixes since `9d490dc`; reviewed through
`cc0298c8ebfcdc5c95f97509136350ab90d851ec` plus the live working tree.
This is an independent review of the integration, not a replacement for the
original 31 feature investigations or final release qualification.

## Assessment

The fixes generally use shared semantic infrastructure: module export contracts,
function-type overload metadata, domain registry constructors, and optional-field
value types. Ownership fixes require recorded fingerprints before deleting build
outputs. These are preferable to editor-only symptom patches.

Final approval remains conditional on the lead's fresh full-suite run. During this
review the lead found integration regressions in that run and assigned repairs;
this document must not be cited as evidence that those gates passed.

## Independently checked

- Nominal aliases: reviewed assignability ordering, source-union handling, and
  dynamic `any` compatibility. Ran five existing nominal tests successfully.
- Overloads: reviewed propagation through function types, module export rewriting,
  inferred-type widening, and conversion back into callable metadata. Ran thirteen
  overload tests successfully, including imported Lua overloads, LOVE overload
  checking, active-signature selection, and Tween's whole-contract validation.
- Inline imports: export-extraction regression passes. The shadowed-loader
  regression failed in the then-existing test binary (`local result: any`); this
  was reported immediately. The lead was already replacing the loader guard, so
  this intermediate failure is not attributed to the eventual final tree.
- Module recursion: inspected the new normalized-path, thread-local in-progress
  set and RAII guard. No RefCell borrow survives recursive inference. Drop removes
  entries on early returns/unwind; a 32-entry bound limits recursion even when
  path aliases avoid direct identity detection. The lead added this after a real
  module-cycle stack overflow in the full suite. Its fresh test outcome belongs
  in the final verification log.
- Ownership: reviewed fingerprint-matching deletion, refusal to remove unrecorded
  or changed output, output-root identity reset, and bounded on-disk cache
  fingerprint loading. Existing fresh adversarial CLI evidence is in
  `post-fix-review/ownership.json`, `pipeline-root.json`, and `cache-integrity.json`.
  Those probes were performed by the other integration reviewer, not rerun here.
- Formatter: reviewed opening-line multiline-state handling and restoration of
  trailing string/comment content. Runtime and LSP reproduction evidence is in
  `post-fix-review/formatter.log`; this review did not independently rerun it.

Commands (existing nonincremental test executable at time of execution):

```sh
target/debug/deps/tua_core-44ac6552d922e736 overload --nocapture
# 13 passed
target/debug/deps/tua_core-44ac6552d922e736 nominal --nocapture
# 5 passed
target/debug/deps/tua_core-44ac6552d922e736 inline_require --nocapture
# 1 passed, 1 failed (intermediate shadowed-loader regression)
```

## Inline smoke snapshot: pre-existing, not cleared

Re-executed the pre-feature-fix evaluator executable (file timestamp November 7,
16:11) with per-position records:

```sh
target/debug/examples/inline_eval \
  --manifest benchmarks/inline-eval/manifest.toml --set smoke \
  --max-positions 0 --records --out /tmp/tua-inline-review.json --json
```

The aggregate differences **exactly reproduce** the release-preparation mismatch.
The compact independent result is
[final-inline-baseline-diff.json](post-fix-review/final-inline-baseline-diff.json).
Among the differences, phase1 gains five suggestions and eleven saved characters;
current-file and phase1-prototype each gain two suggestions and twenty incorrect
characters. This is a deterministic behavior change, not a timing assertion.

The expected snapshot was last updated in `c1a9988` after a deliberately reviewed
parser diagnostic change. `git diff c1a9988 -- benchmarks/inline-eval/smoke`
is empty. Between that snapshot and `9d490dc`, changes under
`crates/tua-core/src/inline_completion` only alter documentation, but the parser
and normalization changed substantially. Candidate admission calls the parser in
`inline_completion/validate.rs`; acceptance depends on the candidate versus
baseline diagnostic multiset. Therefore parser changes are a plausible cause,
not a proven per-position explanation.

**Safe action:** preserve the current expected snapshot until a controlled
comparison of per-position records at `c1a9988` and the current revision identifies
the admitted/rejected positions and validates their syntax/diagnostics. Then
update only justified expectations in a dedicated commit with that evidence.
Neither the aggregate diff nor the absence of inline algorithm edits alone
justifies refreshing the snapshot. No snapshot or evaluator source was changed
by this review.

## Additional regression probe and limits

A same-thread filesystem fallback probe was added temporarily to module-index
tests: A exports `require('b').value`, B changes from number to string, and A is
queried again unchanged. Source-only cache identity may miss this dependency
change; the probe checks that specific risk rather than extrapolating from source
inspection. The fixture is preserved at
[transitive-cache-regression.rs](post-fix-review/transitive-cache-regression.rs).
**Confirmed integration defect, Medium severity, high confidence.** A freshly
compiled focused run failed at `module_index/tests.rs:801`: the second query
returned `Some(Primitive(Number))`, expected `Some(Primitive(String))`. The first
query correctly inferred Number. Exact command:

```sh
CARGO_INCREMENTAL=0 cargo test -p tua-core \
  final_review_transitive_module_cache_tracks_dependency_edits -- --nocapture
```

Evidence: [transitive-cache.log](post-fix-review/transitive-cache.log). The temporary
test remains in `module_index/tests.rs` at the lead's request for the repair.
`required_module_exports_for_path` validates its cache against only the queried
file's source; newly inferred transitive dependencies are absent from that key.
Recommended fix: track dependency fingerprints or conservatively avoid retaining
filesystem-fallback exports that depend on other modules. Keep the project
snapshot's dependency-aware path. Acceptance: both number-to-string edits and
dependency deletion change the result on the same thread without editing A or
restarting the process. The lead was notified immediately; this report records
pre-repair evidence and does not imply that the eventual delivered tree retains
the defect.

Position-insensitive `is_unshadowed_lua_root` conservatively treats a shadowed name
anywhere in the AST as unavailable globally. Applying it to `require` prevents
false built-in resolution but can reduce information at unrelated scopes. This
is an inspected precision limitation, not a reproduced final-tree defect here;
prefer the existing position-aware binding infrastructure in further work.

No product fixes, commits, publishing, or external communication were performed
by this reviewer. Full platform/editor qualification, long sessions, and all
possible import-cycle shapes are outside this independent pass. The lead owns
final fresh builds, integration regression repairs, and release gates.

## Follow-up implementation (authorized after the initial review)

The lead delegated EDITOR-G01 after the review-only pass. Actual protocol evidence
showed `Input:get("move")` selecting the first broad literal union instead of the
specific pair-return overload. The first-match algorithm considered both valid
and never compared specificity. Commit `5acbb1e` ranks matching literal contracts
by number of alternatives, preserving declaration order for ties. The new real
Input pair/scalar regression passes; all 39 signature-help tests passed in a fresh
nonincremental run. Protocol replay after a fresh CLI rebuild remains pending.
No unrelated code was included in that unsigned commit; no co-author trailer.

The transitive cache finding above is repaired in `0bffb26`. The lead's subsequent
full core run passed that regression and the recursion/shadowing/provenance
controls, but reported five remaining checker regressions (1716 passed, five
failed, one ignored). Those repairs are assigned to the language owner. See
[IMPLEMENTATION.md](IMPLEMENTATION.md) for all 71 normalized findings and their
implementation/test-artifact mapping. Final gate results must be appended after
the final source changes, not inferred from these intermediate runs.

Fresh actual protocol replay now passes after `CARGO_INCREMENTAL=0 cargo build -p
tua-cli`: `node internal/audits/features-2026-11-07/post-fix-review/editor.js`
asserts active signature 2, `function c:get(name: "move"): number, number`.
Raw results are refreshed in `post-fix-review/responses.json`; the replay also
returns configured audio cue `ping` and buses `master`/`sfx`. The first attempt
hit a 30-second initialize timeout on the fresh executable; retry used a
120-second audit-harness request deadline and passed. This changes no product
request deadline and is not a typing-latency measurement. No further product
changes were needed after the focused signature tests.

## Inline baseline cause resolved by a controlled comparison

A retained October 26 evaluator exactly reproduced the previous expectations.
Historical/current per-position comparison found 19 removed and 33 added backend
records, all in `grid.lua`: method-declaration `Grid` prefixes and the indexed
assignment on line 31. Repeating both runs with `--no-menu --records` yielded
identical complete record arrays and totals. This isolates the cause to ordinary
completion-menu selection/admission and supersedes the earlier parser hypothesis.
See [EXPECTATIONS.md](../../../benchmarks/inline-eval/smoke/EXPECTATIONS.md) and
[raw causal evidence](post-fix-review/inline-menu-causal.json). A dedicated snapshot
refresh now records those explained menu changes; fresh current-tree `CARGO_INCREMENTAL=0 make inline-eval-smoke` passed.
Snapshot and provenance committed in `4611f73e`.

Final verification: `CARGO_INCREMENTAL=0 make inline-eval-smoke` rebuilt the
current-tree evaluator as needed and passed against the explained refreshed
snapshot. Commit: `4611f73e`. Log:
[inline-current-tree-check.log](post-fix-review/inline-current-tree-check.log).
