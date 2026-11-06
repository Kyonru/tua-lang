# Prioritized improvement backlog

Investigation only: no item below was implemented. Estimates are engineering
ranges for implementation plus meaningful regression coverage, not commitments:
**S** roughly 1–2 days; **M** 3–5 days; **L** 1–2 weeks. Unknown interactions can
change these substantially. Preserve the current binary/probes as a baseline.

## Correctness fixes — recommended sequence

| Order / ID | Underlying cause and findings | Direction / dependency | Effort | Acceptance criteria |
|---|---|---|---|---|
| 1 / B01 | LSP encoding boundary — **WF-01** | Document-aware UTF-16 ↔ core/source conversion for every standard query/result/edit; reuse existing incremental conversion. Independent of core native-parser provenance. | M–L | ASCII/BMP/non-BMP/CRLF fixtures; apply rename and completion edits exactly; no wrong-token edits; all ranges within original text; real VS Code regression. |
| 2 / B02 | Token trivia reconstruction — **CMP-02** | Preserve syntactic tokens and comments through lowering rather than feeding rendered comment text to a symbol parser. Can proceed independently of B01. | S–M | Valid typed headers with inline/block comments check/build normally; offered suppression applied to a header produces valid source and diagnostics; worker remains responsive after malformed inputs. |
| 3 / B03 | Project ownership missing from require resolution — **WF-02** | Pass containing workspace to module graph/search/cache identities. Retain intentional import/declaration-library mechanisms. | M | Same-name incompatible modules in two roots remain independent across open/root order, Lua/Tua, unsaved updates and deletion; checker/hover/completion/definition agree. |
| 4 / B04 | Callable identity uses names instead of bindings — **HOV-02** | Resolve scope/HIR identity before obtaining callable metadata; do not replace first-match with last-match. Foundation for contextual validation and member identity. | M–L | Nested/sibling/recursive same-name functions preserve each contract in checker, return inference, hover, resolve and signatures; parameter shadows remain correct. |
| 5 / B05 | Native instance types recovered by source-text spelling — **CMP-07**, **U-01** | Replace `local name = ` scanning with declaration/expression-backed native constructor facts. Reuse B04 identity and existing constructor metadata; test provenance explicitly. U-01 attribution must be resolved while doing this, not assumed fixed. | M | Compact/spaced/multiline/equivalent split/direct constructor forms yield the same receiver type, members and signature. Preserve checker rejection control. Direct chain has its own assertion, independent of hover-result spans. |
| 6 / B06 | Contextual checking and callable metadata differ by path — **TYP-01**, **TYP-03**, **HOV-04** | One expected-callable/body-checking path for direct/aliased/imported callbacks and annotated initializers; retain vararg and optional/default metadata. Avoid context-free body-cache reuse. Metadata preservation can be a smaller independent slice. | M–L | Invalid callback returns and native vararg tails rejected; correct callbacks accepted; hover/signatures distinguish optional/default from required-nilable parameters; all routes agree. |
| 7 / B07 | Separate expression suffix walkers — **TYP-02** | Share call/field/index/method result traversal between checker and editor, keeping validation hooks. Coordinate with B05, not another factory special case. | M–L | Chained and split local/imported/re-exported factory reads get the same checked type; wrong assignments fail; unknown dynamic Lua remains permissive. |
| 8 / B08 | Fast completion bypasses semantic augmentation — **CMP-01** | Augment cached current-file results from revisioned project import facts; use the existing autoimport resolver. Record semantic parity before timing. | M | Identical candidate/import edits for omitted/invoked/retrigger contexts; actual VS Code provider/menu offers exported value/type; apply import once; preserve measured latency without dropping candidates. |
| 9 / B09 | Prefix-only completion edits and raw insertion strings — **CMP-03**, **CMP-04**, **CMP-05** | Shared syntactic edit plan: complete token range, quote form, separator, existing argument list, insert vs replace. Display labels remain separate from valid source text. | M–L | Applied edits select exact intended module/asset/key; replace old suffix; no second argument list; quote/backslash filenames round-trip; keyword/punctuation keys use valid access; no overlapping additional edits. |
| 10 / B10 | Lua value lists conflate syntax count, missing and unknown — **TYP-04**, **TYP-05** | Normalize final-call/vararg expansion and scalar/nonfinal/parenthesized cases once, preserving slot provenance. Use for arguments, assignments and returns. | M–L | Final multi-return calls accepted with correct slot checks; definitely absent values infer nil; dynamic unknown remains any; Lua/LuaJIT runtime oracle and exact diagnostic tests. |
| 11 / B11 | Source/parser provenance omitted in expression fallback — **HOV-03** | Translate cursors into normalized AST space and map only source-backed results out; reject generated-only spans. Different layer from B01, but share coordinate tests. | M | Arithmetic/literal/member hover before/inside/after native classes has correct type and bounded original-source range; class-prefix invariance; generated boolean[] never leaks into unrelated user expression hover. |
| 12 / B12 | Method identity is lost after call-result receivers — **HOV-01** | Expose resolved member identity + original member span and docs through SemanticModel; reuse for hover/signature/definition. Depends conceptually on B05/B07 rather than LOVE-only patch. | M | Void and nonvoid LOVE/local/imported chains show method signatures/docs on method token; correct return-result hover elsewhere; void `Text:set` cannot fabricate a next receiver. |
| 13 / B13 | Client snippet capability ignored — **CMP-06** | Negotiate at LSP boundary; serialize plain-text fallback or omit snippet-only entries, consistently on resolve and text edits. | S | True/false/absent capability matrix; unsupported client receives no snippet placeholders or format 2; VS Code retains snippets/choices. |

Do not require a wholesale architecture rewrite before addressing source corruption
or the valid-source panic. Each row can be split into behavior-sized changes with
cross-surface controls. Conversely, a formatter workaround, missing annotation,
or per-LOVE-method patch is not acceptance for a shared semantic defect.

## Missing capabilities and usability — separate from confirmed bugs

| ID / category | Evidence and current boundary | Suggested next step / acceptance | Depends on / effort |
|---|---|---|---|
| G01 / capability | Negative `is` branches do not subtract a known type; positive narrowing is implemented. Typing report boundary 1. | Specify complement behavior, then test unions, early returns, nil and unknown types. Do not claim current positive-only contract is violated. | Shared narrowing; M |
| G02 / syntax usability/design | Nullable inline callable grouping is awkward; named callable aliases work. Typing boundary 2. | Decide intended parenthesized type grammar; ensure function/tuple/grouping ambiguity is deliberate, then document and test. | Grammar decision; M |
| G03 / documented effect-analysis limit | Indirect closure mutation can invalidate guarded locals beyond current analysis. Typing boundary 3. | Keep documented safe-copy guidance; evaluate a bounded invalidation/effect model without pretending arbitrary dynamic calls can be proven. | Semantic design; L |
| G04 / capability | Active overload chooses by arity, not argument type; alternatives remain visible and docs describe it. HOV-G01. | Reuse checker applicability for same-arity ranking, conservative incomplete/unknown fallback. | B04/B06; M |
| G05 / documentation usability | Native class method prose reaches hover, not signature help. HOV-G02. | Carry resolved documentation identity into signatures without reparsing comments in extension. | B12; S–M |
| G06 / annotation authoring | Incomplete tag hover absent; valid tags work but repeat syntax fences. HOV-G03; known roadmap item. | Prefix-aware tag explanation and one syntax block; no invented semantics for unknown tags. | Existing tag catalog; S–M |
| G07 / incomplete-edit recovery | Same-line bare callback member access has no result while multiline/contextual variants work. Completion observation 2. | Extend bounded recovery preserving callback scope; compare same source across newline layouts. | Recovery/shared parsing; M |
| G08 / suggestion relevance | Colon completion also offers non-callable data fields. Completion observation 3. | Decide whether to filter or clearly rank callable fields while preserving dynamic tables. | B09; S |
| G09 / unverified hypothesis | `fun` prefix with snippet `filterText="fn"` may hide a function snippet. No actual menu reproduction for this specific case. | Test real client filtering before labeling a defect or changing aliases. | Editor experiment; S |
| G10 / explicit design boundary | Generic classes, protocols/interfaces, abstract machinery are outside current supported contract. | Gather concrete cases and design semantics; no “fix missing generics” ticket inferred from this audit. | Separate language design; not estimated |

No-return unannotated stub inference is deliberately conservative and is not
TYP-05. The capability notes preserve that distinction rather than demanding
strictness for all dynamic Lua.

## Performance and qualification work

| ID | Evidence / recommendation | Acceptance | Effort |
|---|---|---|---|
| P01 | Warm protocol requests are fast in bundled projects; cold first import completion is ~37–118 ms median, post-edit ~4–14 ms in this run. This is not a before/after regression comparison. | Preserve exact semantic results while profiling cold/project-surface cost. Repeat fixed revision, workload, sample count and context/capability matrix; include import candidates. | M per measured optimization |
| P02 | Actual provider requests and suggestion UI were exercised, but not a key-event-to-render benchmark. | Add deterministic VS Code keystroke/menu timing with validated expected items, separate from server times; include file switches and unsaved imports. | M |
| P03 | No hours-long sessions, cross-platform editor parity or third-party UI qualification. | Replay realistic edits over longer sessions; track memory/crashes/stale state; execute UTF-16 and applied-edit matrix on supported native CI and at least one nonsnippet client. | M–L / native resources |
| P04 | Existing trusted suite passes but misses the audit's boundary cases. | Promote minimal reproducible fixtures into existing core/LSP/editor tests as fixes land; assert exact diagnostics/types and apply edits, not just nonempty responses. Do not treat conditional early returns as executed runtime coverage. | Included in each fix |

## Deduplication decisions

- Typed signature comments and the suppression action are **one CMP-02** emitter
  issue; no action-only workaround is proposed.
- Native vararg checker and editor symptoms are **one TYP-03** metadata issue.
- Missing assignment slots and final-argument expansion remain two observable
  findings but are one **B10** value-list implementation family.
- Source Unicode conversion (**WF-01**) and normalized native syntax provenance
  (**HOV-03**) are different coordinate layers; do not merge them into a vague
  “range issue.”
- Shared wrong function identity (**HOV-02**) is one finding even though several
  editor surfaces agree on the wrong answer.
- Whitespace-sensitive native receiver recovery (**CMP-07**) was isolated after
  disproving the initial explicit-init hypothesis. Direct constructor-chain loss
  (**U-01**) is retained with unresolved attribution, not silently attributed to
  whitespace or hover provenance and not counted as another unique root.
