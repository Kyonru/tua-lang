# Cross-feature reconciliation queue

- Typing agent: independently reproduce STORES-01 shortened inferred callback rejection and STORES-02 optional-field nil-write rejection. Decide shared root causes, retain store reproductions as affected workflows.
- Modules/editor agent: stores/editor-import-results.json reports any/no signatures for require("game").game; unconfirmed until fixture/root setup independently checked. Signals default-export bus navigation loses identity while payload signatures work (SIGNALS-03).
- Runtime domains: signals once reentrancy and state machine nested transitions share lifecycle design concerns but have distinct implementations; keep separate defects, group backlog under callback lifecycle contracts.
- Release qualification baseline: two nominal assignment failures and inline snapshot mismatch are known before this audit. Avoid re-counting as new findings.
- Lead cross-checks: signals once count=2 and state machine cancellation/async state loss independently rerun in cross-checks/first-domains.json. Store one-param callback independently reproduced in cross-checks/store-callback.json. Initial store CLI direct-file invocation in first-domains.json was a harness error superseded by isolated-project probe, not a product finding.
- Callback families: POOLS-03 duplicates STORES-01 (ignored extra callback args). POOLS-01 reset/dispose parameter Any and ECS system callback Any are domain contract propagation gaps, not automatically the same root as assignability.
- Editor agent: input literal-selected same-arity overload chooses wrong active signature (INPUT-G01). Audio agent reports unrelated strings/function-type fragments in play/stopBus/on string completion; see audio/lsp.json once available. Independently validate and consolidate shared candidate enumeration.
- Numeric option validation: INPUT-01 negative unary/hex deadzones bypass decimal-only parsing; inspect other domains before proposing shared numeric constant parsing.

- Functions/checker agent: sprites unknown_local rejects invalid animation name, equivalent `return S:player("absent")` builds then crashes; inspect call validation in returned expressions.
- Shared helper inclusion/lowering: vectors chained vec2().normalized leaves nil global; commented/static-bracket math helper references miss injection. Camera agent reports alias `local C=Camera; C.new()` emits no runtime. Consolidate architectural remedy around bound semantic references, while preserving distinct normalizer/inclusion failures.

- Reconciled: TYPING-03 owns STORES-01 and POOLS-03 shortened-callback rejection; TYPING-04 owns STORES-02 optional nil deletion. Keep feature symptoms as evidence, count each shared defect once. TYPING-02 is residual of prior TYP-05 and must be marked previously known family. Lead independently reran its emitted nil-concatenation failure in cross-checks/missing-assignment-slot.json.
- High findings independently rerun so far: AUDIO-01 (cross-checks/audio-runtime.txt); sprite zero-duration hang (actual LOVE with bounded hook, cross-checks/sprite-zero-duration.log); TYPING-01 dictionary write (cross-checks/dictionary-assignment.json); TYPING-02 missing RHS (cross-checks/missing-assignment-slot.json).

- CLASS-01 High independently confirmed by lead with passing self-control: cross-checks/class-self.json. Constructor flattening drops direct self value while captured local forces safe path; preserve documented initialization ordering in suggested fix.

- Modules cross-check completed: MODULES-01 confirms inline require property type loss (store + ordinary function); MODULES-02 confirms imported ECS schema loss, both single-root. MODULES-03 adds declaration-overlay disagreement among surfaces. Exclude earlier unconfirmed root setup hypotheses; refer to modules fixtures/controls.

- Asset pipeline ownership root change overwrites pre-existing file: independently confirmed fresh fixture by lead in cross-checks/asset-ownership.json. High-priority filesystem ownership blocker; no real user file involved.

- Formatter long-string opening line loses semantic trailing spaces; independent lead build/fmt/build runtime lengths 5->3 in cross-checks/formatter-string.json. High.
- Shader agent hover has nested tua/glsl Markdown fences in shaders/lsp.json; editor agent should reconcile formatting ownership.

- Editor reconciliation complete: EDITOR-01 owns generic colon string-completion receiver fragments (audio symptom); EDITOR-02 owns SHADERS-02 double-fenced Markdown. Count SHADERS-02 once under EDITOR-02. INPUT-G01 is prior HOV-G01 capability gap, not a new defect.

## Final reconciliation status

All 31 lanes and both independent reviews are complete. MODULES-01/02 close the initial store/ECS import observations. STORES-01 and POOLS-03 consolidate under TYPING-03; STORES-02 under TYPING-04; SHADERS-02 under EDITOR-02. AUDIO-01 is Medium; inherited parent-bus limits remain a design question. The earlier entries are investigation history, not a queue of unfinished agent work. Remaining capability and qualification work is listed in BACKLOG.md and VERIFICATION.md.
