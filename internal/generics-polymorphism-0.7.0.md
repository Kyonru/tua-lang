# Tua 0.7.0 implementation plan: protocols, then generics

Status: active implementation plan, updated November 8, 2026. The initial
method-only protocol slice now parses, checks local and imported class
conformance, supports protocol-typed values, and erases contracts from Lua.
Protocol-specific IDE completion, hover (including requirement comments),
signature help, navigation, references, and rename now have core/LSP request
coverage. A real `tua lsp` process probe confirms invoked completion survives
an incomplete cross-file protocol edit; remaining semantic surfaces and full
runtime/editor qualification are still in progress. Native generic syntax and
inference are not implemented. Existing single inheritance is retained. This
document is not a release announcement.

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
- `parser/dialect.rs`: native generic declarations/applications are rejected.
- `parser/model.rs` and `parser/doc.rs`: LuaDoc generic metadata exists;
  `docs/annotations.md` correctly says it does not provide generic inference.
- `types/model.rs`: types and aliases currently have no declaration-owned
  generic binder/instantiation representation.
- `types/assignability.rs`: function variance and structured failure paths
  exist; mutable arrays currently compare element types covariantly. Do not
  copy that rule into mutable generic containers without a soundness decision.
- `classes.rs`: protocol declarations and class `implements` clauses now retain
  source spans and erase through class lowering. Local and imported
  method-contract checks are implemented; protocol completion, hover,
  signature-help, and navigation have initial cross-file core/LSP coverage, but
  the protocol IDE slice is incomplete.
- `crates/tua-core/src/analysis/host.rs`: project module/type indexes reuse
  last-good parsed and semantic facts when an open source file becomes invalid,
  so type-only protocol imports stay available during edits.
- LSP already advertises implementation navigation. Extend its shared query;
  do not add a second unrelated implementation lookup.

Evidence as of this update: `cargo test -p tua-core protocol` passes all 12
protocol-filtered tests. Core and LSP editor requests cover imported protocol
completion, hover, signature help, definition, and implementation navigation;
the incomplete-edit completion case first establishes last-good project facts.
Both full package suites pass with stack-sensitive serial settings recorded in
`internal/audits/devex-space-invaders-0.7.0/REPORT.md`. That report contains a
five-module game exercise, generated-Lua smoke test, exact limitations, and
remaining verification gaps. No native generic alias, function, or class
implementation has landed.

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
- Revisit mutable array/record widening independently: characterize existing
  write-through-alias behavior, choose compatibility versus a checked read-only
  view, and document the decision. Do not silently change old array code as a
  side effect of introducing generic classes.
- Reuse shared binder/substitution machinery for supported LuaDoc `@generic`
  forms; unsupported annotation forms must remain clearly documented.

### Generic design decisions (G0)

The first native generic release uses a narrow, type-only surface:

```tua
type Box<T> = { value: T }
type Result<T, E> = { value: T | nil, error: E | nil }
local box: Box<number> = { value = 4 }
```

- Only type aliases accept generic parameters in G1. Parameters are ordered,
  required, unbounded, and non-variadic. Generic classes and function
  declarations remain rejected until their own milestones land.
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
- G2 introduces first-order generic function binders and call inference using
  the same IDs and substitution engine. Its solver begins with constraints from
  declared argument/expected parameter pairs, uses assignability constraints
  for repeated occurrences, and reports unresolved required variables rather
  than widening them to `any`. A body must type-check under symbolic binders
  before any call is considered. Callback context and return-only variables
  require explicit ambiguity diagnostics until a sound inference rule is
  implemented.
- G3 generic class parameters are invariant because instances are mutable.
  Subtyping does not widen `Pool<Derived>` to `Pool<Base>`. Protocol bounds are
  checked through existing conformance facts, while the constructed value keeps
  its concrete class identity and substituted inherited members.
- Initial implementation limits will be explicit constants with boundary
  tests: maximum type nesting, recursive alias expansion work, per-request type
  instantiations, and retained memoized instances. Cancellation and caches must
  include source/module revision plus declaration identity and arguments.
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
| `semantic.rs`, module index/link, `analysis` | Snapshot-owned project queries, conformance reverse index, generic instantiation memoization | No request-time project scans or stale conformance after edits |
| `check` | Requirement declaration validation and implementation diagnostics | Report real errors once; defer project-dependent facts during fast diagnostics |
| `emit/lowering`, `emit` | Validated erasure operations and source provenance | Runtime-equivalent output; no conflicting edits or lost comments |
| Core IDE query modules | Resolve protocol members and instantiated signatures through shared semantics | Hover, completion, signatures, inlays, navigation agree |
| `tua-lsp` | Existing query adapters, capabilities, UTF-16 ranges, cancellation | No protocol or generic type checker inside handlers |
| VS Code | Grammar, snippets, completion presentation, documentation, tests | No separate semantic parser in extension code |

Prefer focused modules for protocol facts and substitution over expanding the
already broad class/environment code with many special cases. Exact module
names are implementation choices, not a reason to introduce a new crate.

Cache keys must include declaration identity, relevant source/module revision,
and type arguments. Bound recursion, instantiation count, retained memory, and
cancellation work. Share immutable facts across snapshots. Test both fresh
analysis and a warm cache after a dependency changes.

## 5. Milestones and commit-sized work

Effort is relative complexity, not a calendar promise: S = localized change,
M = several connected modules, L = cross-cutting semantics/editor work.
Each row may require multiple behavior-sized commits. Do not split a behavior
from the tests and current documentation that prove it.

| ID | Scope and dependencies | Work and completion gate | Status | Effort |
| --- | --- | --- | --- | --- |
| P0 | Contract and baseline; first | Settle syntax/semantics, diagnostic locations, namespace and receiver rules. Add positive/negative fixture specifications and characterize mutable-container risk. Existing inheritance regression stays green. | Partial: method-only protocols, explicit nominal conformance, colon dispatch, and no-runtime-erasure rules are in code. Receiver edge cases and mutable-container interactions still need characterization. | M |
| P1 | Protocol syntax and identities; P0 | Parse declarations/conformance clauses, scope type names/members, preserve docs and spans, recover edits, format and erase safely. Unsupported use must never be silently accepted. | Partial: local and exported protocol declarations, class clauses, spans, type identities, erasure, and method-comment parsing are covered. Malformed-edit recovery and formatter behavior need dedicated coverage. | L |
| P2 | Local conformance and dispatch; P1 | Share method compatibility, inherited methods and conformances, multiple obligations, protocol assignments/calls, and structured errors. Invalid signatures fail at the correct span. | Partial: local/imported conformance, protocol-typed values, inherited method satisfaction, and missing/incompatible method tests pass. Multiple obligations, inherited conformance edge cases, variance and exact diagnostic slots need broader verification. | L |
| P3 | Modules and invalidation; P2 | Type imports, exports/re-exports, declaration-file contracts, cycles and missing modules. Unsaved contract/member edits refresh dependents without transient false errors. | Partial: exported protocols, named type-only imports, and project diagnostics for missing/incompatible imported methods in a separate implementing file work, including when `implements` is the import's only use. Saved and unsaved contract edits refresh those diagnostics. Re-exports, declaration modules, cycles, and broader dependency-edit freshness remain open. | L |
| P4 | Complete protocol IDE slice; P3 | Member completion, hover/docs, signatures, inlays, definitions, implementations, references/rename, tokens, type explanations, actionable diagnostics. Verify real LSP responses and installed VS Code. | Partial: core and in-process LSP requests cover completion after a typed method prefix, incomplete edits with last-good project facts, hover signatures and protocol requirement comments (with the implicit receiver hidden for colon calls), signature help, definition, implementation navigation, references, and rename across type-only imports. A real `tua lsp` process probe confirms invoked completion after an incomplete imported-protocol edit. Semantic tokens classify protocol types as interfaces and requirements/calls as methods. Inlay hints and type explanations preserve imported protocol member types. Installed VS Code and runtime qualification remain open. | L |
| P5 | Usable protocol examples/docs; P4 | Add runnable game/UI and negative examples, runtime/source-map/hot-reload qualification; preserve adapters for Lua interop. | Partial: class/typing/module docs and a feature-tour sample now explain the shipped slice; an audit game build ran under system Lua with a LOVE shim. LOVE/LuaJIT, source maps, hot reload, editor UX, and visual behavior remain unverified. | M |
| G0 | Binder/substitution design; after P5 | Specify generic inference, recursive types, variance, first-class functions, contextual typing, limits and error policy; reuse protocol bounds. No unused framework-only abstraction. | Decisions recorded above: native alias syntax, binder identity, substitution semantics, arity/error behavior, inference boundaries, erasure, mutability variance, and resource limits. G1 implementation remains open. | M |
| G1 | Generic aliases; G0 | Scoped binder IDs, arity checks, type applications, bounded capture-avoiding substitution, recursion/occurs handling, imports and IDE display. Connect machinery to its first consumer. | Open. | L |
| G2 | Generic functions; G1 | Universally checked bodies, per-call inference, returns/callbacks, independent instantiations, navigation and actionable failures; align supported LuaDoc forms. | Open. | L |
| G3 | Generic classes and bounds; G2 | Constructor inference, instance members, inherited substitutions, `Self`, invariant mutable containers and protocol constraints. Retain runtime/class identity and editor parity. | Open. | L |
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
- Consider an implement-missing-methods code action after semantic correctness.
  Generated stubs must clearly be unfinished and never claim a working return
  implementation. This is polish, not a prerequisite to valid protocols.

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

On the current macOS host, prior full debug tests required
`CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1` for an existing
stack-sensitive test. Record those settings if used; do not present them as a
new generic solver requirement or hide a new recursion regression behind them.
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
- [ ] Real installed VSIX tests cover typing protocols/generics and navigation.
- [ ] Docs, examples, migration notes and version metadata match implementation.
- [ ] Focused independent code review covers binder identity, conformance,
      variance, cache invalidation and erasure; record unresolved findings.
- [ ] Packaged binaries/VSIXs pass archive integrity, version, checksum and
      executable smoke tests before publication is separately authorized.

## 11. Next implementation batch

The first local protocol batch and an exported-protocol cross-file batch are
landed. Keep the actor/panel, inherited-method, incompatible-signature, and
cross-file fixtures as regression anchors. G0's binder and substitution
decisions are recorded above; G1 alias implementation remains open. Continue
with P3 freshness tests for saved and unsaved edits, then the remaining P4
protocol IDE surfaces and P5 runtime/source-map/hot-reload qualification. Do
not treat this partial protocol implementation as completion of the 0.7.0 plan.
