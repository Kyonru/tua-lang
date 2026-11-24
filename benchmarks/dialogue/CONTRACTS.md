# Dialogue M0: contracts and compatibility decisions

Status: M0 decision record, 2026-11-23. Implementation baseline: Tua `46a70a89`;
planning baseline: `646eebfe`. The runtime and grammar are unchanged by M0.
**Future** sections below constrain subsequent milestones; they are not new APIs
available in this commit. See [README](README.md) for measured evidence and commands.

## Audit: current ownership and integration points

| Surface | Source of truth | Finding / next integration |
| --- | --- | --- |
| Authoring | `crates/tua-core/src/dialogue_asset/mod.rs` | Version-one parser retains text IDs, decoded prose, labels, character imports, events and source spans. Delays are validated/normalized finite numeric strings. |
| Emission | `crates/tua-core/src/emit/dialogue.rs` | Emits Lua data plus shared-character requires. Choice IDs survive; line text IDs are not consistently emitted as explicit fields. Internal `@line:` / `@choice:` addresses are lowering details. |
| Typed API | `crates/tua-core/src/dialogue.rs`, `vendor/runtime/tuadialogue.contract.tua` | One bundled declaration feeds the constructor, asset exports and editor types. Optional `start` arity uses a function declaration; callback type annotations cannot replace it without losing that arity fact. |
| Runtime | `vendor/runtime/tuadialogue.lua` | Validates/copies selected document fields; snapshots recursively copy current presentation. Unknown legacy input fields are ignored. No document/checkpoint version field exists. |
| Analysis/editor | `analysis/database.rs`, `analysis/snapshot.rs`, `analysis/dialogue_queries.rs` under `tua-core/src` | Cached parser models, project-wide text IDs and typed character exports serve diagnostics/completion/hover/definition. No second semantic resolver belongs in LSP or the extension. |
| Project lifecycle | `crates/tua-cli/src/build/mod.rs`, `build/watch.rs`; existing dialogue CLI/LSP tests | Shared emission, source maps, cache and dependency invalidation. Data-only imports do not bundle the player. Invalid edits preserve last-valid output. |
| Localization | `crates/tua-core/src/i18n.rs`, `docs/localization.md` | Catalog index already validates literal keys/placeholders and distinguishes text/plural `string` results from existing array-shaped “dialogue” entries. Arrays are not this graph format. |
| Signals | `crates/tua-core/src/signal.rs`, existing imported signal tests | Declarations can describe payloads without executing handlers. The new nested-export probe exposes a validation boundary; reuse the declaration facts, not an assumption that every bus reference retains its schema. |
| Saves | `crates/tua-core/src/save.rs`, `vendor/runtime/tuasave.lua`, `docs/save-contracts.md` | Typed snapshots, historical input schemas, validation and migration already exist. Dialogue should export plain checkpoint data and supply a recipe, not another persistence engine. |

The current player states are `idle`, `awaiting_advance`, `awaiting_choice`,
`completed`, `cancelled`, and `disposed`. It has no implicit control-flow loop,
context, local variables, registered action runner, localization resolver or
checkpoint API. `generation` and `visit` are runtime lifecycle tokens, not durable
cross-process IDs. The public contract exposes methods; Lua implementation fields
and the internal `enter` method are not a supported persistence interface.

## D1 — Stable identity

**Future metadata contract (M1/M4/M8):**

| Identity | Decision |
| --- | --- |
| Document | Explicit author-controlled logical `documentId`, unique in a project/content namespace. Do not derive durable identity from an absolute path, build location, prose hash or module cache address. |
| Text/line | Existing `// @id` becomes explicit `textId` metadata on emitted line nodes and presentation snapshots. Preserve project-wide uniqueness and spelling across translation/prose edits. |
| Choice | Existing option text ID remains both choice identity and localization key. Array order and translated label never identify a selection. |
| Entry | Authored label remains a start target. A checkpoint records stable document/text/choice identity and compatible content version, not solely the current internal node address. |
| Action | M4 introduces an explicit stable action ID, scoped by document and line identity. Repeated event names are allowed; event name, timestamp and declaration index cannot identify a consequential action durably. |
| Character | Module/export references continue to resolve authoring. If later checkpoint/history data needs durable character identity, add explicit character metadata in M5 rather than treating a display name or portrait path as identity. |

M1 may add metadata without changing existing flow. The exact authoring spelling
for document identity is an M1 parser decision; these data-field names and identity
rules are settled here, not claims of currently accepted directives.

Legacy scripts without explicit document/action identity remain playable. They
are not automatically eligible for checkpoints that need those identities. File
moves and prose edits must not silently rewrite IDs; intentional renames require
catalog/reference updates and, when persisted, explicit content migration.

A content revision is separate from document identity. M8 must validate a
compatibility revision/manifest; byte hashes alone cannot decide whether a saved
state is compatible with an edited graph.

## D2 — Three independent version boundaries

| Boundary | Current | Future rule |
| --- | --- | --- |
| Authored syntax | Required `@dialogue 1`; other headers fail with TL5012 | Keep v1 readers. Additive syntax may use previously rejected directive positions. Changed interpretation of accepted v1 text requires a new authored version or explicit migration. |
| Emitted/structured schema | Unversioned `entry` + `nodes`, designated **legacy schema 0** in this record only | M1 introduces explicit `schemaVersion = 1` for identity-aware documents. A newer player accepts legacy 0 through its existing behavior, validates recognized schema 1, and rejects unknown explicit versions before traversal. |
| Checkpoint schema | Not implemented | M8 begins at `checkpointVersion = 1`, with separate document/content compatibility fields. Restore validates all three; a migration must be explicit and validated. |

No `schemaVersion` field is added by M0. In particular, the old runtime ignores
unknown fields: adding a version number alone cannot make an old player reject a
future document. Forward compatibility is **not** promised. M1 must test and
document minimum runtime requirements/capability checks for versioned documents,
including project-provided runtime replacements; never silently lower new
semantics into an old reader that ignores them.

Legacy schema 0 keeps its selected-field copy behavior. New versioned schemas
reject unknown semantic fields to catch misspellings, with explicit schema
upgrades for new semantics. This is not a generic executable extension mechanism.
Parsing/checking must reject unsupported authored versions before generating any
partial output. Runtime validation errors retain useful document/source identity.

## D3 — Ownership and lifecycle

| Value | Policy |
| --- | --- |
| Caller-supplied document | Preserve defensive copying and validation by default. Do not substitute compile-time `Readonly` for runtime ownership. |
| Presentation snapshot / returned event | Caller-owned copy; mutations cannot affect the player or another session. |
| Future context facts | Host owns gameplay state. The player validates/copies an explicitly supplied serializable facts snapshot; it never keeps arbitrary entity objects or calls methods while evaluating expressions. Refresh happens at an explicit context-update boundary, not by observing arbitrary alias mutation. |
| Future local defaults | Copy per player/start. `start` resets defaults; looping within a session preserves locals. Cancel permits inspection until restart; dispose releases references. Restore is a separate operation, not a disguised `start`. |
| Future handlers | Host-owned functions/services, separate from serializable context/locals. Runtime stores only the explicit binding needed by that player and releases it on disposal. |
| Future event arguments | Capture validated values for a line visit when that line is entered. Do not let later context mutation silently change a queued payload. An action requiring fresh authority must revalidate it in the host. |
| Future transcript | Bounded, copied records containing identifiers and displayed text; never retain the whole context or GPU/audio objects. |

The M2 `Readonly` view is an authoring aid. It is not runtime freezing: a host can
still mutate its own aliases. Shared immutable documents remain a later measured
optimization, limited to validated compiler-owned representations with no mutable
caller aliases; do not change default ownership based on type annotations alone.

Define choice availability from the latest accepted context snapshot. M3 must
revalidate a submitted choice before accepting it. A context refresh may update
available replies without restarting the current line or replaying its events.

A locale change is presentation refresh, not a new visit. M1 preserves selected
IDs and fired-event state; its default will replace the line's text and finish
its reveal immediately rather than replay a partial reveal or its effects. Hosts
wanting a new utterance must explicitly start a new visit. The exact presentation
notification API is deferred to M1; the no-replay rule is fixed.

## D4 — Bounded work and observable failure

At the M0 checkpoint, enforced limits were: 8 MiB per textual source, the shared
128 MiB/20,000-file project limits, and 64 authored/runtime events per line.
The M0 runtime had no arbitrary automatic transitions or pending actions. The
following table was its proposal; the M3 decisions below supersede the implemented rows.

| Future limit | Initial default / ceiling | Milestone and behavior |
| --- | --- | --- |
| Automatic transitions per update/resume | 256 / 4,096 | M3/M7: exceed → deterministic `faulted` state and `transition_budget_exceeded`, no spinning or partial action replay |
| Condition AST nesting | 32 / 128 | M3: compile-time diagnosis; validate dynamically supplied documents too |
| Context/local data nesting | 32 / 128 | M2: reject cycles/over-depth on ingestion, before retaining data |
| Pending asynchronous actions per player | 64 / 256 | M7: reject admission with `pending_action_limit`; do not discard an existing action |
| Completions processed per update | 64 / 256 | M7: process FIFO, leave remainder queued for later updates; automatic-transition budget still applies |
| Transcript records | Disabled by default; opt-in 200 / 10,000 | M8: evict oldest record at capacity; zero disables recording |

Limit options are finite integers validated at construction; ceilings bound
configuration mistakes. These are starting contract choices, not performance
claims. Qualify them on adversarial fixtures before their milestone lands; any
adjustment must update the decision record and tests together.

Future faults carry a code, document/node/action source identity and a message,
with no secret/context payload dump. `faulted` stops automatic work and remains
inspectable; `cancel`, `start` and `dispose` provide explicit recovery/cleanup.
Do not silently skip an invalid branch or retry a consequential action. Stale
asynchronous completions are ignored using per-player generation/visit/action
identity, with bounded optional tracing. Host handler exceptions become explicit
action failures under M7 policy, not indefinite waits.

Keep current programmer-input assertions and v1 method return behavior unchanged.
Do not retrofit the new fault states into legacy playback during this milestone.

### M3 qualification of automatic work

M3 implements 256 automatic edges per transition/refresh by default, configurable
from 1 to 4,096. It faults with `transition_budget_exceeded`; restart/cancel/dispose
are explicit recovery. AST depth is a fixed **32**, rather than a configurable
ceiling of 128: both authored and runtime inputs reject excess nesting before
execution. Expressions also cap at 256 tokens/instructions (8 KiB authored text).
Bounded loops and rejection cases are executable regressions; these limits are not
performance guarantees. Context data uses the existing Save.shape 32-level/100,000-entry
validation, rather than a new dialogue-specific serializer.

A no-choice fallback is mandatory for conditional groups and runs when no visible,
enabled choice remains. Refresh follows it as a new visit; otherwise refresh keeps
visit/generation and fired events. Ordered comparisons are numeric; absent operands
return false. Equality supports scalar/nil tests; boolean operations short-circuit
and treat nil as false without Lua's numeric/string truthiness. The compiler checks
all paths, including unreachable operands. Locale changes do not reevaluate flow.

## D5 — Imported-type probe results and API implications

The `fixture/probes/` files are minimal reproductions using **existing** APIs.
They are outside the compiled source root and are checked individually by the
baseline runner. See `results/baseline.json` for exact diagnostics/compiler identity.

- `Readonly` write rejection and known-field type checking are working guarantees;
  the CLI regression asserts them through a module re-export.
- Unknown context-field reads currently pass. M2's script resolver must validate
  paths against the declared context shape; do not assume a general `.tua`
  property read necessarily diagnoses a typo.
- A declared bus nested in a re-exported record currently accepts a wrong payload.
  Keep this reproduction for M4's schema-resolution work. The direct-import probe
  rejects the same invalid payload, distinguishing this from ordinary exported bus declarations.

Known permissive cases are recorded as gaps, not success criteria that lock in a
bug. A future compiler fix may make them fail checking and should update the
recorded results; the benchmark runner deliberately does not assert that a gap
must remain permissive. No new generic syntax or module-wide `any` workaround is
justified by these probes. Current core regressions already cover imported
readonly completion/hover and direct signal payload inference; new DSL reference
validation must also share that semantic path.

## D6 — Minimal M1 proposal

1. Extend the compiler-owned document facts with explicit line text IDs and
   logical document identity; emit additive typed metadata through schema 1.
2. Map existing line and choice IDs directly to existing catalog keys. Keep
   source prose as the default text and extraction seed, not an array-shaped
   `I18n.t` dialogue entry and not a second catalog engine.
3. Reuse `CatalogIndex` placeholder sets and scalar text/plural result rules.
   Missing/extra placeholders and incompatible entry shapes become authored
   diagnostics. `%{name}` values are plain text, never re-parsed as markup.
4. Preserve the existing enabled-i18n configuration gate and fallback chain.
   Per-player locale selection uses the existing per-call locale capability,
   without changing the global locale or unrelated players.
5. Character plain names stay valid. Optional localized-name metadata belongs in
   the shared character contract; a name computed once when requiring a character
   module cannot by itself support live locale switching.
6. Use a host-neutral resolver adapter for headless playback and optional `I18n`
   integration. Keep unresolved/dynamic catalog entries explicitly dynamic; never
   evaluate catalog Lua in the compiler/editor. Runtime host code may execute its
   normal imported catalogs.
7. Extend localization coverage/navigation with dialogue text IDs, and compile
   every guide example. Re-run the M0 fixture suite and matching baseline; report
   added costs separately from any unrelated optimization.

The concrete constructor/options/resolver spelling is intentionally deferred to
M1's small typed probe. This record settles identity, ownership, fallback and
version boundaries without falsely advertising an unimplemented API.

## D7 — Checkpoints and consequential effects

M8 must not infer durable effect delivery from `generation`, `visit`, event array
position or an in-memory fired bit. Unknown legacy events are non-resumable across
an uncertain effect boundary unless the host supplies an explicit policy.

Use stable action identity plus a host-owned durable execution/idempotency key.
Saving the dialogue after giving a reward has a crash window; saving before the
reward has a different one. Exactly-once consequences require an atomic host
transaction or idempotent handler. Dialogue core cannot promise either merely by
serializing its own state. Save bindings serialize checkpoint data only; they do
not acquire graphics/audio handles or silently replay pending non-resumable work.

## Compatibility gate

`crates/tua-cli/tests/dialogue_contracts.rs` copies the preserved fixture into an
isolated project and checks/builds its real emitted Lua. It exercises legacy
inline characters and consecutive lines, shared portraits, both event clocks,
choice loops, two independent players, mutable input/snapshot isolation,
restarts/cancellation/disposal, wrong imported types, unused player omission,
project runtime replacement, and unsupported authored versions.

The fixture is the stable M0 compatibility seed. Extend it in later milestones
without rewriting away old syntax or expected outcomes. Existing dialogue parser,
LSP and build/watch tests remain relevant; this small fixture does not replace
those suites or establish installed-editor usability.
