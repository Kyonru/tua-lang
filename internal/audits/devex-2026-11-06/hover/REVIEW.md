# Independent review of workflow and typing audit reports

Reviewer: hover/signature lane, reused for a skeptical final review. Reviewed
`workflow/REPORT.md` and `typing/REPORT.md` against their scripts, raw results,
relevant documented contracts, implementation points, and negative controls.
No owner report or product file was edited by this reviewer.

## Outcome

The confirmed findings in these two reports have adequate reproducible evidence
and defensible severity. No High finding was overturned, and no unsupported
language feature was incorrectly promoted to a defect in the reviewed material.
The reports distinguish observations, code-path root-cause inferences and
proposed fixes. Two precision issues were found and corrected or sent to the
lead; details below preserve the review history.

## Corrections and clarifications

1. **WF-02 single-root control / false-positive wording — resolved in evidence.**
   The initial `workflow/lifecycle.js` used a `number` assignment in both roots,
   even though B exported string. That made B invalid alone and meant “both
   projects pass single-root controls” was not established by that script.
   The independent typing fixture already used matching number/string contracts,
   so the module-isolation defect itself remained established. The lead changed
   B to a string contract, added real single-root A/B controls, and reran the
   lifecycle evidence. The latest JSON contains `single-root-control-a`,
   `single-root-control-b` and the failing `multi-root-isolation` session.
   This reviewer additionally ran distinct matching-contract CLI/LSP controls:
   `hover/review-single-roots.js` and `review-single-roots-evidence.json`.
   Both check successfully, return the expected number/string hover, and have
   empty pull diagnostics. These controls can be linked from the final report.

2. **WF-02 root-order explanation — wording clarification recommended.**
   Different fixture layouts select different winning roots. The evidence does
   not contain a systematic same-fixture root-order permutation experiment.
   Prefer “the winning project differed across fixture layouts” to a causal
   claim that a specific root-order change was experimentally isolated. The
   global-root source inspection supports an ordering hypothesis, which should
   stay labeled as such. The lead acknowledged this wording refinement.

3. **Typing reproduction commands — resolved.**
   Earlier finding paragraphs used literal `node .../typing/run.js` placeholders.
   The full runner path appeared in the introduction, but the user requested
   exact runnable commands. The typing agent replaced these with full repository-
   relative commands. The latest report has no `.../typing` command placeholders.

4. **TYP-05 should remain a definite-absence finding, not an anti-`any` claim.**
   The report correctly separates a missing known scalar/parenthesized slot from
   unknown dynamic function returns and deliberate no-return stub inference.
   `docs/language.md:344–355` supplies further support: nil-only initialization
   establishes a nil binding contract unless explicitly broadened. The captured
   TL4028 warning independently states that the second slot is nil while hover
   reports any. Preserve this precise distinction in the final backlog.

## Evidence checks performed

- **WF-01:** the independent `score`/`points` rename reproduction from this lane
  is genuinely distinct from the lead's `value`/`renamed` fixture. Applying server
  edits with UTF-16 indexing corrupts adjacent characters only after the emoji
  prefix; ASCII control is exact. The captured VS Code result applies those edits
  to an actual text document, not just a hand-built string. Source conversion
  functions at `protocol.rs:1518` and `:1572` lack document-aware encoding
  conversion. High severity is justified by source corruption; no claim of an
  entire-server crash is needed.
- **WF-02:** compared raw hover paths/diagnostics with fixture contracts and
  independent multi-root results. The distinct importer filenames in the typing
  reproduction rule out an explanation based solely on duplicate `main.tua`.
  The corrected isolated controls establish the unwanted project interaction.
- **Actual suggestion UI:** visually inspected `workflow/editor-suggest.png`.
  It visibly says **No suggestions** at `createEnt`. The report correctly
  rejects `offsetParent: false` as a visibility oracle for this fixed-position
  widget and relies on the actual screenshot. Member completion control succeeds.
- **Performance:** every table value checked agrees with
  `workflow/performance-evidence.json`. There are 3 cold, 30 warm and 30 post-edit
  completion samples per project, plus hover/signature samples and 3 incomplete
  samples. The script checks the expected imported completion member and rejects
  null sample responses. Measurements are stdio request times, not keystroke/UI
  latency; the report makes that limitation explicit. They do not establish a
  before/after performance regression or long-session behavior.
- **TYP-01/TYP-02:** inspected CLI/LSP results and the lead's distinct
  `render`/`module.make().stats.hp` cross-checks. Both builds execute under Lua and
  print `number` despite string-facing contracts. The alias callback control and
  split property-chain control are meaningful and reduce the chance that this
  is an intentional dynamic policy. High severity is justified.
- **TYP-03/TYP-04:** independent probes in the hover lane already confirm native
  vararg metadata loss (LuaDoc control rejects) and final-call multi-return arity
  rejection. Existing raw responses support the typing report; these are counted
  once under typing, not again under hover.
- **TYP-05:** raw results contain the contradictory known-nil linter warning,
  successful checker status and `any` hover. The pure Lua oracle checks scalar
  and parenthesized-call slots. Runtime evidence is labeled Lua 5.4, not falsely
  advertised as an executed LuaJIT check.
- **Request validity:** checked bounds for all **94 typing-lane probe positions**
  against the exact `cases.json` source buffers: zero out-of-bounds positions.
  Manually checked the high-finding symbol anchors, signature positions and
  workflow editor cursor positions. Boundary positions for one-character
  parameters still resolve the intended token in the captured response; they
  are not unrelated root-name queries. Results are recorded in
  `hover/review-validation.json`.
- **Documented boundaries:** negative `is` narrowing, indirect mutation of a
  narrowed captured variable, generic classes/protocols, grouping syntax and
  no-return stubs are kept in limitations/design gaps, with controls where
  available. This avoids inflating defect counts with unsupported features.
- **Preservation:** `git diff --stat` remained empty during this review; additions
  are audit artifacts under the approved directory. No commit or product fix was
  made by this reviewer.

## Remaining review limits

This was a targeted review of two lanes, not a new exhaustive execution of all
fixtures or a review of every proposed future fix. No fix has been applied to
prove the inferred root causes. The lead owns final 58-row aggregation and final
cross-lane deduplication. Prototype fixes must retain source-coordinate and
binding identity controls, because a common wrong answer across editor and
checker surfaces can otherwise look like parity.
