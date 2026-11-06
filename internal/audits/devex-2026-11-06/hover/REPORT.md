# Hover, signature help, and cross-surface consistency

Audit lane: H01–H11. Baseline `d7aed2e5f1297b80a948e055985375bceaff6f0c`.
This lane made no product changes and no commits. All observations below use
real stdio LSP requests to `target/release/tua`; focused cases also run `tua check`.
This is protocol evidence, not a claim that every VS Code rendering path was tested.

## Assessment

The straightforward paths are useful: annotated/inferred locals, typed fields,
LuaDoc functions imported from Lua, imported class aliases, direct class members,
Lua/LOVE catalog documentation, multi-return local inference, and nested-call
active parameter selection behaved correctly in the exercised cases. The weak
boundaries are declaration identity (same-name local functions), expression
hover across normalized native syntax, and preserving callable metadata as facts
travel into signature/hover formatting.

Four confirmed findings are owned here: one High and three Medium. Three
capability/usability gaps are separately identified and not counted as defects.
Native vararg metadata and final-argument multiple return defects belong to the
typing lane; UTF-16 range corruption belongs to workflow; header-comment panic
belongs to completion. Independent cross-checks for these are preserved here.

## Reproduction and evidence inventory

Run commands from the repository root. Node dependencies already used by the
repository suffice; the probes reuse `editors/vscode/scripts/lsp-profile-common.js`
for framing. Build `cargo build --release -p tua-cli` if the binary is absent.
Set `TUA_AUDIT_BINARY=/absolute/path/to/tua` to audit another server.

```sh
node internal/audits/devex-2026-11-06/hover/probe.js
node internal/audits/devex-2026-11-06/hover/focused.js
node internal/audits/devex-2026-11-06/hover/expression-provenance.js
node internal/audits/devex-2026-11-06/hover/class-variants.js
node internal/audits/devex-2026-11-06/hover/imports-and-docs.js
node internal/audits/devex-2026-11-06/hover/crosscheck-unicode.js
node internal/audits/devex-2026-11-06/hover/crosscheck-comment.js
```

- `probe.js` creates 15 fixture projects under `target/devex-audit-2026-11-06/hover/`.
  `evidence.json` preserves source, exact positions, hover/signature responses,
  inlay hints and received diagnostics. Selected case names can be passed as
  arguments; those runs write `selected-evidence.json` instead.
- `focused.js` creates and removes temporary projects. `focused-evidence.json`
  preserves sources, complete request parameters/responses and CLI outcomes for
  12 cases. CLI `check` and diagnostic requests disable lint rules to isolate
  semantic issues; this is not a claim that every program is lint-clean.
- Other scripts likewise preserve source and full relevant responses in named
  JSON files beside the script and remove temporary projects.
- Request positions are zero-based UTF-16. These probes avoid emoji except the
  explicit independent UTF-16 check. Captured temporary absolute paths are
  incidental; rerunning scripts creates new projects with the same sources.
- The scripts record observed responses rather than assuming all cases pass.
  Read the assertions/expected-vs-actual below when interpreting output.

## HOV-01 — Method hover on call-result chains loses the method identity

**Category:** editor correctness / semantic parity. **Severity:** Medium.
**Confidence:** High. **Status:** confirmed by real LSP, with a valid minimal
LOVE source and passing CLI checker. Root cause is code-path evidence, not a
fix experimentally applied during this audit.

### User impact

Hovering a method written directly after a constructor/factory call yields no
help, or merely the return type of the whole call. The same method has usable
signature help, completion and definition. This makes chaining materially worse
than introducing a local variable, even when the receiver is statically known.

### Minimal reproduction

```tua
love.graphics.newCanvas(20,20):setFilter("nearest", "linear")
```

Run `node internal/audits/devex-2026-11-06/hover/focused.js`, case `chain-love`.
Request `textDocument/hover` at `{line:0, character:34}` (inside `setFilter`).

**Expected:** the `Canvas:setFilter` signature and catalog documentation,
consistent with a direct local `canvas:setFilter` and with signature help.
Basis: `docs/lsp.md:108` promises completion, hover and signature help for typed
call results; `docs/lsp.md:186` promises LOVE documentation in hover.

**Actual:** hover is `null`. At `{line:0, character:41}`, signature help returns
`function Canvas:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)`
with description and all parameter documentation. Completion contains
`setFilter`; definition resolves the built-in method. CLI check exits 0 and
pull diagnostics contain no errors.

A return-valued control, `local width = love.graphics.newCanvas(20,20):getWidth()`,
returns hover `expression: number` over the **entire constructor/method call**,
not the hovered `getWidth` signature/docs. Local `width` correctly hovers as
number. Evidence: `focused-evidence.json`, `chain-love` and `chain-love-value`;
`evidence.json`, `love`.

### Likely root cause / test gap

- `crates/tua-core/src/hover/mod.rs:104` starts from a dotted-name chain; its
  fallback at `:335–344` is expression-result hover.
- `crates/tua-core/src/semantic.rs:1742–1778` traverses function-call argument
  expressions but does not emit a typed method-name candidate for call suffixes.
  Consequently a void call has no result hover; a value call exposes the whole
  expression result.
- Signature help already resolves expression receivers through shared semantics
  at `crates/tua-core/src/signature_help/mod.rs:167–183`.
- `hover/tests.rs:2076` checks **locals assigned from** chained LOVE calls, not
  the method token. `analysis/tests.rs:7414` checks a factory-return **field** and
  a separate method signature, leaving the method-hover/documentation hole.

### Smallest maintainable direction / regression

Expose a resolved member access at a source position through `SemanticModel`
(receiver type, member/catalog identity and original member span). Reuse it for
hover and signature/definition consumers rather than adding a LOVE-only hover
special case. Test void and value-returning constructor/factory chains, both dot
and colon access, local and imported native classes, correct method ranges/docs,
and the negative control `Text:set(...):getWidth()` (void really ends that chain).
This is independent of HOV-03: it reproduces without native class normalization.

## HOV-02 — Same-name local functions resolve to the outer signature

**Category:** language correctness / binding identity / cross-surface parity.
**Severity:** High. **Confidence:** High. **Status:** confirmed by CLI and LSP,
independently reproduced by the typing agent with a different function name and
body. This is not a restriction on generics or dynamic Lua.

### Minimal reproduction

```tua
local function use(value: number): number return value end
do
 local function use(value: string): string return value end
 use("inner")
end
use(1)
```

Run `node internal/audits/devex-2026-11-06/hover/focused.js`, case `shadow-signature`.
Inner hover position `{line:3, character:2}`; inner signature help position
`{line:3, character:5}`. The outer call is a control.

**Expected:** inner hover/signature uses string and the program type-checks;
outer call retains number. Basis: ordinary Lua lexical local-function shadowing,
which Tua retains (`docs/language.md` Names And Keywords / Lua compatibility),
and existing scope infrastructure/tests for innermost parameter bindings.

**Actual:** inner hover/signature both show `function use(value: number): number`.
CLI and LSP reject `"inner"` with TL1003 `expected number, found string`.
Outer call correctly remains number. The typing agent's distinct `convert`
fixture also observes wrong inner result inference and false return-type errors.

**Evidence:** `focused-evidence.json` case `shadow-signature`; independent
`../typing/results-shadowed.json`, case `shadowed-local-functions`.

### Root-cause evidence / existing gap

`TypeEnvironment::function` at `crates/tua-core/src/types/environment.rs:296–300`
returns the first function with a matching **name**, without a scope/declaration
identity or query position. Signature resolution checks that lookup before local
binding resolution (`signature_help/mod.rs:204–224`). Hover uses the same lookup
at `hover/mod.rs:328–335`. These paths therefore agree with each other while
agreeing on the wrong declaration. The checker symptom demonstrates that editor
parity alone is an insufficient oracle.

`hover/tests.rs:103` tests same-name **parameters** within differently named
functions. The inspected hover/signature tests do not exercise distinct local
functions with the same name in nested blocks. Reusing a name is valid and
common in helper callbacks and nested scopes.

### Recommended fix and regression

Index function contracts by shared scope/HIR declaration identity; resolve the
binding at the query/call position before retrieving its contract. Preserve
position-aware module/overload handling instead of changing first-match to
last-match. Add nested-block, sibling-block, recursive local function, parameter
shadow and source-order fixtures; check checker diagnostics, inferred returns,
hover, completion resolve and signature help for inner and outer uses.
Dependency: agree on one declaration-backed callable resolution path before
patching per-feature lookups. This is a priority correctness fix.

## HOV-03 — Expression hover uses normalized class coordinates as source coordinates

**Category:** source provenance / hover correctness. **Severity:** Medium.
**Confidence:** High observation; high-confidence inferred root cause.
**Status:** confirmed by LSP with prefix/no-prefix controls; related class-chain
completion issue separately cross-checked by completion agent.

### Reproduction

```tua
class Counter {
 value: number = 0
 read(): number { return self.value }
}
local result = 40 + 2
print(result)
```

Run `node internal/audits/devex-2026-11-06/hover/expression-provenance.js`.
Hover the `+` at `{line:4, character:18}` or `40` at `{line:4,character:15}`.

**Expected:** number expression hover over the original arithmetic/literal span,
just as for the same final two lines with no class prefix. The class declaration
does not affect the arithmetic expression's type or source location.

**Actual:** both hovers become `null`; `result` on the next line still hovers as
number. The no-prefix and native-function-with-default prefix controls produce
correct number expression hovers.

A stronger wrong-answer case is `focused.js` / `chain-class`:

```tua
class Counter {
 value: number = 0
 init(value: number) { self.value = value }
 read(): number { return self.value }
}
Counter.new(1):read()
```

Hover at `{line:5,character:16}` returns `expression: boolean[]`, range
`{start:{line:5,character:15},end:{line:7,character:1}}`, beyond the six-line
program (and trailing empty line). Definition correctly targets `read` at line 3.
The false type/range resembles internal normalized class syntax, not user code.
CLI check/pull diagnostics with lint disabled accept the source. Related
completion/signature failure is not claimed to have the same proven root;
completion owns further constructor-binding evidence.

### Likely root and coverage gap

`SemanticModel::type_of_expression_at_position` (`semantic.rs:285–294`) passes
source positions directly into AST traversal (`:1537–1546`) and returns raw node
spans. `hover/mod.rs:338–344` forwards those spans. Unlike other native-aware
queries, this path does not use `raw.parser_position_for_source` and
`parser_position_origin` around the traversal/results. Class normalization
changes the AST's coordinate space.

Existing chained-hover test (`hover/tests.rs:2076`) queries locals rather than
expression tokens; `analysis/tests.rs:7414` uses ordinary functions, not a native
class prefix. Passing name hover tests do not exercise the fallback.

### Fix direction / regression

Translate source cursor to parser coordinates before AST lookup, translate
candidate spans back to source, and reject generated-only candidates. Use shared
provenance helpers, not class-line-count offsets. Test arithmetic/literal hover
before/inside/after classes, constructor and fluent chains, methods with inline
and multiline bodies, generated-only spans, and ranges bounded by the document.
Do not merge this with the UTF-16 defect: it reproduces with ASCII-only source.

## HOV-04 — Native optional/default parameters are displayed as required

**Category:** callable metadata / editor usability correctness.
**Severity:** Medium. **Confidence:** High. **Status:** confirmed by real LSP;
checker behavior is correct in the exercised cases.

```tua
local function damage(amount: number = 5): number return amount end
local value = damage()
damage(1)
```

Run `node internal/audits/devex-2026-11-06/hover/focused.js`, cases `default` and
`optional`. Hover `damage` on the last line or request signature help just after
its opening parenthesis.

**Expected:** visible indication that `amount` may be omitted (and ideally its
default). `docs/language.md:554–573` defines the omission/default contract;
`docs/lsp.md:196–199` advertises optional parameter support.

**Actual:** both hover and signature show `damage(amount: number): number`.
The explicit `amount?: number` variant shows `amount: number | nil`, still without
an optional parameter marker. The checker accepts `damage()` in both cases, so
this is loss of call-contract presentation, not false rejection.

**Evidence:** `focused-evidence.json` / `default`, `optional`; `evidence.json`
/ `basics` additionally shows documented defaults losing their optional label.

**Root evidence:** `TypedFunction.required_parameter_count` is retained
(`types/environment.rs:613,1167,1212`), but `typed_parameter_label`
(`signature_help/mod.rs:772–777`) formats only name/type. Hover rebuilds every
parameter with `FunctionParameter::named` (`hover/mod.rs:586–592`), again without
optional/default metadata. TypedParameter itself has only name/type/docs
(`types/environment.rs:679–683`).

**Gap and fix:** inspected native signature tests exercise classes, aliases and
varargs, but not optional/default presentation. Derive call-facing optionality
from the shared callable contract (required count / richer parameter metadata)
for hover, signature and completion. Keep an optional parameter's body type
nilable while displaying its omission contract separately. Test defaulted,
explicit optional, required-nilable and required parameters, including receiver
stripping and imported functions. Native vararg loss is a related metadata
boundary but separately owned by TYP-03 because it also defeats checking.

## Capability and usability gaps — not counted as confirmed defects

### HOV-G01 — Overload active selection uses arity, not argument type

`focused-evidence.json` / `overload-type`: after typing `choose(42)`, a string-first,
number-second LuaDoc overload pair keeps activeSignature 0 (string). Both options
remain available. `signature_help/mod.rs:1029–1042` intentionally selects by
argument count; `docs/lsp.md:198–200` describes that behavior. Thus this matches
the current contract. A reasonable improvement is type-aware best-match ranking
that shares checker overload applicability and remains conservative for incomplete
arguments. Acceptance: same-arity literal/primitive/Lua/LOVE overloads select a
compatible signature without hiding alternatives. Preserve arity-only fallback
when types are unknown. Effort: medium, after callable identity/metadata fixes.

### HOV-G02 — Native class method docs reach hover but not signature help

`imports-and-docs.js` imports CounterInstance and a documented `add` method.
Hover includes its description and named @param/@return tags; signature help
shows the correct method and argument types but no documentation fields.
`signature_from_function_type` (`signature_help/mod.rs:868–885`) creates type-only
signatures with `None` documentation. `docs/comments.md:92–96` explicitly promises
resolved **hover** documentation, which passes; treating this as a missing
capability avoids overstating the contract. Reuse definition/documentation
identity while retaining the resolved type, rather than copying comments into
another parser. Test local/imported/inherited methods plus Lua module functions.
Effort: small–medium after resolved-member infrastructure (HOV-01).

### HOV-G03 — Incomplete annotation tags have no help; complete tag hover repeats syntax

`evidence.json` / `annotation`: `---@par`, bare `---@param`, and an unknown tag have
null hover. `focused-evidence.json` / `full-tags`: a valid `---@param amount number`
hovers with semantics and the offline guide link. The complete tag renders its
syntax twice, once in a `tua` fence and once in a `lua` fence. Incomplete-tag
support is already explicitly pending in ROADMAP.md; it is not a regression or
proof that valid annotations do not work. A parse-independent recognized-tag
prefix lookup plus one syntax block would improve authoring. Do not invent
semantics for unknown tags or claim @generic adds language generics.

## Independent cross-checks and consolidation

- **Workflow Unicode finding:** `crosscheck-unicode.js` and
  `unicode-crosscheck.json` apply actual rename edits as UTF-16. ASCII control
  yields intended `local points...print(points)`; one emoji prefix yields
  `localpointse...printpointse)`. Confirms source corruption, not cosmetic range
  drift. Count under workflow, not HOV-03.
- **Completion header-comment panic:** `crosscheck-comment.js` and
  `comment-crosscheck.json` confirm ordinary `local function f(x: number) -- hello`
  causes `textDocument/diagnostic` error -32603 and CLI check exit 1; the same
  comment on its own body line passes. The request worker panics at
  `emit/mod.rs:1656`, `UnexpectedToken('-')`; server exitCode remains null.
  Do not report this as termination of the entire LSP process.
- **Typing native vararg finding:** `focused-evidence.json` / `typed-vararg`
  accepts a boolean tail with native `...: number` and displays untyped `...`.
  LuaDoc `---@vararg number` control rejects the same argument and preserves
  `...: number` in hover/signature. Shared cause: native TypedFunction creation
  sets `vararg_type: None` (`types/environment.rs:1214`). Count under TYP-03.
- **Typing final-argument multi-return finding:** focused `multi-return` rejects
  `consume(pair())` with TL1004 despite `(number,string)` producer/consumer
  contracts. Signature/hover still display both declared types. Count under
  typing rather than a separate signature issue.

## Tested positive coverage and limitations

See `coverage.json` for all H rows. Positive observations include alias/local/
field hover; LuaDoc parameter docs on imported Lua functions; class imported
alias and method documentation; typed callback parameters from table.sort;
ordinary `counter:add` implicit self stripping versus `counter.add(counter,...)`;
LOVE Text dimensions, Canvas filter signature docs; string.find nilable multiple
returns; nested calls/string commas/comments/table constructors/function
expressions; vararg active-parameter clamping; complete annotation tags; and
intentional null signature after void Text:set.

Native class spaced-local variants work with inline and multiline method
bodies (`class-variants-evidence.json`). Later ten-variant controls isolated
assignment whitespace, not explicit init, as CMP-07; direct constructor-chain
receiver loss remains U-01 (see `../completion/CLASS-FOLLOWUP.md`). Do not
generalize the failure to all class usage. Likewise same-name function failure must not be generalized to
all lexical parameter shadowing, which existing tests exercise successfully.

This lane did not run a new broad Rust suite; the lead ran/recorded the baseline
suite. New probes are deliberately external fixtures, not installed product tests.
Markdown correctness was inspected in returned LSP Markdown, not visually checked
in VS Code by this lane. Malformed syntax permutations, every LOVE overload and
unbounded recursive programs are not exhaustively enumerated. Known generic
class/protocol limitations remain design questions, not hover bugs.
