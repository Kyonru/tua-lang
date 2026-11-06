# Completion, suggestions and edit safety audit

Revision: `d7aed2e5f1297b80a948e055985375bceaff6f0c`. Investigation only; no product code changed. This lane read the LSP parity, semantic resolver, Lua defaults, LOVE DevEx and test-audit skills. No applicable AGENTS.md was found by the lead.

## Summary

**Seven confirmed findings: three High and four Medium.** One additional client-filtering hypothesis and two bounded usability/recovery observations are recorded separately and excluded from defect counts. Expression-hover provenance in the class-chain reproduction is owned by the hover lane. A bounded follow-up established a distinct whitespace-sensitive class instance defect (CMP-07), while direct constructor-chain resolution remains verified symptom U-01 with root unresolved; see [class follow-up](CLASS-FOLLOWUP.md).

The broad semantic completion baseline works well: scoped locals and parameters, shadowed libraries, nested table fields, arrays, call results, nested call/index/field chains, common fields of unions, module exports from both source languages, standard-library recovery, LOVE object methods, literal enums, asset paths, expected-table keys and multiline callback scope all produced useful results. The important failures are at **routing and edit construction boundaries**: normal invoked requests skip autoimports, suggestions can insert invalid source, and a standard suppression action can reach a compiler panic.

This lane executed **45 completion scenarios in three capability/context modes** (135 requests plus selected-item resolve requests), eight diagnostic/code-action scenarios, and applied returned edits in test buffers. It also ran CLI checks of eight minimized resulting sources. Sources, exact cursor coordinates, selected raw items, resolved items, complete candidate labels and outcomes are saved beside this report. These are protocol observations, not human usability studies. The lead independently exercised real VS Code for the autoimport case.

## Reproduce the lane

From the repository root, with the current release binary built:

```sh
cargo build --release -p tua-cli
node internal/audits/devex-2026-11-06/completion/probe.js
TUA_AUDIT_TRIGGER=omit TUA_AUDIT_OUT=internal/audits/devex-2026-11-06/completion/context-omitted.json node internal/audits/devex-2026-11-06/completion/probe.js
TUA_AUDIT_SNIPPETS=false TUA_AUDIT_OUT=internal/audits/devex-2026-11-06/completion/no-snippet-support.json node internal/audits/devex-2026-11-06/completion/probe.js
node internal/audits/devex-2026-11-06/completion/actions.js
node internal/audits/devex-2026-11-06/completion/apply-edits.js
node internal/audits/devex-2026-11-06/completion/class-chain.js
```

The first script creates its disposable fixture under `target/devex-audit-2026-11-06/completion/project`; run it before `actions.js`. `apply-edits.js` reads the default `evidence.json` and uses a separate `applied` fixture. It expands the limited snippet constructs observed in the returned items (first choice/default and tab stops), then applies the exact text edits and additional edits in descending source order. It does **not** emulate all VS Code snippet semantics. The resulting malformed paths and quoted keys do not depend on snippet expansion.

`probe.js`, `actions.js`, and `apply-edits.js` accept `TUA_BIN=/absolute/path/to/tua`. The shared-harness class cross-check accepts `TUA_AUDIT_BINARY`. All scripts default to the repository's `target/release/tua`. No source fixtures outside audit scratch paths are changed. No network service is involved.

Standard client initialization for completion advertises `snippetSupport: true` and resolve properties `documentation`, `detail`, `additionalTextEdits`. The default completion requests carry `context: {triggerKind: 1}`. The omitted-context run removes that object entirely. The no-snippet run sets `snippetSupport: false` and retains ordinary invoked requests. Every source uses a single `<cursor>` marker removed before opening; stored `position` is computed from the exact preceding text using JavaScript UTF-16 columns. ASCII-only reproductions here isolate the defects from Unicode handling.

## CMP-01 — Invoked identifier completion bypasses project autoimports

- **Category:** correctness / LSP routing and cross-surface consistency.
- **Severity:** High. **Confidence:** high. **Status:** confirmed protocol; independently confirmed by lead with different module names and in actual VS Code.
- **Impact:** invoked requests, including the verified VS Code Ctrl+Space/provider path, cannot discover exported project functions or types. Automatic typing requests that use a trigger-character context may take a different route; this audit does not claim every automatic request is missing candidates. The documented automatic import workflow appears absent even though missing-type quick fixes and context-less test requests can find the same exports.

Minimal project:

```tua
-- src/exports.tua
export type ExportedPlayer = {health:number}
local function makeExported(): ExportedPlayer return {health=1} end
return {makeExported=makeExported}
```

```tua
-- src/main.tua, value case
local p = makeExpor<cursor>
```

```tua
-- src/main.tua, type case
local p: Expor<cursor> = nil
```

Expected: `makeExported` or `ExportedPlayer`, with the corresponding import edit on resolve, for ordinary invoked completion. Basis: `docs/lsp.md:151–179` and existing LSP autoimport tests.

Actual: `evidence.json` cases `autoimport-value` and `autoimport-type` have `labels: []`. The same source/position with no context has both expected candidates in `context-omitted.json`; resolve supplies `local Exports = import("./exports")` or `import type { ExportedPlayer } from "./exports"`. `actions-evidence.json` case `unknown-type` also successfully offers and applies the type import quick fix, establishing that the project export is indexed. Lead evidence in `../workflow/crosscheck-evidence.json` / editor artifacts provides independent reproduction (see lead verification log for exact filenames).

Observed code path: `crates/tua-lsp/src/server/handlers.rs:1414–1427` early-returns invoked identifier completions from `fast_identifier_completion_for_uri`. `analysis.rs:284–301` calls `core.current_file_completion`; it does not augment results. Project imports are appended only by `analysis.rs:1269–1286` and subsequent autoimport helpers. This is a proven routing divergence, not a missing export inference hypothesis.

Coverage gap: `crates/tua-lsp/src/server/tests/completion.rs:1019` and `:1062` test autoimports with `completion_request`, whose implementation at `tests/mod.rs:413` omits context. Explicitly invoked tests use a separate helper at `:427`, so the ordinary client route is not covered by these autoimport regressions.

Recommended direction: retain the cached current-file completion fast path, then merge project import candidates from a revisioned shared snapshot. Avoid copying the import algorithm into a second fast-path implementation. Candidate resolution and request context must not change availability.

Regression: table-driven identical sources across omitted context, `triggerKind:1`, and incomplete-completion retrigger; assert candidate identity and apply resolved imports. Include a VS Code Ctrl+Space/provider regression. Test both type and value exports, existing named/namespace imports, and no duplicate import after resolution. Approximate effort: **M**, dependent on preserving the measured latency budget.

## CMP-02 — An ordinary comment after a typed function signature panics; suppression can insert it

- **Category:** correctness / crash / diagnostic and code-action workflow.
- **Severity:** High. **Confidence:** high. **Status:** confirmed CLI and LSP; independently verified by hover agent with passing control.
- **Impact:** harmless documentation placement makes `tua check` exit with a panic and causes pull diagnostics to fail with an internal error. A suggested suppression action can introduce the failure into previously analyzable code.

Minimal standalone source:

```tua
local function f(x: number) -- hello
 return x
end
print(f(1))
```

Expected: valid commented typed Lua source; normal diagnostics and successful check. Basis: ordinary Lua comments are supported and type erasure must preserve comments. Moving the comment to its own next line is a passing control (independent evidence in `../hover/comment-crosscheck.json`).

Actual: `applied-evidence.json`, case `trailing-comment`, records `tua check` exit 1 and:

```text
thread 'tua-main' panicked at crates/tua-core/src/emit/mod.rs:1656:10:
existing syntax token should remain a valid symbol: UnexpectedToken('-')
```

Editor path: open `function love.udpate(dt: number)\nend\n`, request diagnostics and code actions. The server offers `Suppress TL5006 for this line`, inserting ` -- tua-ignore-line TL5006` immediately after `)`. Apply it and request `textDocument/diagnostic`: error `-32603`, `language server request worker panicked`. `actions-evidence.json`, `callback-typo` → matching outcome, and `actions-stderr.log` capture it. The rename action and file-level suppression both work. The process remains alive and subsequent action scenarios succeed: **this is not evidence of total LSP process death**.

Likely root cause, closely supported by the panic: `emit/mod.rs:1638–1656` obtains `close.to_string()` and recreates a symbol with `TokenReference::symbol(format!("{}{trailing}", token.to_string().trim()))`. The string includes comment trivia, so a symbol-only constructor rejects `-`. `code_action/mod.rs:324` correctly exposes the broader emitter failure through ordinary same-line suppression; fixing only that quick fix would leave handwritten comments broken.

Coverage gap: annotation-erasure tests at `emit/tests.rs:96` and `:183` cover typed functions, and suppression tests at `server/tests/code_actions.rs:4` cover suppression edits, but neither composes a typed signature closing token with trailing comment trivia and then rechecks the resulting source.

Recommended direction: preserve the existing token's syntactic identity and trivia using token APIs; avoid reparsing arbitrary rendered trivia as a symbol. Keep suppression actions routed through the standard emitter/analysis. Do not convert a valid-source panic into a silent omission.

Regression: CLI/check and LSP diagnostics for named/anonymous typed functions with single-line and block comments after `)`, both with and without return annotations. Add action-application regression for signature-line suppression and verify recovery on the next edit. Approximate effort: **S–M**, priority before UI expansion.

## CMP-03 — Completion edit planning ignores text to the right of the cursor

- **Category:** correctness / insertion and replacement / active editing.
- **Severity:** Medium. **Confidence:** high. **Status:** confirmed raw edits and applied-source CLI checks.
- **Impact:** correcting an existing name/path can append old suffix text or a second argument list. Plain identifiers in the middle of a token yield no candidates at all, while member/path contexts produce candidates with destructive or misleading edits.

Three related boundary reproductions (`<cursor>` marks the request):

```tua
local playerName = 1
local x = play<cursor>Wrong
```

Actual: zero candidates (`middle-identifier`), although end-of-prefix `play<cursor>` returns `playerName`.

```tua
local m = require("lib<cursor>ZZZ")
local img = love.graphics.newImage("assets/pl<cursor>ZZZ.png")
```

Selecting `library` / `assets/player.png` changes them to `libraryZZZ` / `assets/player.pngZZZ.png`. Source checks produce missing-module warning / missing-image error, respectively. The completion item's range ends exactly at the cursor, not at the end of the path token. The fixture includes the intended valid module and valid PNG.

```tua
local img=love.graphics.newImage("assets/player.png")
img.setFilter<cursor>("nearest", "nearest")
```

Selecting `setFilter` and accepting its default placeholder produces `img:setFilter("linear")("nearest", "nearest")`, retaining the old arguments after a newly inserted call. The middle-name variant `img.setF<cursor>ilter(...)` additionally leaves `ilter`. The catalog documents `setFilter` as returning no value, so calling its result cannot be a correct completion. The latter variant produces `TL4004` for `ilter`; the end-name variant is accepted by `tua check`, a related unchecked call-result boundary, **not separately claimed as a fully investigated checker finding**. Runtime LOVE execution of this malformed chain was not performed.

Expected: a replacement edit should replace the relevant current token and preserve existing argument lists, or offer an explicit insert/replace choice that the client can honor. It should not promise a module/asset/method candidate and insert a different identifier/path. Basis: normal editor replacement semantics; `docs/lsp.md` promises useful completion during active/incomplete editing.

Evidence: `evidence.json` cases `middle-identifier`, `module-middle`, `asset-middle`, `love-existing-open`, `love-existing-call`; `applied-evidence.json` corresponding resulting sources and check diagnostics. LOVE dot-to-colon correction itself works after resolve and is not the bug.

Root-cause evidence: `completion/context.rs:372–399` explicitly refuses an identifier when another identifier character follows the cursor. `completion/mod.rs:92–106` constructs replacement spans from the prefix only and always ends at the cursor. `completion/mod.rs:2639` and `:2812–2840` construct function-call snippets without right-context information. `server/protocol.rs:732–737` serializes the prefix-only span. These are distinct code sites sharing the absent syntactic edit plan; a fix should address all rather than patching one string path.

Coverage gap: `completion/tests.rs:25` and `server/tests/completion.rs:82` explicitly assert ranges ending at the cursor and use end-of-token fixtures. Method-snippet tests at core `:2035` verify generated placeholders/colon edits, not insertion before an existing call. No assertion applies these edits to right-suffix contexts.

Recommended direction: introduce a shared context-aware completion edit description with token start/end, insertion/replacement policy, current quote, and whether an argument list follows. Keep display label, filter prefix and insertion source separate. Use it for globals, members, modules/assets and callable snippets. Preserve cursor placement and combine additional colon/import edits without overlaps.

Regression: apply edits in middle/end-of-token and existing-call cases, reparse/check the result, and assert the selected symbol/path remains exact. Test empty prefixes and mixed `.lua` files. Approximate effort: **M**, prerequisite for CMP-04/CMP-05 improvements where reusable context is useful.

## CMP-04 — Asset path completion does not escape the active string delimiter

- **Category:** correctness / asset suggestions / escaping.
- **Severity:** Medium. **Confidence:** high. **Status:** confirmed protocol plus applied-source parser rejection.
- **Impact:** an otherwise valid project filename containing an apostrophe produces invalid source when selected inside a single-quoted loader call.

Fixture includes `src/assets/quote'image.png` (copied from a valid bundled PNG). Source:

```tua
local img=love.graphics.newImage('assets/quo<cursor>')
```

Expected insertion: a valid string representing the filename, e.g. `assets/quote\'image.png` within the current quotes. Actual `insertText` and `textEdit.newText`: literal `assets/quote'image.png`. Applied source becomes `newImage('assets/quote'image.png')`; `tua check` returns `TL0001` expected `)` and unclosed string. Evidence: `evidence.json` case `asset-apostrophe`; `applied-evidence.json` same case.

Likely root cause: `analysis/project_queries.rs:377–444` maps the raw asset path directly to `insert_text: Some(candidate.path)`; completion context exposes a prefix but no encoding-aware insertion plan. The protocol layer passes it through unchanged. This is not a path-resolution failure: the valid file is discovered and proposed.

Coverage gap: existing asset path edit tests at `server/tests/completion.rs:28` and `analysis/tests.rs:1929` use simple names inside double quotes. They check candidate existence and ranges, not parsed insertion with quote/backslash characters.

Recommended direction: encode insertion for the actual Lua string form using shared string/token utilities. Keep the candidate label/path metadata unescaped for display and resolution. Extend to other string-valued completions after confirming equivalent routes rather than adding a LOVE-only escaping rule.

Regression: apply suggestions for filenames containing single quote, double quote and backslash in both quoted forms; verify decoded Lua string equals the original path and the source parses. Long-string syntax is a separate supported-context decision. Approximate effort: **S–M**, naturally follows the context/edit-plan work.

## CMP-05 — Non-identifier record keys are offered as invalid dot accesses

- **Category:** correctness / member completion.
- **Severity:** Medium. **Confidence:** high. **Status:** confirmed protocol plus applied-source parser rejection.
- **Impact:** completion invites an invalid expression for legitimate record keys that require bracket syntax.

```tua
local t={["foo-bar"]=1,["end"]=2,normal=3}
local v=t.<cursor>
```

Expected: omit keys illegal in dot syntax or insert a properly escaped bracket access (`t["end"]`). Actual labels `end`, `foo-bar`, `normal`; the first two have no insertion rewrite or additional edits, even after resolve. Selecting `end` produces `t.end` and `TL0001: expected identifier after '.'`. `foo-bar` would also fail to denote that key (`t.foo - bar` rather than `t["foo-bar"]`); only the `end` applied-source parser failure is used as the decisive check.

Evidence: `evidence.json` case `quoted-member` (raw/resolved items); `applied-evidence.json` same case. The normal key is a passing control.

Root-cause evidence: `completion/mod.rs:1860–1892` collects table fields and emits their raw names as labels with `insert_text: None` and no additional edits. It does not check keyword/identifier syntax. `protocol.rs:700–703` uses the label when no explicit insertion exists.

Coverage gap: ordinary table/nested field completion tests (`completion/tests.rs:997`, `:1277`) prove type-driven members but not quoted keys/keywords. This is a syntax correctness boundary, not a need to infer dynamic keys.

Recommended direction: use existing identifier/keyword classification when constructing member candidates; ideally generate a shared bracket-access edit that replaces the separator plus token and escapes the string. Avoid silently changing a data key's spelling. Approximate effort: **S–M**.

Regression: indexed literal keys that are keywords, contain punctuation/quotes, and ordinary identifiers. Apply each insertion and compare its accessed key to the original. Include partial suffix replacement and colon contexts (non-callable data fields should not be presented as callable methods without qualification).

## CMP-06 — Server sends snippets even when the client explicitly declines them

- **Category:** protocol correctness / non-VS Code client compatibility.
- **Severity:** Medium. **Confidence:** high. **Status:** confirmed capability negotiation/response mismatch; no third-party editor UI exercised.
- **Impact:** a conforming client without snippet support receives unsupported placeholder syntax instead of usable plain text. VS Code itself supports snippets, so this finding must not be described as a VS Code-specific typing failure.

Initialize with:

```json
{"capabilities":{"textDocument":{"completion":{"completionItem":{"snippetSupport":false}}}}}
```

At empty source request completion, or request members of `love.graphics`. Actual response still contains `insertTextFormat: 2`, such as `function ${1:name}(${2})\n  ${0}\nend`, or signature-generated LOVE call snippets. Evidence: `no-snippet-support.json` records false capability and identical snippet-bearing items (`snippet`, `love-global`, `love-colon`). The supporting capability was explicitly false, not merely omitted.

Expected: honor the client's capability, either by returning plain labels/plain insertion or withholding snippet-only candidates. Basis: the client/server snippet capability contract; the server already negotiates other completion capabilities.

Root-cause evidence: initialization stores completion label-detail capability (`handlers.rs:331`, helper `:3087–3096`), but repository search finds no snippetSupport reader. `protocol.rs:697–746` unconditionally sets format 2 whenever the core item is a snippet; the serializer accepts label-detail support but no snippet-support flag.

Coverage gap: tests assert snippet format/contents in positive cases (`server/tests/completion.rs:707`) and separately cover label-detail negotiation; there is no false/absent snippet-capability matrix. Current harnesses commonly omit capabilities yet assert snippets, reinforcing the blind spot.

Recommended direction: negotiate and store client snippet support at the LSP boundary. Keep core snippets reusable; choose a documented fallback at serialization/resolution and apply it consistently to `textEdit.newText` as well as `insertText`. Approximate effort: **S**.

Regression: initialize with true/false/absent support and inspect both initial and resolved items. No `${...}`/`$0` or format 2 in unsupported-client edits; snippets unchanged for VS Code capability. Include methods with choice placeholders.

## CMP-07 — Whitespace-sensitive native class instance inference

See [CLASS-FOLLOWUP.md](CLASS-FOLLOWUP.md) for ten body/init controls, the exact
`local counter=Counter.new(1)` versus `local counter = Counter.new(1)` reproduction,
shared source-line matching root, recommended regression matrix and direct-chain
verified symptom U-01. This is High severity, confirmed with independent lead
cross-check requested.

## Observations, limits and non-findings

1. **Unverified client filtering hypothesis:** source `fun<cursor>` returns the `function` snippet with `filterText: "fn"`. The source of this alias is `completion/mod.rs:942`. A client that filters solely against `filterText` may hide it for `fun`. The raw server mismatch is recorded in `evidence.json` → `function-prefix`, but actual menu behavior was not established by this lane. Do not count it as a confirmed user-visible defect without the lead's editor result.
2. **Inline callback recovery limitation:** `f(function(player) player.<cursor> end)` has no completion, while the multiline counterpart and a valid `print(player.<cursor>health)` return `health`. This is a narrower unsupported edit layout; no general callback inference failure is claimed. Recovery code is line-oriented and only erases unfinished access before selected punctuation (`analysis/recovery.rs:182–228`). Existing `incomplete_event_payload_member_keeps_callback_scope` tests use `print(payload.)` / concatenation rather than a same-line bare statement followed by `end`.
3. **Suggestion relevance:** `t:<cursor>` for `{health=1,run=function(self) end}` includes the non-callable `health` field as well as `run`. Record as a usability improvement candidate, not proof that callable field semantics are broken. All 45 recorded scenarios have no duplicate labels. Nearby local `playerScore` ranks ahead of farther `playerName`; `sortText` preserves that order.
4. **Resolve works for LOVE methods:** initial payloads intentionally omit eager docs/additional edits. Resolve adds method docs, LOVE wiki links, exact signatures and the dot-to-colon edit. Absence in the initial response alone is **not** a finding.
5. **Scoped behavior is sound in tested controls:** hidden block locals are absent, local `math` shadows library members, union completion exposes only common fields, and `any`/void-result receivers do not invent members. Chained `make()[1].stats.` and direct LOVE constructors provide members. Malformed unrelated parentheses preserve standard-library completion; unclosed multiline class bodies retain `self` members.
6. **Code actions mostly apply cleanly:** LOVE dot-to-colon, callback rename, unused-local removal, type annotation, inline local, duplicate-key rename, type autoimport, and tested suppressions remove the targeted diagnostic. Missing-module creation was inspected as a structured `CreateFile` plus edit operation, but the script deliberately does not apply resource operations; it is an inspected-only subcase, not a tested successful creation.
7. **Class receiver issue cross-check:** `class-chain.js` independently corroborates the hover lane's Counter/init fixture: completion is empty, signature null and hover range/types wrong. The class default/init warning itself is expected. Controls from the hover lane establish that ordinary class instances without the problematic explicit init pattern work; do not generalize to all classes. The false expression type/range belongs to HOV-03; the local whitespace dependency is now independently established as CMP-07, and direct constructor-chain completion/signature remains U-01.
8. **Client configuration:** VS Code defaults enable quick suggestions in strings/other code but disable them in comments (`editors/vscode/package.json:72–82`). Manual annotation completion returning `param` is verified; absence of automatic comment suggestions is configuration, not server failure. Local model/inline completion features are opt-in and were not exercised here. Actual client UI integration and Unicode belong to the lead workflow lane.

## Suggested implementation order

1. **CMP-02: preserve function-token trivia** (S–M). Eliminate panic in valid handwritten source and quickfix outcomes; test CLI and diagnostic recovery.
2. **CMP-01: unify current-file fast-path augmentation** (M). Restore autoimports without sacrificing latency. Add actual client context coverage before further optimizations.
3. **CMP-03: shared completion edit plan** (M). Resolve prefix/suffix and existing-call behavior together. Make applied-source tests the oracle.
4. **CMP-04 + CMP-05: syntax-aware literal/member insertion** (S–M each, reusing step 3). Keep display labels independent of valid source insertion.
5. **CMP-06: client capability fallback** (S). Small protocol adaptation, separately test resolved edits.
6. **CMP-07: native constructor/binding semantic resolution** (M). Eliminate source-line whitespace dependence; retain direct-chain U-01 as an explicit regression target with root still unresolved. Coordinate with declaration/provenance work.
7. Broaden recovery/client usability after these correctness boundaries: inline callback layout, callable-only colon relevance, and verified snippet filtering. No language syntax expansion is required for these fixes.

Effort is relative: S = a focused patch plus targeted matrix; M = shared-query/edit-plan changes with cross-surface validation. These are not calendar commitments.
