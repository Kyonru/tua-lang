# Typing and inference audit

Audit baseline: `d7aed2e5f1297b80a948e055985375bceaff6f0c`. Investigation only;
no product files or tests were changed, no commit was made. The lead owns the
final deduplicated finding IDs and severity decisions. This lane owns T01–T18
and independently cross-checked multi-root isolation and two hover-lane findings.

## Reproduction and evidence

From the repository root, with the matching release CLI built:

```sh
cargo build --release -p tua-cli
node internal/audits/devex-2026-11-06/typing/run.js
# Or select a case/family (substring filter):
node internal/audits/devex-2026-11-06/typing/run.js callback-context
lua internal/audits/devex-2026-11-06/typing/runtime-oracles.lua
node internal/audits/devex-2026-11-06/typing/verify-multi-root.js
```

`TUA_AUDIT_BINARY=/absolute/path/to/tua` overrides the main runner's binary.
`cases.json` contains exact source/configuration and probe anchors for every
scenario. `run.js` recreates only its own generated projects under
`target/devex-audit-2026-11-06/typing/`, runs `tua check`, starts actual stdio LSP
servers, opens the source buffers, issues hover/completion/signature requests,
and requests pull diagnostics. It never executes fixture modules during
analysis. The final `results.json` preserves actual CLI status/output, exact
zero-based request positions, LSP results and diagnostics; filtered runs write
`results-FILTER.json`. These are observational trials, not an assertion suite:
a successful script exit means the probe ran, not that the observed behavior
was correct. Expected behavior is reviewed against the docs and controls below.

Probe positions are calculated from exact strings with JavaScript UTF-16 code
units; all typing-lane fixture text is ASCII. The lead separately tests Unicode.
Malformed/missing anchors are rejected by the runner. Some initial exploratory
probes landed on root names rather than final members; final `results.json`
contains corrected positions. Null expression hover in the provenance
cross-check is confirmed against identical text without a native-class prefix.

## Overall result

Straightforward annotations, stable inferred assignments, nested record writes,
array and dictionary contracts, LuaDoc module parameters, imported class
instances, nominal class checks, inheritance, recursive aliases, and several
control-flow guards are dependable in the exercised cases. The strongest gaps
are **silent checker/editor disagreement**, not merely absent suggestions.
The checker accepts typed callback and call-result-property programs whose
editor-visible contract says something different from their actual values.

Five confirmed defect groups are detailed here: two High, three Medium.
Scope/provenance/isolation defects independently verified here are owned by the
hover/workflow lanes and must not be counted twice. Capability gaps and
explicitly documented conservative boundaries are listed separately.

## TYP-01 — Contextual callback return contracts are skipped on common call paths

- **Category:** correctness; shared typing/checker consistency.
- **Severity / confidence / status:** High / high / confirmed by CLI and LSP;
  independently reproduced by the lead with a distinct callback and runtime
  output (`workflow/crosscheck-evidence.json`).
- **Affected workflows:** typed higher-order functions, event handlers,
  callbacks stored in annotated locals. A developer sees a strong return type
  while invalid callback implementations pass checking/building.

Minimal reproduction (`callback-context`):

```tua
local function use(cb: (x: number) -> string): string return cb(1) end
local result = use(function(x) return x + 1 end)
print(result)
```

**Expected:** reject the callback's numeric return against its expected `string`
return. `docs/typing.md` (Function Types) explicitly says typed callback
arguments inherit parameter types and callback bodies are checked against their
return type. `docs/language.md` (Function Types) promises the same representation
across checking/editor surfaces.

**Actual:** CLI exit 0, LSP diagnostic `items: []`. Hover inside the callback says
`parameter x: number`; hover of `result` says `local result: string`. Actual Lua
callback returns a number, confirmed by the lead's distinct `render` fixture.
An explicit `x: number` parameter does not repair checking
(`callback-annotated-param`). An annotated local callback also checks
successfully and gives its body parameter `any` (`callback-function-value`):

```tua
local callback: (x: number) -> string = function(x) return x + 1 end
```

**Control:** adding `local alias = use` and invoking `alias(function(x) return
x+1 end)` rejects the same numeric return (`callback-overload`). `table.sort`
comparators reject non-boolean returns (`stdlib-sort`). This is not a deliberate
policy to trust all anonymous callbacks; behavior depends on the resolution path.

**Evidence:** `results.json` cases `callback-context`,
`callback-annotated-param`, `callback-function-value`, `callback-overload`,
`stdlib-sort`. Run `node internal/audits/devex-2026-11-06/typing/run.js callback` for the family.

**Likely root cause (code-path inspection):**

- `crates/tua-core/src/check/mod.rs:2772` `check_typed_function_call_arguments`
  infers argument types and compares candidate signatures without contextual
  body checking. Anonymous functions without return annotations yield dynamic
  returns in `infer_expression_return` (`:1392`).
- The contextual implementation already exists at `check_expression_assignable_at`
  (`:1122`), reached by `check_argument_list` for `Callable::Function`, explaining
  the alias control. Direct named declarations take `Callable::TypedFunction`.
- Local annotated initialization (`:580–634`) also compares an inferred function
  type instead of routing through the contextual expression checker.
- `checked_function_bodies` (`:1236–1251`) is keyed only by source location, so
  simply running a second contextual check after a context-free pass may be
  suppressed. Fix ordering/ownership, not only one missing invocation.

**Existing tests / blind spot:**
`check/tests.rs:1437` tests nested callback mismatch with an **explicit** return
annotation; `:5237` tests `table.sort`'s function-signature path. Neither proves
unannotated anonymous returns passed directly to a named typed function or
assigned to a typed local.

**Smallest maintainable direction:** choose expected callable parameter types
before analyzing anonymous arguments; use one contextual body-checking path for
named calls, aliases, overload selections, local initializers and table fields.
Preserve explicit annotations and make body cache identity/context appropriate.

**Regression:** table-driven direct/aliased/imported function calls and annotated
locals; wrong unannotated return rejected, correct return accepted, contextual
parameter hover/completion retained. Include nested records, optional callback
arguments and overloaded candidates to prevent a built-in-only fix.

## TYP-02 — Call-result property chains look typed in hover but bypass contracts

- **Category:** correctness; expression-resolution parity.
- **Severity / confidence / status:** High / high / confirmed CLI + LSP;
  independently reproduced by lead (`workflow/crosscheck-evidence.json`).
- **Impact:** ordinary builder/factory/module code silently loses checking as
  soon as a property is read after a call, despite useful editor type inference.

Minimal single-file reproduction (`local-call-chain`):

```tua
local function factory() return {nested={count=1}} end
local value = factory().nested.count
local bad: string = value
print(value,bad)
```

Cross-file variant (`module-property`): `model.tua` exports `factory`, then a
consumer reads `m.factory().nested.count` and assigns it to `string`.

**Expected:** `TL1003 expected string, found number`, consistently with the
hovered inferred type and ordinary record-property checking.

**Actual:** both cases check successfully and LSP pull diagnostics are empty.
Hover says `local value: number`. The lead's distinct `module.make().stats.hp`
fixture checks/builds and prints Lua `number` despite the declared string slot.

**Controls:** direct imported scalar properties and direct factory scalar returns
reject the mismatch (`module-simple`, `module-return-simple`). Splitting the
chain into `local instance=m.factory(); local value=instance.nested.count`
rejects it (`module-chain-split`). This is not unknown dynamic third-party code.

**Reproduction:** `node internal/audits/devex-2026-11-06/typing/run.js local-call-chain`,
`node internal/audits/devex-2026-11-06/typing/run.js module-`. Raw evidence is in `results.json` by those IDs.

**Likely root cause:** `check/mod.rs:1497–1564` handles `VarExpression` suffixes
as field or bracket indexes and returns `None` for a call suffix. A property
chain ending in a field is represented differently from a chain ending in a
call, so `check_chained_calls` (`:1783`) does not repair this path. The base
inference walker in `types/environment.rs:3646–3684` has a parallel suffix
limitation. Editor receiver/expression queries use richer semantic resolution
(`semantic.rs:273`), creating a second answer rather than one shared fact.

**Existing tests / blind spot:** `check/tests.rs:4418` and `:4461` exercise
chains ending in method calls and their last-call return type, not factory-call
chains ending in a field/index consumed by an assignment contract.

**Fix direction:** centralize typed expression/suffix traversal (call, field,
index, method) in the shared semantic resolver, with checker hooks for argument
validation. Do not add only a completion/hover special case; those surfaces
already demonstrate the expected fact.

**Regression:** local/imported/re-exported Lua and Tua factories; terminal field
and bracket-index chains; direct annotated initializer and subsequent local use;
compare checker, hover, signature receiver and completion. Assert wrong
assignments fail and equivalent split/chained forms agree. Preserve unknown/any.

## TYP-03 — Native named-function vararg annotations disappear

- **Category:** correctness plus signature/hover presentation.
- **Severity / confidence / status:** Medium / high / confirmed CLI + LSP and
  independently verified by hover lane (`hover/focused-evidence.json`).
- **Impact:** typed vararg APIs accept wrong tail values, and editor signatures
  omit the useful declared tail type.

```tua
local function log(prefix: string, ...: string) print(prefix, ...) end
log("prefix", 1)
```

**Expected:** reject numeric vararg; hover/signature expose `...: string`.
Native typed varargs are explicitly documented in `docs/typing.md` and
`docs/language.md`.

**Actual:** CLI exit 0; no LSP diagnostic. Hover/signature label is
`function log(prefix: string, ...): any`, with untyped `...`.
The zero-fixed-parameter form also accepts invalid input (`vararg-only`).

**Control:** the equivalent LuaDoc `---@vararg string` function rejects numeric
input (`vararg-doc`). The hover lane independently used a `...: number` API.

**Reproduction:** `node internal/audits/devex-2026-11-06/typing/run.js vararg`; `results.json` contains exact
requests and responses; the initial `defaults` case also demonstrates the hole.

**Root cause:** `types/environment.rs:1208–1215` builds native `TypedFunction`
metadata with `has_varargs` but unconditional `vararg_type: None`. Typed call
candidates consume this metadata (`check/mod.rs:3811–3856`) and signature labels
also consume it (`signature_help/mod.rs:780`, corroborated by hover lane).

**Coverage gap:** `check/tests.rs:1982` covers LuaDoc varargs; `:2013` covers
native varargs through a **function-type alias/annotated local**, not the direct
native named-function metadata construction path.

**Fix direction:** preserve native vararg type in canonical `TypedFunction`
construction and propagate it through exports; avoid checker-only rejection or
signature-only label patches.

**Regression:** direct and exported native named functions, zero and nonzero
fixed parameters, valid/invalid extra arguments, LuaDoc and function-type
controls; assert hover/signature retain the type and CLI/LSP agree.

## TYP-04 — Final multi-return call arguments count as one argument

- **Category:** correctness; Lua value-list semantics.
- **Severity / confidence / status:** Medium / high / confirmed CLI + LSP,
  independently observed by hover lane; pure Lua oracle executed.
- **Impact:** valid Lua-style forwarding is rejected, forcing artificial locals
  and making normal multi-return APIs awkward.

```tua
local function pair(): (number,string) return 1,"a" end
local function consume(a: number,b: string): string return b .. a end
local result = consume(pair())
print(result)
```

**Expected:** accept: an unparenthesized final call expands its two return values.
**Actual:** CLI and LSP report `TL1004: argument count mismatch: expected 2,
found 1`. Explicit `return pair()` forwarding and multi-assignment work in
controls (`multiple-return-forward`, `multiple-returns`).

**Reproduction:** `node internal/audits/devex-2026-11-06/typing/run.js function-argument-expansion`.
`lua internal/audits/devex-2026-11-06/typing/runtime-oracles.lua` prints `expanded final call a1` on Lua 5.4.8;
this is a Lua value-list rule also applicable to the project's Lua 5.1 runtime
contract. This lane did not execute the oracle on LuaJIT.

**Root cause:** `check/mod.rs:2958` obtains syntactic call arguments, and
`function_args_call_arguments` (`:4161`) maps AST expressions one-to-one.
`check_argument_list` (`:2713`) and typed candidate checks use that length.
Return inference already has tuple-slot logic; it is not normalized into call
argument lists before validation.

**Coverage gap:** checker multiple-return tests (`check/tests.rs:1532`, `:1593`,
`:1619`) focus on assignments/returns; argument-count tests use scalar arguments.

**Fix direction:** a shared Lua value-list normalization layer should distinguish
final unparenthesized calls/varargs from scalar/nonfinal/parenthesized expressions,
carry source spans for expanded slots, and feed all callable variants.

**Regression:** `(a, pair())`, `consume(pair())`, nonfinal `pair(), scalar`,
parenthesized `(pair())`, zero/one/many known returns, varargs, colon self,
external overloads and unknown dynamic returns. Check both count and slot types.

## TYP-05 — Definitely absent assignment slots become `any`, not `nil`

- **Category:** correctness; inference precision/value-list semantics.
- **Severity / confidence / status:** Medium / high / confirmed CLI + LSP;
  pure Lua oracle executed. Related to TYP-04 but a distinct symptom/path.
- **Impact:** a statically certain nil value escapes later assignments and reads
  as dynamic, undermining contracts without any intentional `any` boundary.

```tua
local a,b = 1
local wrong: string = b
print(a,b,wrong)
```

**Expected:** infer `b: nil`, reject `wrong: string = b`. The inferred-local
contract in `docs/language.md:344` treats a definitely nil initializer as a
nil-only binding, rather than an intentional dynamic boundary.
**Actual:** CLI succeeds; lint warns `TL4028` that extra variables become nil and
`TL4045` about reading `b`, but the type checker does not reject the incompatible
assignment. Hover says `local b: any`. `local a,b=(pair())` has the same behavior,
even though parentheses force a single Lua value.

**Reproduction:** `node internal/audits/devex-2026-11-06/typing/run.js missing-slot-literal` and
`node internal/audits/devex-2026-11-06/typing/run.js parenthesized-return`; `runtime-oracles.lua` asserts both
second values are nil. Raw results preserve the contradictory lint/type answers.

**Likely root cause:** `ast_utils.rs:10–30` maps every extra assignment position
to a return index of the last syntactic expression. For scalars/parenthesized
calls that index is definitely absent, but the inference functions return `None`.
`types/environment.rs:3278–3286` skips storing the fact and
`check/mod.rs:616–627, 679–695` ultimately falls back to `Type::Any`.
The API conflates unknown type with absent known return slot. Return statement
checking already treats missing later slots as nil (`check/mod.rs:1322`).

**Coverage gap:** direct multi-return and fallthrough tests cover populated
slots, not a later typed use of a missing scalar/parenthesized assignment slot.
The linter independently catches list width, which can hide the checker gap if
tests merely assert that some diagnostic exists.

**Fix direction:** distinguish known absent slot, unknown/dynamic return and
known type in shared value-list inference. Use the same rules for locals,
assignments, returns and calls. Retain gradual behavior for unknown calls.

**Regression:** uninitialized local, `local a,b=1`, `(pair())`, direct `pair()`
with too few known returns, varargs and unknown functions; assert precise type
and TL1003 separately from linter warnings. Group implementation with TYP-04.

## Capability gaps and documented boundaries (not counted as confirmed defects)

1. **Negative `is` narrowing.** `union-is` and `negative-is` reject a return that
   would require subtracting `number` from `number | string` after an `is number`
   branch. Positive branch narrowing passes (`union-is-positive`). Current docs
   and `check/tests.rs:3303` promise positive branch behavior; false-condition
   handling (`narrowing.rs:747–786`) does not complement runtime type predicates.
   This is a useful enhancement, not an unsupported-language defect.
2. **Inline nullable callable grouping.** `((x:number)->number) | nil` is rendered
   as a union containing a tuple, and rejects assigning a function (`forward-local`,
   `mutual-recursion`). An explicit callable alias works (`forward-local-alias`,
   `mutual-recursion-alias`). Documented parenthesized **array** forms work
   (`grouped-callback-array`, `grouped-union-array`). Classify broader grouping
   syntax as a usability/design gap pending its intended grammar, not generics.
3. **Indirect mutation of narrowed locals.** A local closure can set a guarded
   outer variable to nil before its next read (`closure-mutation`), and checking
   does not invalidate that local guard at the call. `docs/language.md` Union
   Types explicitly warns to copy the guarded value before indirect mutation or
   other code; treat as a documented effect-analysis limitation. Record-path
   alias mutation *is* rejected (`path-narrow`).
4. **Generics and protocols.** Generics are explicitly unsupported; the native
   generic trial gets TL0004 (`generic-unsupported`). No defect inferred from
   absent generic classes/interfaces. Ordinary inheritance, base-typed arrays,
   nominal assignment rejection and recursive `Self` aliases were exercised.
5. **No-return stubs.** An empty unannotated function returns `any` in the editor
   (`void-return-slot`), with a deliberate-stub linter warning. Do not conflate
   this conservative stub policy with the definitely missing scalar slot in
   TYP-05. A separate policy decision could improve inferred no-value returns.

## Independent findings owned elsewhere

- **Multi-root isolation (workflow lane):**
  `verify-multi-root.js` and `multi-root-crosscheck.json` independently show
  `menu/src/view.tua` resolving `shared` to `inventory/src/shared.lua`, displaying
  number instead of string and reporting false TL1003. Distinct main filenames
  exclude basename-only collisions. Here inventory wins; the lead's other root
  ordering chose the other project. Root ordering does not make this safe.
- **Local function lexical shadowing (hover lane):**
  `shadowed-local-functions` / `results-shadowed.json` uses distinct `convert`
  bodies. Inner string-returning function body and valid call are both rejected
  against the outer number function. Hover/signature and inferred `inner` value
  show the outer number contract. `TypeEnvironment::function` (`:296`) uses
  name-only lookup. This is separate from ordinary local-value shadowing, which
  passes in `scope-shadow`.
- **Native-class expression provenance (hover lane):**
  `provenance-crosscheck-*` / `results-provenance-crosscheck.json` independently
  shows arithmetic and literal hover disappearing after a valid native class.
  Identical expressions without a class or after a defaulted function work.
  Local-variable hover stays correct. Exact source/request coordinates are
  included. The hover lane owns deeper range-corruption variants; this lane
  does not claim independently reproducing those variants.

## Recommended implementation order

1. Resolve lexical/project identity findings first (owned by other lanes), so
   semantic facts cannot attach to the wrong symbol or project.
2. Canonicalize callable metadata and contextual checking (TYP-01 + TYP-03).
   TYP-03 is a smaller metadata preservation slice; TYP-01 requires checking
   order and callback-context coverage. Approximate effort: small and medium.
3. Share expression suffix evaluation between checker and editor (TYP-02),
   preserving lexical identities and current narrowing. Approximate effort:
   medium; broader than a one-line completion fix.
4. Normalize Lua value lists and distinguish missing versus unknown slots
   (TYP-04 + TYP-05). Approximate effort: medium; regression matrix matters more
   than adding isolated argument-count exceptions.
5. Evaluate negative predicate narrowing and callable grouping usability after
   the correctness paths agree. Approximate effort: medium/grammar-dependent.

Acceptance must compare CLI diagnostics, LSP diagnostics, hover/inlay types and
applicable completion/signature responses for the same fixture, including
negative controls. A passing editor-only test is insufficient for TYP-01/02.
