# Editor/LSP audit

Candidate 0.6.0 / baseline 9d490dc. Actual stdio requests confirm **one Medium and one Low defect**, plus one previously known feature gap. Product files were not modified. The shared 389-test LSP baseline was not rerun.

## Reproduce

From repository root:

```sh
node internal/audits/features-2026-11-07/editor/probe.js
node internal/audits/features-2026-11-07/editor/verify.js
```

The probe starts absolute `target/release/tua lsp`, creates isolated fixtures under `editor/project`, records exact requests and raw results in `responses.json`, and sends an incremental unsaved edit. `verification.log` records passing assertions. An initial logging-only TypeError after responses were saved was corrected and the complete probe rerun successfully; no product finding came from it.

## EDITOR-01 — Colon string completion enumerates receiver internals

**Confirmed defect · Medium · new shared-root finding, consolidates audio/lsp.json.**

Minimal generic reproduction (`project/src/method.tua`):

```tua
type Receiver = {kind: "unrelated" | "leaked", call: (self: Receiver, mode: "yes" | "no") -> nil}
local r: Receiver = {kind="unrelated", call=function(self: Receiver, mode: "yes" | "no") end}
r:call("")
```

Request completion at zero-based line 2, UTF-16 character 8 (inside quotes). Expected `yes`, `no`; actual sole label is `no") -> nil, kind: "unrelated`. Ordinary free-function literal union completion returns precisely yes/no in the control. This independently proves the audio symptom does not depend on audio assets or static cue configuration.

Audio play/stopBus/on requests likewise return event names, duplicates, `smoothstep`, and a fragment containing function signatures. Audio's expected cue/bus names are missing. Labels can become malformed source when accepted. Raw full results: `responses.json` labels `method`, `audio-cue`, `audio-bus`, `audio-event`, `literal`; original independent evidence: `../audio/lsp.json`.

Cause: `crates/tua-core/src/completion/mod.rs:2066` indexes the function parameters directly with visible active argument index, without accounting for colon self. At :call's first argument it gets Receiver, resolves the alias and stringifies the full record (`:2078`). `:2368` splits that formatted type on every pipe, and `:2404` accepts fragments based only on their opening/closing quote. Thus nested receiver fields/functions become apparent enum choices. This is a shared semantic completion issue, not separate per-audio-method defects.

Existing simple literal-union tests at `completion/tests.rs:2519` and `:2537` cover free functions; they do not exercise explicit self plus nested unions. Smallest maintainable improvement: select the effective argument type through a shared colon-aware call resolver, then enumerate actual Type literal/union nodes instead of parsing display strings. Regression: generic explicit-self method above, dot-call equivalent, audio on/play/stopBus, and ordinary free-function positive; assert exact candidate set and apply accepted insertion.

## EDITOR-02 — Shader hover nests fenced Markdown inside a Tua fence

**Confirmed defect · Low · new, consolidates shader lane observation.**

`project/src/shader.tua` loads `effect.glsl`; hover inside `fx:send("alpha", 1)` at line 1 character 11. Expected a GLSL code block followed by source location and explanatory prose. Actual begins ` ```tua\n```glsl\nextern number alpha;\n``` ` and appends a final extra fence. Under Markdown fence rules the GLSL opener is literal text inside the outer Tua block; the last fence opens a new block. This is protocol Markdown corruption, not a missing semantic uniform result. Exact raw value: `responses.json` / `shader-hover`; independent shader evidence: `../shaders/lsp.json`.

`crates/tua-core/src/analysis/shader_queries.rs:160` constructs complete fenced Markdown in Hover.contents; `crates/tua-lsp/src/server/protocol.rs:182` wraps every contents value in a Tua code fence. Establish explicit content format/language at the core-to-protocol boundary (or move shader prose into documentation and expose its declaration with a language tag). Do not guess format by text substring. Existing shader query coverage did not prevent this protocol composition defect; add an actual protocol hover assertion for balanced GLSL fences and prose outside them, alongside ordinary Tua hover. No VS Code rendering run was performed.

## EDITOR-G01 — Literal-aware active overload selection (known)

`responses.json` / `input-overload`: after complete `c:get("move"` signature 0 describes scalar down/left/right/up; signature 1 correctly describes move and two returns. `signature_help/mod.rs:1062` ranks only by arity. This reproduces INPUT-G01 and prior `internal/audits/devex-2026-11-06/hover/REPORT.md` HOV-G01; **do not count it as a new defect**. Reuse compiler overload applicability to rank proven literals, retaining conservative fallback for incomplete arguments. Add same-arity literal selection assertions, not merely label availability. Current roadmap includes overload-ranking follow-up; no duplicate new backlog item is needed.

## Coverage and limits

| Surface | Result |
|---|---|
| Typed local hover, signature parameter selection, inlay types/arguments | Tested, positive |
| Definition, references and rename | Tested; correct declaration and three occurrences |
| UTF-16 coordinates after same-line astral emoji | Tested hover, rename/reference ranges and decoded semantic token at 26..31 |
| Incremental unsaved edit | Tested; replacing call with string immediately changes hover number -> string and preserves rename identity |
| Document/workspace symbols and full semantic tokens | Tested representative function/local workflow |
| Incomplete `player.` member access | Tested; health/name returned |
| Free-function enum string completion | Tested; exact yes/no candidates |
| Generic colon method, audio completion, shader hover | Tested; defects above |
| Input literal-selected overload | Tested; known limitation |
| Configuration | Isolated project, lint off and shader validator off; baseline defaults otherwise |
| Completion resolve, token delta/range, stale concurrent burst results, cross-file rename, imported invalidation | Not exercised in this bounded pass; modules lane owns imported behavior |
| VS Code GUI, activation, latency/long sessions, call hierarchy, refactor application | Not exercised; no readiness claim |

No prior fixed issue was reopened without reproduction. Prior HOV-G01 is explicitly deduplicated; no matching generic colon enum corruption or shader fence fix was found in the inspected prior hover/completion reports or current roadmap. Prior nominal assignment and inline-eval failures are outside these results. Highest-value improvement is shared call-argument semantics and structural literal enumeration; shader formatting is a smaller protocol-contract fix.
