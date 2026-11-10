# Tua 0.7.0 implementation plan: protocols, then generics

Status: implementation and qualification plan, updated November 10,
2026. Method-only protocols and the scoped native generics plan (aliases, free
functions, generic classes, and one protocol bound per generic parameter) are
implemented with separate commits and regression coverage. Protocol contracts
and generic arguments erase from Lua. Single inheritance is retained; generic
inheritance substitutes applied parent types. Remaining protocol edge coverage,
cross-module class inheritance, installed-editor/runtime qualification, and
cross-platform release checks are tracked below and must not be described as
passed. The locally packaged macOS arm64 VSIX was installed into an isolated
VS Code profile and passed its real-server suite, including protocol
implementation navigation and cross-file rename. A November 10 follow-up audit
also fixed comment loss in generic class-bound lowering; emitted Lua and the
real-server extension-host check pass. See
`internal/audits/generics-polymorphism-followup-2026-11-10.md` for exact
commands and workspace-check limitations. This document is not a release
announcement.

## 1. Outcomes and release boundaries

A developer should be able to:

1. Describe a behavior once and accept unrelated classes that explicitly provide
   it, without manufacturing an inheritance relationship or receiver wrappers.
2. Get a precise error at a missing/incompatible implementation and navigate
   between the contract, its implementations, and its uses.
3. Write reusable typed functions, data aliases, and classes without losing the
   relationship between input and output types.
4. Keep ordinary Lua execution, LOVE compatibility, gradual `any` boundaries,
   source maps, and responsive editing.

Protocol support is the first complete deliverable, including editor support
and runtime tests. Generics remain a 0.7.0 goal, but no incomplete feature should
ship solely to meet the version number. If the scope must shrink, record it
explicitly before release. Do not mark a syntax-only implementation complete.

Out of the initial scope: multiple class inheritance, abstract classes, default
protocol implementations, associated types, runtime reflection/type tests,
implicit method binding, operator overloading, higher-kinded types, generic
parameter packs, specialization/monomorphization, and a new runtime.

## 2. Implementation snapshot

- `docs/classes.md`: final-by-default classes, single inheritance, `override`,
  `super`, nominal class identity, imported instance types, virtual dispatch,
  and the initial method-only protocol contract.
- `examples/features/src/polymorphism.tua`: base-typed actor arrays and explicit
  receiver-binding adapters for unrelated UI/gameplay objects.
- `parser/dialect.rs`, `parser/model.rs`, and normalization: native generic
  aliases, free functions, generic classes, applied generic parents, and one
  protocol bound per generic parameter retain declaration identity and spans.
  Unsupported or malformed bounds produce targeted diagnostics.
- `parser/model.rs`, `parser/doc.rs`, and the shared type scheme model: generic
  function binders retain declaration identity through checking, imports,
  re-exports, hover, signatures, definition, references, and rename.
- `check/mod.rs`: generic function values specialize from known callback
  parameter and return signatures; callback-only inference of a generic callee
  remains ambiguous.
- `types/model.rs` and `types/environment.rs`: type aliases have declaration-
  owned binder identities and structured applications, with simultaneous
  substitution through supported structured type positions. Expansion keeps
  recursive edges symbolic when a declaration repeats, even if its arguments
  change, and bounds chains of distinct aliases.
- `type_database/mod.rs` and project queries: named and namespace imports,
  named re-exports, and declaration identities are preserved across modules.
- `parser/doc.rs`: generic alias applications in LuaDoc type text are parsed
  structurally for shared validation; LuaDoc `---@generic` remains metadata-only.
- `types/assignability.rs`: function variance and structured failure paths
  exist; mutable arrays currently compare element types covariantly. Generic
  function inference separately treats binders inside mutable arrays,
  dictionaries, and record fields as invariant.
- `classes.rs`: protocol declarations and class `implements` clauses now retain
  source spans and erase through class lowering. Local and imported
  method-contract checks are implemented; protocol completion, hover,
  signature-help, and navigation have cross-file core/LSP coverage. Generic
  bounds require explicit conformance, preserve canonical imported protocol
  identity, and expose protocol members through the bounded type parameter.
- `crates/tua-core/src/analysis/host.rs`: project module/type indexes reuse
  last-good parsed and semantic facts when an open source file becomes invalid,
  so type-only protocol imports stay available during edits.
- LSP already advertises implementation navigation. Extend its shared query;
  do not add a second unrelated implementation lookup.

Evidence as of November 10, 2026, after the incomplete generic-class completion
fix: plain `make verify` passes with the Makefile's default 16 MiB Rust test
stack and parallel execution. The latest focused suites include 1,899 core
tests passed (1 ignored) and 434 LSP tests passed; the broader CI-equivalent run
also exercised 92 CLI library tests (2 ignored), 137 CLI integration tests,
3 gameplay tests, 1 helper-default test, 3 LOVE example tests, 1 LSP smoke
test, 1 Lua syntax test, and 180 VS Code extension tests. Clippy,
schema/version/configuration checks, language evaluation, JSON validation, and
diff checks also pass. VS Code inspector tests require local loopback access;
they failed under the restricted sandbox and passed on the loopback-enabled
full verification run. Core and LSP
editor requests cover imported protocol completion, hover, signature help,
definition, and implementation navigation; the incomplete-edit completion
cases now include both imported protocols and members inherited by a generic
class parameter from last-good facts. Regressions exercise mutually
recursive imported contracts and method returns, positive and negative
conformance, exact nominal hover, and protocol-member completion. Imported
generic bounds accept explicitly conforming classes and retain the diagnostic
for same-shaped classes that do not declare conformance. Additional regressions verify cyclic dependency
deletion invalidation through AnalysisHost and watched-file/pull-diagnostics
LSP flow, parser/editor recovery across UTF-8 prefix boundaries for a nested-type
protocol fixture, and inherited imported-protocol diagnostic ranges after
Unicode-prefixed lines, multiple imported source files, transitive class
inheritance, and create/edit invalidation of missing type-only protocol
dependencies. The `source_pipeline` nightly fuzz target exercises normalization
and parsing, including protocol-related seeds; it ran for 31 seconds (21,595
executions, no crash; peak RSS about 600 MiB). The separate
`protocol_and_paths` target exercises LSP framing and path confinement, not
protocol parsing; it ran for 31 seconds (711,266 executions, no crash; peak RSS
about 533 MiB). Neither target directly exercises editor declaration recovery.
Prior full-package evidence and its stack-sensitive serial settings are recorded in
`internal/audits/devex-space-invaders-0.7.0/REPORT.md`. That report contains a
five-module game exercise, generated-Lua smoke test, exact limitations, and
remaining verification gaps. Generic aliases and the G2 generic free-function
slice are implemented. Generic classes support constructor inference, member
substitution, applied inheritance, imported instance aliases, and protocol
bounds. The VS Code real-server host passes imported generic alias hover,
value/member hover, completion, and signatures. After the cyclic protocol fix,
the 180-test extension check passes with local loopback access and the rebuilt
40-file 0.7.0 VSIX passes its installed real-server test on this macOS host.
That installed test covers imported protocol completion, hover, requirement
definition, implementation navigation from a protocol-typed call, and
cross-file requirement rename, plus generic alias/class/function completion,
hover, signatures, and alias definition. The editor test places the cursor
inside the identifier being renamed, as VS Code requires. The protocol
feature-tour source now has a CLI runtime regression that builds the example
and runs its assertions with system Lua 5.4.8, covering a bounded generic
helper and heterogeneous protocol dispatch; the same test checks that a
nonconforming class reports both missing methods. LOVE, LuaJIT,
source-map/hot-reload, cross-platform, visual screenshot review, and long-session
release qualification remain open.

## 3. Language contract and remaining decisions

The initial protocol syntax and several semantics below are implemented. The
remaining bullets are acceptance requirements or design decisions, not claims
that the complete protocol slice is finished. The milestone table in Section 5
tracks what still needs verification or implementation.

### Protocol declarations and explicit conformance

Use one term and spelling: `protocol`, with `implements` on classes. Do not
introduce synonymous `interface` syntax in the first release.

```tua
protocol Updatable {
  update(dt: number): nil
}

final class Panel implements Updatable {
  elapsed: number = 0
  update(dt: number) {
    self.elapsed += dt
  }
}

open class Actor implements Updatable {
  update(dt: number) {
  }
}

local function tick(items: Updatable[], dt: number)
  for _, item in ipairs(items) do
    item:update(dt)
  end
end

tick({ Actor.new(), Panel.new() }, 0.016)
```

Contract rules:

- Method requirements only; no fields, bodies, constructors, static members,
  default argument expressions, or implicit optional requirements initially.
- Explicit parameter and return types on requirements. Support the existing
  optional/variadic and multi-return type forms where their meaning is already
  defined. Specify syntax and omission diagnostics rather than inferring a
  contract from its implementations.
- `implements A, B` may list multiple protocols; class `extends Base` continues
  to mean exactly one superclass. Combined header order: `extends` before
  `implements`. Treat new keywords contextually where possible; test existing
  Lua identifiers named `protocol` and `implements`.
- Classes opt in explicitly. Same-shaped unrelated classes and plain tables do
  not conform automatically. Protocol values cannot be constructed or used as
  runtime expressions. `any` remains a documented unchecked escape hatch.
- A subclass inherits its parent's conformances. An inherited concrete method
  can satisfy a new protocol. A subclass override must preserve every inherited
  conformance, even if the existing override checker takes another code path.
- Extra methods are permitted. Requirements with the same name from multiple
  protocols must all be satisfied by the concrete implementation; reject an
  unsatisfied obligation rather than choosing the first declaration.
- Protocol inheritance is deferred from the first slice. Multiple explicit
  conformances cover the initial use cases without a second inheritance graph.
- A protocol-typed expression exposes only the contract, not inferred concrete
  implementation members. Narrowing must not invent a runtime type test.
- Parameters are contravariant and returns covariant; required/optional arity,
  varargs, multi-return positions, and nil padding obey the existing function
  contract. Preserve current class override rules unless deliberately revised.
- `Self` on a requirement initially means the protocol's instance view, not the
  particular hidden concrete class. Returning an implementer can satisfy it;
  accepting only that concrete class cannot satisfy a protocol-wide input.
  Exact-self/associated-self semantics are deferred and must not be implied by
  hover or documentation.
- Methods remain colon calls with an implicit receiver. A protocol requirement
  does not create a bound closure. Dot calls and extracted methods follow the
  existing explicit-receiver contract; test both supported and rejected forms.
- Protocol type imports/re-exports use the existing type namespace and module
  identity. Importing a type must not execute a module merely to obtain it.
- Protocol annotations and conformance declarations erase from Lua. Existing
  construction, metatables, `super`, initialization order, and hot reload stay
  unchanged. Conformance does not add proxy objects or dispatch tables.

### Generic direction after protocols

Proposed surface to evaluate against the parser and formatting contract:

```tua
type Result<T> = { value: T | nil, error: string | nil }
local function identity<T>(value: T): T
  return value
end
-- Later: class Pool<T> and function tick<T: Updatable>(value: T).
```

- Generic parameters belong to a declaration identity and scope, never just a
  string such as `T`. Nested `T` binders cannot capture each other or an imported
  alias of the same name. Preserve navigation spans for binder declarations.
- Start with aliases, then function inference, then classes and protocol bounds.
  Generic protocols follow only after ordinary protocols and class generics are
  complete; do not require them to ship basic conformance.
- Type arguments are erased. Runtime values and call evaluation count/order are
  identical to their nongeneric Lua equivalents.
- Infer from arguments and contextual callbacks with a bounded, documented
  algorithm. A function body is checked for all allowed type arguments, not
  separately accepted because one call happens to use a convenient type.
- Distinguish an unsolved inference variable from deliberate `any`. An unresolved
  required type argument gets an actionable diagnostic, not silent `any`.
  Handle empty tables, return-only type parameters, conflicting inputs, nil,
  unions, callbacks, recursive functions, and independent repeated calls.
- First-class generic functions retain their binders when assigned or exported;
  otherwise explicitly reject the unsupported use. Do not accidentally turn
  one call's specialization into the declaration's global signature.
- Define explicit call-site type arguments before enabling that syntax; avoid
  ambiguity with comparison operators. Defaults, packs, and overload-specific
  generic ranking can remain deferred with clear diagnostics.
- Mutable generic class parameters are invariant initially. No user-declared
  variance until its soundness and editor behavior are specified. Protocol
  views expose method contracts only; no writable-field variance is implied.
- Generic function inference is invariant when a binder flows through a mutable
  array, dictionary, or record position. Existing non-generic mutable-array
  assignment compatibility remains unchanged and should be characterized
  independently; do not silently change old array code as a side effect of
  introducing generic classes.
- Reuse shared binder/substitution machinery for supported LuaDoc `@generic`
  forms; unsupported annotation forms must remain clearly documented.

### Generic design decisions (G0)

The first native generic release uses a narrow, type-only surface:

```tua
type Box<T> = { value: T }
type Result<T, E> = { value: T | nil, error: E | nil }
local box: Box<number> = { value = 4 }
```

- G1 aliases and G2 free functions accept ordered, required, unbounded,
  non-variadic binders. Generic class headers/parent applications parse, but
  class type application and substitution semantics remain deferred to G3.
- Applications use `Name<Arg, ...>` only in type positions. Explicit call-site
  type arguments are excluded from the initial function inference design, so
  `f<T>(x)` cannot be mistaken for a comparison expression. Defaults, packs,
  higher-kinded types, variance annotations, and generic protocols are deferred.
- Each parameter has a stable identity derived from its owning type declaration
  identity and ordinal. Type resolution binds by identity, never by the printed
  spelling alone; a parameter named `T` shadows an outer/imported alias `T`
  within its declaration body. Navigation retains each declaration span.
- The parser model stores declaration binders and a structured type application
  node with source spans. The semantic type model preserves an instantiated
  alias reference (declaration identity plus arguments) until resolution; it
  must not eagerly stringify or globally mutate the generic declaration.
- Alias expansion uses simultaneous substitution over the complete type tree.
  A substitution value is inserted as-is rather than recursively substituted
  again, which prevents capture and order-dependent replacement. Recursive
  aliases are allowed when their instantiated expansion revisits the same
  declaration with compatible arguments; expansion depth and work are bounded.
- Generic alias arity is exact. Bare uses of a generic alias, extra or missing
  arguments, and unknown generic names are errors at the smallest relevant
  type span. They never silently become `any`. G1 does not infer arguments.
- Generic alias declarations and applications erase completely. Runtime
  expressions, evaluation order, module loading, and emitted Lua are unchanged.
  Formatter round-trips preserve syntax; Lua 5.1 output contains no type syntax.
- G2 implements first-order inference for direct calls and function values,
  independent per-call instantiation, imported/re-exported schemes, variadic
  arguments, and universally checked bodies. The shared solver reports
  conflicts and unresolved binders rather than widening them to `any`. Callback
  and return-only inference remain explicitly ambiguous; generic methods and
  explicit call-site type arguments remain unsupported. LuaDoc supports
  unbounded `---@generic T` on free functions and binds it through `---@param`,
  `---@vararg`, and `---@return`; bounds and generic methods produce targeted
  diagnostics. Generic binder definition, references, and rename work in one
  file, including nested shadowing. Binders inside mutable arrays, dictionaries,
  and record fields are invariant across call arguments, preventing widened
  writes into narrower caller-owned containers while retaining concrete types
  for read-only generic uses. TL1015 highlights the conflicting actual, the
  ambiguous callback argument, or the whole call for unresolved binders.
- G3 generic class parameters are invariant because instances are mutable.
  Subtyping does not widen `Pool<Derived>` to `Pool<Base>`. Protocol bounds are
  checked through existing conformance facts, while the constructed value keeps
  its concrete class identity and substituted inherited members.
- Generic call solving retains at most 4,096 distinct specializations for one
  source-file check and reuses identical `(scheme, argument types)` results
  within that check. The checker lifetime supplies the source/module revision
  boundary, so this cache cannot survive an edit or cross projects. A generated
  source regression asserts the first over-budget call receives one positioned
  `TL1015`. Alias expansion and recursive constraint walkers also have depth
  or work guards. Direct tests now pin the generic alias expansion work
  boundary at 4,096/4,097 applications and the native array type nesting
  boundary at 64/65 suffixes; protected comment/string text does not count.
  Generic angle nesting is also bounded by the existing operator budget. No
  cross-request generic instance cache is retained, so persistent-cache
  invalidation and retained-instance limits are not applicable until such a
  cache is introduced.
- LuaCATS `@generic` is not enabled by G1. The G2 parity gate must name each
  accepted LuaCATS form and normalize it to the same binder model; unsupported
  LuaCATS forms remain diagnosed or documented as unsupported.

## 4. Shared architecture and implementation boundaries

Paths below are relative to `crates/tua-core/src` unless stated otherwise.

| Layer | Implementation direction | Required invariant |
| --- | --- | --- |
| Parser/model, `classes.rs` | Structured protocol requirements and class conformance clauses with original spans; later generic binders and applications | No regex-only semantic model; recover partial declarations without losing unrelated code |
| `hir`, `scope`, `semantic_index` | Declaration/member/type-parameter IDs, import identity, namespace binding | Same spelling in two modules or scopes does not merge identities |
| `types/model.rs`, `types/environment.rs` | Protocol instance views, conformance facts; later type schemes and substitutions | Preserve ordinary class nominal identity and dynamic `any` |
| `types/assignability.rs` | One reusable conformance/signature compatibility operation returning structured failures | Checking, diagnostics, IDE explanations use the same result |
| `semantic.rs`, module index/link, `analysis` | Snapshot-owned project queries and conformance reverse index; bounded generic-call memoization is scoped to one checker/source revision | No request-time project scans or stale conformance after edits |
| `check` | Requirement declaration validation and implementation diagnostics | Report real errors once; defer project-dependent facts during fast diagnostics |
| `emit/lowering`, `emit` | Validated erasure operations and source provenance | Runtime-equivalent output; no conflicting edits or lost comments |
| Core IDE query modules | Resolve protocol members and instantiated signatures through shared semantics | Hover, completion, signatures, inlays, navigation agree |
| `tua-lsp` | Existing query adapters, capabilities, UTF-16 ranges, cancellation | No protocol or generic type checker inside handlers |
| VS Code | Grammar, snippets, completion presentation, documentation, tests | No separate semantic parser in extension code |

Prefer focused modules for protocol facts and substitution over expanding the
already broad class/environment code with many special cases. Exact module
names are implementation choices, not a reason to introduce a new crate.

The checker-local generic call cache uses the full function scheme (including
binder/declaration identity) and inferred argument types as its key. Its
per-check lifetime scopes it to the source and immutable environment revision;
it does not cross snapshots. Bound recursion, distinct instantiation count,
retained per-check cache entries, and cancellation work. Share immutable project
facts across snapshots. A future snapshot-level cache would need an explicit
module/source revision in its key and fresh-versus-warm invalidation tests.

## 5. Milestones and commit-sized work

Effort is relative complexity, not a calendar promise: S = localized change,
M = several connected modules, L = cross-cutting semantics/editor work.
Each row may require multiple behavior-sized commits. Do not split a behavior
from the tests and current documentation that prove it.

| ID | Scope and dependencies | Work and completion gate | Status | Effort |
| --- | --- | --- | --- | --- |
| P0 | Contract and baseline; first | Settle syntax/semantics, diagnostic locations, namespace and receiver rules. Add positive/negative fixture specifications and characterize mutable-container risk. Existing inheritance regression stays green. | Verified: method-only nominal contracts, colon dispatch, explicit-receiver dot/extracted calls, accepted and rejected receiver cases, mutable class-array/protocol-array interactions, and existing class inheritance are covered by positive and negative tests. The current protocol call contract is explicit in docs. | M |
| P1 | Protocol syntax and identities; P0 | Parse declarations/conformance clauses, scope type names/members, preserve docs and spans, recover edits, format and erase safely. Unsupported use must never be silently accepted. | Implemented with remaining malformed-edit coverage: local/exported protocol declarations, class clauses, spans, identities, erasure, and docs are tested. Balanced multiline parameters, inline comments, trailing return comments, formatter round-trip, header comments between declaration tokens, and unclosed declarations are covered. Comments around generic function and class protocol bounds survive parser normalization and Lua erasure; a follow-up found and fixed comments being attached to an erased generated type alias. `protocol`/`implements` remain ordinary identifiers outside declaration position, including beside Tua type syntax. Editor-only recovery closes an unclosed protocol body after lexically parsing its header (including comments containing braces), so current-buffer locals and its partial contract remain queryable while diagnostics still report the missing brace. It also closes incomplete generic class parameter lists and missing class bodies in editor-only parsed copies, preserving partial class binders and bounds; the original source still receives syntax diagnostics. Prefix tests iterate UTF-8 boundaries and directly assert that a completed protocol header and generic class header recover before their first member; the loop's conditional checks do not assert successful recovery at every malformed prefix. Malformed class-like text inside protocol bodies and protocol-like declarations nested inside class bodies are diagnosed without panicking or producing overlapping lowering edits. Declaration extraction tracks Lua block scope, leaving nested protocol and generic class text for the parser to reject instead of erasing it as top-level declarations. The `source_pipeline` fuzz target exercises normalization/parsing with protocol-related seeds; `protocol_and_paths` covers LSP framing and path confinement, not protocol parsing. Neither directly exercises editor declaration recovery. | L |
| P2 | Local conformance and dispatch; P1 | Share method compatibility, inherited methods and conformances, multiple obligations, protocol assignments/calls, and structured errors. Invalid signatures fail at the correct span. | Partial: local/imported conformance, protocol-typed values, inherited satisfaction through multiple class levels, valid inherited overrides, multiple obligations and variance pass/fail cases are covered. Optional arguments, typed varargs, each return slot, and return arity have explicit regressions. Diagnostics identify mismatching slots and required-argument/vararg/return-count failures; incompatible inherited overrides link each local or imported protocol requirement inherited through the same-file class hierarchy. New LSP matrices exercise parameter, return, required-argument-count, vararg, and return-count mismatches, asserting exact override and related requirement ranges. Missing and incompatible imported methods carry navigable related information to the protocol requirement. Imported generic bounds accept explicitly conforming classes and reject same-shaped nonconforming classes in CLI/LSP regressions. LSP regressions check primary and related ranges after Unicode-prefixed lines, including multiple inherited requirements originating in separate imported modules. | L |
| P3 | Modules and invalidation; P2 | Type imports, exports/re-exports, declaration-file contracts, cycles and missing modules. Unsaved contract/member edits refresh dependents without transient false errors. | Partial: exported protocols, named and namespace type-only imports, and project diagnostics for missing/incompatible imported methods work, including when `implements` is the import's only use. Saved and unsaved contract edits refresh those diagnostics. New pull-diagnostics coverage verifies unsaved consumer edits that remove conformance, reintroduce it with a missing method, and then restore the method. A second regression changes a protocol through a two-hop declaration-file re-export, removes the intermediate export, and restores it, asserting that consumer diagnostics update and stale conformance errors disappear. Named declaration-file re-exports and namespace re-exports resolve to the original protocol identity. Mutually recursive same-file and cross-file protocol signatures and method returns retain nominal identity: matching conformance passes, incompatible parameter types are rejected, returned `B` remains `B`, and member completion agrees with exact hover. A changed requirement inside the cross-file cycle refreshes dependent diagnostics while the other protocol's completion remains available in the same snapshot. Deleting a protocol dependency invalidates its last-good contract and reports the missing import instead of stale conformance errors; a watched-file/pull-diagnostics replay confirms this when the deleted module is one node in a cyclic protocol graph. Creating a missing type-only protocol dependency clears the import error, and editing the created contract refreshes conformance diagnostics. Renaming a dependency, updating the consumer import, and waiting for the scheduled full-diagnostics pass resolves the new contract and reports its incompatibility. Multi-workspace isolation is tested with same-named protocol contracts and same-named generic aliases with different nullability; each consumer resolves only its own root's type, regardless of workspace-root ordering. Workspace-folder changes reindex open-document analysis and refresh push diagnostics. Delete/recreate cycles cover each file in a three-module re-export chain, and a renamed direct protocol dependency now survives repeated delete/create cycles. Broader cancellation-race permutations remain open. | L |
| P4 | Complete protocol IDE slice; P3 | Member completion, hover/docs, signatures, inlays, definitions, implementations, references/rename, tokens, type explanations, actionable diagnostics. Verify real LSP responses and installed VS Code. | Partial: core and in-process LSP requests cover completion after a typed method prefix; protocol-name completion after `implements`; and protocol-bound completion in incomplete multiline generic class headers before `>` is typed. Incomplete headers continue to offer type completion without treating `implements` as a global keyword or suggesting types from comments. Other covered behavior includes incomplete edits with last-good project facts, hover signatures and protocol requirement comments (with the implicit receiver hidden for colon calls), signature help, definition, implementation navigation, references, and rename across type-only imports. Recursive same-file and cross-file protocol fixtures assert exact hover/completion agreement. A real `tua lsp` process probe confirms invoked completion after an incomplete imported-protocol edit. New core and LSP tests preserve completion for a protocol-bounded generic class parameter in both a fresh incomplete buffer and after editing a valid class into an incomplete state. An imported `.lua` LuaDoc-generic regression checks specialized hover/signature, inferred result hover, member completion, and definition back to the Lua declaration. Semantic tokens classify protocol types as interfaces and requirements/calls as methods. Inlay hints and type explanations preserve imported protocol member types. The installed VSIX real-server test passes imported protocol completion, documented hover and requirement definition, implementation navigation from a protocol-typed call, cross-file requirement rename, generic alias/class/function hover, completion, signatures and definitions, plus unsaved imported generic alias edits that update and restore dependent hover types. Cross-platform qualification remains open. | L |
| P5 | Usable protocol examples/docs; P4 | Add runnable game/UI and negative examples, runtime/source-map/hot-reload qualification; preserve adapters for Lua interop. | Partial: class/typing/module docs and a feature-tour sample explain the shipped slice. A CLI regression builds `src/protocols.tua` and runs its assertions under Lua 5.4.8, exercising two unrelated concrete classes through a protocol array and a bounded generic helper that returns each concrete class. The negative case verifies that a class missing both methods reports both obligations. An end-to-end CLI regression also executes a failing generic class method under Lua and maps the traceback to its Tua line. Existing CLI/extension tests exercise source maps and hot reload. LOVE/LuaJIT runtime qualification and visual behavior remain unverified on this host. | M |
| G0 | Binder/substitution design; after P5 | Specify generic inference, recursive types, variance, first-class functions, contextual typing, limits and error policy; reuse protocol bounds. No unused framework-only abstraction. | Decisions recorded above: native alias syntax, binder identity, substitution semantics, arity/error behavior, inference boundaries, erasure, mutability variance, and resource limits. | M |
| G1 | Generic aliases; G0 | Scoped binder IDs, arity checks, type applications, bounded capture-avoiding substitution, recursion/occurs handling, imports and IDE display. Connect machinery to its first consumer. | Implemented: exact-arity and unknown-name diagnostics, structured substitution, named/namespace imports, named re-exports, shared protocol and class signatures, LuaDoc type-text parsing, formatter/emission erasure, and core editor queries. Resolution preserves the first repeated declaration as a symbolic edge even when its arguments change, preventing long synthetic type chains; separate work/depth guards bound other recursion. Focused core/project tests cover local and imported growing recursion. Broad release qualification remains open. | L |
| G2 | Generic functions; G1 | Universally checked bodies, per-call inference, returns/callbacks, independent instantiations, navigation and actionable failures; align supported LuaDoc forms. | Implemented for generic free functions, direct/assigned/local/imported/re-exported calls, varargs and unbounded LuaDoc tags. Mutable container positions are invariant during inference. A per-source-file checker budget limits generic call instantiations to 4,096 and reports one TL1015 when exceeded; the boundary has a generated-source regression. Callback-only/return-only inference, methods, and explicit call-site arguments remain diagnosed or unsupported. Core and CLI runtime tests cover independent multi-return slots and execute erased generic tuple results; the locally installed VSIX real-server test verifies imported generic function hover, completion, signatures, result hover, and definition navigation. | L |
| G3 | Generic classes and bounds; G2 | Constructor inference, instance members, inherited substitutions, `Self`, invariant mutable containers and protocol constraints. Retain runtime/class identity and editor parity. | Implemented for the scoped surface: local and imported generic classes, constructor/member substitution, applied generic parents including concrete specializations on non-generic subclasses and transitive inherited substitutions, generic `super(...)`, invariant class arguments, explicit one-protocol bounds, cross-file generic function schemes, bounded-body member completion/hover/navigation, erasure, generic override signature comparison after parent substitution, and imported protocol requirements inherited through a local base class. Cross-module class inheritance remains unsupported. The locally installed VSIX real-server test verifies imported generic class hover, field/method completion, constructor/method signatures, field/method definitions, generic alias navigation, and unsaved imported generic alias edits that refresh dependent hover types. Generic methods, generic protocols, and compound bounds remain deferred. | L |
| R0 | Qualification; P5 and selected complete G milestones | Docs/examples, performance, cross-platform CI, installed-editor tests and package smoke; publish honest release scope and migration notes. | Open. | L |

Commit convention: one coherent implementation/fix with its regression tests
and documentation. Use unsigned local commits and no co-author trailer, as
requested. Release preparation, tagging, pushing, and publication are separate
operations from this plan.

## 6. Test strategy and acceptance matrix

Use the existing `Fixture`, analysis snapshot, LSP request, extension-host, and
emitted-Lua harnesses. Avoid a new parallel test runner. Table-driven semantic
cases should assert types/diagnostic codes and spans, not only absence of panic.
Negative fixtures must require the specific failure; an unrelated parse error
must never count as success.

| Risk / fixture family | Cases and oracle | Layer / execution cost |
| --- | --- | --- |
| Parse and recovery | Comments inside headers, blank lines, CRLF, Unicode before declarations, missing braces/types, duplicate methods, keyword identifiers; stable spans and useful partial IDE results | Parser/formatter unit tests; fast |
| Class compatibility | Base/subclass assignments, final/override checks, unrelated same-shaped classes, inherited implementations, explicit conformance, conflicting obligations | Core checker/assignability; fast |
| Method variance | Concrete nominal types, broader/narrower inputs, narrower/broader outputs, optional/required arguments, varargs, nil, every return slot, recursive callback types | Core matrix; fast; do not use `any` to prove variance |
| Receiver correctness | Colon invocation, explicit-self dot calls, extracted methods, callbacks, chained returns; no hidden binding or dropped receiver | Core inference plus emitted Lua execution; medium |
| Import identity | Selected and namespace type imports, `.tua`, `.lua`/LuaDoc where supported, `.d.tua`, direct/deep re-exports, duplicate names across roots, cyclic graphs | Module/snapshot tests; medium |
| Editing freshness | Unsaved requirement changes, removed conformance/method, renamed/deleted dependency, workspace switch, close/reopen, incomplete source, cancellation | LSP protocol replay; medium |
| Diagnostic quality | Protocol + class + method names, expected/found signatures, mismatched argument/return slot, primary and related locations; deduplication and stable code | Core and LSP diagnostics; fast/medium |
| IDE agreement | Exact member labels and insertion ranges, no concrete-only leaks, implicit-self display, active nested-call parameter, docs, nil/union display, inlay types | Shared core fixtures plus actual LSP; medium |
| Navigation and rename | Contract vs implementation targets, inherited methods, all statically known implementers, re-export provenance, collision checks, UTF-16 edits | Navigation/refactor core + LSP + selected editor tests; medium |
| Generic scope | Repeated/shadowed T, unrelated imported T, alias/type-parameter collisions, recursive and mutually recursive applications, wrong arity | Core parser/type tests; fast |
| Generic inference | Identity, optional/union inputs, empty tables, multiple independent calls, callbacks and closures, multi-return, return-only T, constraints, body invalid for arbitrary T | Checker + hover/signature shared fixtures; medium |
| Generic mutation | Pool<Fighter> to Pool<Actor> rejection; reads/writes through aliases; nested mutable fields; any escape explicitly tested | Assignability + runtime counterexamples; fast/medium |
| Emission | Contracts/type arguments erased, comments preserved, side effects once/in order, init/super unchanged, no runtime type imports, correct class traceback positions | Structural output assertions and executable Lua/LuaJIT fixtures; medium |
| Resilience | Deep types, cyclic protocol dependencies, malformed header storms, bounded diagnostics, stack/memory limits, deterministic cancellation | Existing fuzz targets + bounded stress regressions; slower |
| Real UX | Type a declaration and implementation from scratch, import into another file, trigger and fix an error, invoke implementation/rename, use generic class | Installed VS Code + real server; slower and separately reported |

### End-to-end acceptance scenarios

A. **Gameplay/UI loop:** unrelated Actor and Panel explicitly implement
Updatable; a protocol array calls both and runtime state proves dispatch reached
the correct receiver. An extra class with a missing method is rejected.

B. **Signature failure:** change Panel.update to require a string. Error names
Panel, Updatable.update, parameter dt, expected/found types; related location
points to the requirement. Correct it unsaved and observe diagnostics clear.

C. **Cross-file edit:** export the protocol through two modules; change its
return contract in an unsaved buffer. Completion, signatures, checking and
implementation navigation agree in consumers, including after close/reopen.

D. **Safe rename:** rename a protocol method with implementations in different
modules, including an inherited implementation satisfying two protocols. Either
update the entire verified relationship without collisions or refuse with a
specific reason. Never partially rename a family. Dynamic Lua uses remain an
explicit limitation, not fabricated references.

E. **Identity preservation:** generic identity called with Fighter and Panel in
the same file yields two independent concrete result types, both locally and
through imports. A generic body returning a number for arbitrary T is rejected.

F. **Generic pool:** Pool<Fighter> returns Fighter through acquire/release and
chaining. Unsafe widening to Pool<Actor> is rejected. Generic bounds support a
function requiring Updatable while preserving the concrete result type.

## 7. Editor and diagnostic behavior

- Definition on a protocol-typed call goes to its requirement; implementation
  navigation returns statically known concrete methods, deduplicated by source
  identity. On a concrete receiver, definition continues to target its method.
- A single cached reverse conformance index serves implementation navigation and
  refactor analysis. No project filesystem scan per keystroke.
- Hover presents the visible contract and documentation; concrete members can
  indicate which requirement they satisfy without an unbounded implementation
  list. Completion and signature help use the same instantiated signature.
- Protocol annotations offer type-position completions only. Generic parameter
  completion respects scope and shows constraints when present.
- Requirement/member/type-parameter rename must account for conformance links,
  shadowing, overload/duplicate names, imports, and unrelated same-name methods.
  Unsafe or incomplete edits fail explicitly. Document closed-workspace limits.
- Add diagnostic codes only after checking the existing registry. Prefer
  structured failure data with a single renderer over manually assembled
  checker/editor messages. Do not allocate speculative code numbers in a plan.
- Example wording: `Panel does not implement Updatable: missing method update`;
  `Panel.update cannot implement Updatable.update: parameter dt accepts string,
  but the protocol permits number`. Final wording must reflect actual variance.
- Implemented a compiler-backed missing-protocol-method code action. It uses
  related requirement locations, accounts for imported contracts and inherited
  implementations, refuses conflicting same-named signatures, and generates
  `error("TODO: implement ...")` bodies so placeholders cannot silently return
  as though the method were implemented. Core, LSP, and installed VS Code
  application tests cover this workflow; other editor clients still need
  protocol-level qualification.

## 8. Performance, runtime, and compatibility gates

Capture same-host baseline before semantic changes: cold completion, warm
completion, incomplete edits, hover, signature help, format-on-save, dependency
edits, check/build time, and peak/retained memory. Use existing project replays
and synthetic wide/deep conformance and instantiation graphs. Record revision,
fixture size, toolchain, sample count, and p50/p95 rather than a single best run.

Proposed regression trigger: investigate a repeatable >10% p95 increase with
an absolute increase >5 ms for interactive requests. This is an investigation
threshold, not proof that smaller regressions are acceptable. Set hard budgets
from the measured baseline in P0; bound exponential instantiation and long
uncancellable work regardless of median latency. Repeat long-session memory
checks separately; existing ten-minute evidence does not establish leak freedom.

Keep `.lua` gradual and LOVE's existing generated catalog authoritative. Native
protocol conformance need not automatically include LuaDoc classes or LOVE
userdata; define future interop support explicitly and use adapters meanwhile.
Do not reinterpret existing nominal classes as structurally assignable.

Characterize protocols through optional/union values and mutable collections.
A shared behavior is not permission to remove nil checks or write sibling
instances into a container whose owner expects a particular subclass.

Runtime oracles should compare observable behavior against a hand-written Lua
reference: receiver identity, mutation, return tuples, exception locations,
evaluation count/order and initialization. Exact textual Lua snapshots alone
are insufficient. Test supported Lua/LuaJIT and LOVE separately; skipped runtime
or GUI tests remain pending, not passed.

## 9. Documentation and migration

Update the owning pages when functionality actually lands:

- `docs/classes.md`: implements, inherited conformance, relationship to extends.
- `docs/typing.md`: protocol views, assignability, generic identity/constraints,
  mutable-container rules, any and inference limitations.
- `docs/language.md`: grammar, contextual keywords and unsupported forms.
- `docs/modules.md`: type imports/re-exports and runtime erasure.
- `docs/annotations.md`, `docs/lua-interop.md`: exact LuaDoc generic/interop support.
- `docs/lsp.md` and diagnostics reference: navigation, signatures, actionable
  errors, refactor limitations and code actions actually implemented.
- A focused protocol guide if needed, linked through `zensical.toml`; avoid
  duplicating the full contract across multiple pages.
- Commented runnable `examples/features` sources and README, plus a small LOVE
  use case. Keep future syntax out of advertised runnable examples until ready.
- `CHANGELOG.md` under Unreleased; release summary only describes landed work.

Migration notes must state keyword/grammar changes, any stricter assignability,
annotation changes, and matching CLI/server/extension requirements. Do not
retrospectively edit published 0.6.1 behavior claims.

## 10. Verification and release checklist

During each behavior:

1. Run a focused regression that fails for the intended defect/unsupported case.
2. Run the owning test suite; run LSP tests whenever shared semantic or module
   data changes. Verify a second editor surface for inference changes.
3. Run runtime fixtures for lowering/dispatch changes; format twice and assert
   idempotence and comment preservation for grammar changes.
4. Review the diff for repeated semantic logic, unchecked any fallbacks,
   unbounded recursion/caches, accidental runtime imports, and incomplete edits.

Representative commands (use existing targets; add named fixtures, not shell
pipelines that hide failures):

```sh
cargo fmt --all -- --check
cargo test -p tua-core
cargo test -p tua-lsp
cargo test -p tua-cli --test cli
cargo clippy --workspace --all-targets -- -D warnings
make build-features
make lsp-features-smoke
make extension-check
make extension-test
make docs-build
make version-drift-check diff-check
make verify
make ci-check
```

On the current macOS host, the full workspace gate passes through plain
`make verify`: its Rust test target sets an overridable `RUST_MIN_STACK=16777216`
default while retaining parallel tests. Direct `cargo test` still uses Rust's
2 MiB default and can overflow in a deep linter regression; the CLI and LSP
worker stacks are 8 MiB. This is a harness setting, not a generic solver
requirement; keep recording explicit settings for reproducibility.
Editor/runtime commands depend on local tooling; record exact blockers and
repeat them in the appropriate release environment.

Before calling 0.7.0 ready:

- [ ] P0–P5 pass their acceptance gates; generic scope and completed G milestones
      are explicit, with no silently accepted unsupported forms.
- [ ] Negative soundness and mutation cases fail for the right reason.
- [ ] Cross-file saved/unsaved and all relevant IDE surfaces agree.
- [ ] Runtime output, source maps, initialization and hot reload are qualified.
- [ ] Fresh and warm-cache performance and memory evidence is recorded.
- [ ] Linux, Windows, Intel Mac and Apple Silicon trusted release gates pass.
- [x] A locally installed VSIX test on macOS covers protocol/generic typing,
      hover, completion, signatures, and selected definitions.
- [ ] Installed-editor implementation navigation and rename, and other
      supported operating systems, are qualified.
- [ ] Docs, examples, migration notes and version metadata match implementation.
- [ ] Focused independent code review covers binder identity, conformance,
      variance, cache invalidation and erasure; record unresolved findings.
- [ ] Packaged binaries/VSIXs pass archive integrity, version, checksum and
      executable smoke tests before publication is separately authorized.

## 11. Next implementation batch

The protocol, G1 generic-alias, G2 generic-function, and scoped G3
generic-class/bound slices are implemented. Concrete and transitive generic
parent specialization now shares the generic class instance path. Keep the actor/panel,
inherited-method, incompatible-signature, cross-file, and generic alias fixtures
as regression anchors. Remaining engineering work includes broader protocol
header malformed-edit recovery, broader cancellation-race permutations, and a
separate design for cross-module class inheritance. Delete/recreate recovery
is tested for each file in a three-module protocol re-export chain, and a
renamed dependency is tested through deletion, recreation, and compatible
contract restoration. Cross-module class extension is not supported by the
parser/lowering contract and remains an
explicit feature gap unless designed as its own change. Real installed VS
Code, LOVE/LuaJIT runtime, source-map/hot-reload, and cross-platform
qualification remain release gates. Do not treat completion of G3 as
completion of P0-P5 or R0.

### Final audit update — 2026-11-10

The final implementation audit found and fixed three boundary cases in
separate commits: protocol method parameters now require explicit non-empty
types (`17b6b412`); generic member lookup keeps known types after bounded full
alias expansion (`afd004ba`); and comments inside erased generic function
binders survive Lua emission (`7efaf29b`). Regression coverage includes exact
core/LSP diagnostics without conformance cascades, a 4,200-field generic alias
and an over-budget branching alias graph, recursive generic hover and class
`Self` consistency, and emitted-Lua parsing/runtime behavior.

The final completion stress follow-up is also resolved: field completion no
longer runs typed-domain documentation resolution for non-function fields.
This removes repeated receiver-alias resolution from large ordinary records
while preserving documentation on function-valued domain methods. The 4,200-
field core completion regression returns every field, and an imported-alias
stdio LSP replay completed in 0.22 seconds on the audit host. See AUDIT-028 in
`internal/audits/generics-polymorphism-0.7.0-final/REPORT.md`.

The remaining P2 signature-range gap now has installed-protocol LSP coverage
for parameter, return, required-argument-count, vararg, and return-count
mismatches, including inherited overrides and related requirement locations.
Pull-diagnostics coverage also exercises removing a conformance clause,
reintroducing it without its required method, and restoring that method in
unsaved consumer text. A separate test changes a contract through two
declaration-file re-exports, removes and restores the intermediate export,
and checks that consumer diagnostics track each state. These tests close those
specific cases; they do not claim exhaustive malformed-header recovery or
repeated multi-module filesystem rename/delete/create coverage. See AUDIT-029
and AUDIT-030 in the audit report.

At the AUDIT-028 checkpoint, `make docs-build`, the workspace tests, Clippy,
extension tests (180/180), `make benchmark-smoke`, packaging, and the installed
VSIX real-server suite passed on macOS arm64. That checkpoint's aggregate
`make verify` invocation was stopped by the sandbox's loopback restriction;
the loopback-dependent extension checks passed in a local-access retry. A
later full serial verification result is recorded in the boundary and
invalidation follow-up below. The exact VSIX at that checkpoint was
`editors/vscode/tua-0.7.0.vsix`, SHA-256
`8de57325a50b0adbbbe2045f1b83fe556dfdbc833a3b1d39c96a89a1db966f46`. The
updated package after AUDIT-028 is 8.71 MB with SHA-256
`977162265a6b6be4a8b46cbaa7dfd90f9491dc756de1b6fbea9118bbd760a0a3`.

Before the completion fix, an artificial 4,200-field imported-alias request
timed out. After the fix, the same stdio LSP workflow returned 11 matching
fields in 0.22 seconds; a core stress regression also returns all 4,200
members. Linux/Windows/Intel Mac, LOVE/LuaJIT, visual, long-session, and
second-client qualification remain open. The VSIX is ready for manual beta
testing on this host, not general release qualification.

### Boundary and invalidation follow-up — 2026-11-10

A follow-up added a per-character-prefix recovery matrix for protocol and
generic class headers, an unsaved contract change/restore cancellation test,
deletion/recreation coverage for each file in a three-module type re-export
chain, and rename followed by delete/recreate/contract restoration. These cases
pass in the core and LSP suites. A test-driven
review of the explicit complexity limit found that contiguous native `[]`
suffixes were erased before structural-depth counting, allowing arbitrarily
nested array types through the 64-level limit. The parser now checks native
array suffix depth before normalization, rejects the 65th suffix at its source
location, and ignores matching text inside strings/comments. Direct alias
expansion tests also pin the exact 4,096/4,097 work boundary and verify that
over-budget public resolution preserves the complete symbolic input. The
current core suite passes 1,898 tests (1 ignored), the LSP suite passes 433,
and focused Clippy/format checks pass. New regressions verify that protocol
members remain available while editing a protocol-bounded generic class with
an incomplete body, through core and actual LSP requests. The standard parallel
`make verify` gate passes with inherited stack/thread settings removed and
local loopback access; exact results are in
`internal/audits/generics-polymorphism-followup-2026-11-10.md`.
The final local manual-beta package at `editors/vscode/tua-0.7.0.vsix` was
installed and tested with its bundled compiler; its SHA-256 at that checkpoint
was `87c33c1711f5ce22b14ae4809a80719223534dda2a1194740af59f5a746e34eb`.

### Class-header completion follow-up — 2026-11-10

The protocol/generic editor audit found that type completion omitted protocol
names after `implements` and inside an unfinished multiline generic class
bound. Shared editor-only parsing now closes incomplete class generic lists and
missing class bodies for semantic queries; it leaves the user's source intact
so ordinary parsing still reports the unfinished declaration. Completion
recognizes these type positions without treating comments or ordinary Lua
identifiers as class headers. Core, parser, LSP, and installed VS Code
real-server regressions cover these cases, including imported protocol names.

After this follow-up, `cargo test -p tua-core` passed 1,902 tests with one
ignored and `cargo test -p tua-lsp` passed 436 tests. Strict core/LSP Clippy and
documentation generation passed. `make extension-package` passed with local
loopback access and built a macOS arm64 VSIX; the installed real-server suite
passed the header-completion checks. The current artifact is
`editors/vscode/tua-0.7.0.vsix`, SHA-256
`e7dfc55e3de1383a80336f4b2c07d81753d3ec257d506b926377dcce915464e9`.
LOVE/LuaJIT, cross-platform, visual, and long-session qualification remain
open.

### Incomplete protocol-member recovery follow-up — 2026-11-10

The analysis recovery path removes recoverable unfinished lines before it
closes an unclosed protocol body. A regression exercises a protocol with one
completed method, a partially typed next member, and a preceding module local;
the partial line is ignored in the temporary parse while the completed method
and local remain available. The delimiter helper alone does not promise to
repair partial member tokens; completion and project analysis apply their
incomplete-line recovery before using it. `cargo test -p tua-core
incomplete_protocol_member_recovery_keeps_prior_contract_and_module_bindings`
passes. The direct UTF-8 prefix tests still conditionally check strict parsing
because arbitrary prefixes can include incomplete member syntax.

### Protocol diagnostic cancellation permutation — 2026-11-10

An LSP regression queues incompatible, compatible, and incompatible protocol
contract edits before the dependent diagnostics pass runs. Pull diagnostics
must report the final incompatible parameter signature, proving that a stale
intermediate compatible snapshot cannot hide the latest error. This adds the
opposite final-state direction to the existing incompatible-to-compatible
stale-error test; filesystem rename/delete/recreate races and other scheduler
permutations remain open.

### Watched dependency cancellation follow-up — 2026-11-10

An LSP push-diagnostics regression queues consumer conformance diagnostics,
then changes the imported protocol on disk and sends a file-watcher event
before draining the queued pass. The watcher change cancels the stale work and
publishes current consumer diagnostics. Deleting the dependency replaces
conformance checking with a missing-import error; recreating it publishes the
new signature mismatch without a stale missing-import result. Rename combined
with repeated delete/recreate remains open.
