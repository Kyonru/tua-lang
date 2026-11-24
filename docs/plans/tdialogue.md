# Tua dialogue: implementation plan

Status: M0–M3 complete; M4–M10 planned. Baseline: `46a70a89` (portraits and timed events)
and `2e179341` (dialogue documentation). This plan combines the remaining initial
workstream with the expanded dialogue improvements. It does not claim those
improvements are implemented or assign them a released version.

The [dialogue guide](../dialogue.md) documents current behavior. Historical
implementation and verification checkpoints are preserved at the end of this
file; their old “pending” and branch-status statements are not the current backlog.

## Outcome

Support story conversations, NPC interactions, tutorials, radio dialogue and
short announcements through the same typed, renderer-independent player.

Authors should be able to share characters, translate dialogue, branch on typed
state, trigger declared actions, select expressions and voice, and resume safely.
Game authors retain rendering, input, assets, pause policy and persistent gameplay
state. The editor uses the same semantic facts as checking and compilation.

## What already exists

Do not implement these again:

- `.tdialogue` parsing, stable project-unique text IDs, labels, lines, choices and
  explicit endings; generated Lua data and source provenance.
- Typed `Dialogue` builtin, automatic runtime inclusion only when used, explicit
  imports and project runtime replacements.
- Independent players with explicit advance/choose, owned documents/snapshots,
  cancellation, disposal, generation and visit tokens.
- Shared typed character modules, name/portrait metadata, character completion,
  hover, navigation, unsaved overlays and dependency invalidation.
- Timed string-payload events with `shown` and `revealed` anchors, once-per-visit
  delivery, deterministic due-event ordering and pending-event cancellation.
- Optional per-line hold/fade hints; rendering, reveal and expiry remain host-owned.
- Headless examples and a production game integration with separate bounded
  conversation/message queues. `BoundedQueue` itself already exists.

## Consolidated scope

| Improvement or carried-forward item | Delivery milestone |
| --- | --- |
| Localization IDs/catalog integration, translation coverage and placeholders | M1; typed bindings in M2; styled placeholders in M6 |
| Typed dialogue context and serializable local variables | M2 |
| Conditions, conditional choices and branches | M3 |
| Typed event declarations and registered commands/actions | M4; waiting actions in M7 |
| Character expressions and portrait variants | M5 |
| Text emphasis, pauses, reveal speed and pronunciation metadata | M6 |
| Voice, waits, cancellation and asynchronous host actions | M7 |
| Save/resume, transcript and prevention of accidental effect replay | M8 |
| Label/target completion, rename, formatting, diagnostics and preview | M9, with semantic/editor support in each earlier milestone |
| Native examples: panel, speech bubble and non-pausing radio message | M10 |
| Deterministic execution limits and independent-session tests | M0, M3, M7 and the shared verification gate |
| Full upstream and consuming-game qualification | M10 |

The initial plan's portrait delivery and automatic builtin integration are
complete. “Dedicated script editing” is partially complete: highlighting and
character queries exist; the remaining capabilities are explicitly listed in M9.

## Design constraints

1. **One semantic implementation.** `tua-core` owns parsing, symbols, types,
   validation and source mapping. LSP/CLI/extension adapters consume those facts.
2. **No project execution during analysis.** Inspect declarations and literal
   metadata; never run handlers, predicates, catalogs or asset loaders for tooling.
3. **Ordinary typed APIs first.** Reuse `.tua` records, imports, existing generics,
   signals and helper contracts. Demonstrate compiler gaps with minimal imported
   fixtures before proposing general language changes.
4. **No arbitrary Lua inside dialogue files.** Conditions and value references
   form a small validated expression subset; effects go through declared host
   actions. Do not add an unrestricted interpreter or plugin registry.
5. **Host ownership stays explicit.** Core playback has no required LÖVE globals,
   filesystem, graphics, audio or wall clock. It does not decide to pause a game.
6. **Preserve compatibility.** Existing version-one scripts, string events,
   structured documents, `start/advance/choose`, runtime replacement and unused
   helper omission remain covered. Do not silently reinterpret existing prose.
7. **Bound new work.** Limit expression depth, automatic transitions, queued work,
   histories and input sizes. Report exhaustion without freezing an update.
8. **Use emitted Lua as evidence.** Preserve Lua 5.1/LuaJIT compatibility and verify
   the real bundled runtime, not only checker acceptance or disposable prototypes.
9. **Keep content portable.** Use original or appropriately licensed public example
   assets; never copy private game code, dialogue or artwork into Tua fixtures.

## Reuse existing Tua capabilities

| Existing feature | Reuse or integration work |
| --- | --- |
| [Localization](../localization.md) | Catalog index, `%{name}` placeholders, locale/fallback rules, diagnostics and Localization Explorer. Connect dialogue IDs; do not build another localization runtime. |
| [Signals](../signals.md) | Investigate existing declared-event schema and imported payload inference for dialogue event contracts. Do not force every host to use a signal bus. |
| [Audio](../audio.md) | Optional voice adapter over `audioSystem` or raw host audio. Keep subtitle and voice locale/fallback selection explicit. |
| [Sprites](../sprites.md) | Optional expression/animation adapter over `spriteSheet`, with per-player animation ownership. Core retains asset identifiers, not GPU objects. |
| [Typed saves](../save-contracts.md) | A JSON-shaped dialogue checkpoint and optional `Save` recipe. Core checkpoint export/import remains independent of LÖVE and disk. |
| Existing build/watch/editor infrastructure | Source maps, module invalidation, unsaved overlays, rename edits, diagnostics, cache and last-valid output. |

## Implementation sequence

| Order | Milestone | Dependencies | Reviewable result |
| --- | --- | --- | --- |
| 0 | M0 — Contracts and compatibility | Existing implementation | Documented data/API decisions, compatibility fixtures and baseline measurements |
| 1 | M1 — Localization | M0 | Localized lines/choices and coverage tooling |
| 2 | M2 — Context and locals | M0; integrates with M1 | Imported typed context and placeholder bindings |
| 3 | M3 — Conditions and choices | M2 | Deterministic branching and disabled/hidden replies |
| 4 | M4 — Typed events/actions | M0, M2 | Declared names, typed payloads and stable action identity |
| 5 | M5 — Expressions | M0 | Reusable character variants and editor assistance |
| 6 | M6 — Text presentation | M1, M2 | Renderer-neutral text spans and reveal metadata |
| 7 | M7 — Voice and waits | M4; integrates with M1/M6 | Cancellable host actions, voice references and waiting state |
| 8 | M8 — Checkpoints and history | M1–M7 contracts settled | Validated resume policy and bounded transcript |
| 9 | M9 — Authoring tools | Shared facts from M1–M8 | Formatter, rename, navigation, flow diagnostics and preview |
| 10 | M10 — Showcase and qualification | M1–M9 | Native examples, documentation and consuming-game evidence |

This is a delivery order, not permission to postpone basic typing/editor support
until M9. Every new reference or field needs diagnostics, completion, hover and
navigation where relevant in its own milestone. M9 adds tools that span the whole
format. Build the showcase skeleton early and extend it after each milestone.

## M0 — Settle contracts and preserve compatibility

- [x] Audit the parser model, emitted document, bundled contract, runtime,
  localization index, signals, saves and existing editor queries.
- [x] Record exact compiler/runtime revisions and current headless, rendered,
  allocation and editor workloads. Preserve representative version-one fixtures.
- [x] Define stable document, line/text, choice and action identities. Current
  generated node IDs and event-array positions must not become accidental save IDs.
- [x] Decide authored format versioning separately from emitted-document and
  checkpoint versions. Diagnose unsupported versions clearly; retain old readers
  or provide an explicit migration path for any incompatible syntax.
- [x] Define context, local-state, event and presentation ownership. Specify which
  snapshots are copied and which state may be shared immutably across players.
- [x] Define configurable limits and observable failure states for future
  automatic transitions, nesting and asynchronous work. Preserve the existing
  64-event-per-line validation unless an explicit compatible change is justified.
- [x] Write small imported-type probes before selecting new APIs. All illustrative
  syntax/API proposals in implementation reviews must be labeled as proposals
  until parser, runtime and editor tests establish them.

**Acceptance:** old scripts and structured inputs still check, build and execute;
independent players and caller-owned data remain isolated; unused runtimes stay
absent. A short decision record settles each milestone's public contract before
its implementation expands across compiler and runtime surfaces.

### M0 completion record

Completed with an audit/decision record and preserved compatibility evidence;
no production grammar or runtime behavior changed.

- [Contracts and integration audit](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/CONTRACTS.md): stable identity, independent version boundaries,
  ownership, future work limits/faults, checkpoint consequences and the minimal M1 proposal.
- [Baseline guide and results](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/README.md): exact compiler/runtime and fixture/harness provenance,
  separate CPU/allocation runs, native JIT-off/on reference rendering, and actual
  LSP completion/hover/definition/unsaved-edit workloads.
- `crates/tua-cli/tests/dialogue_contracts.rs`: old scripts and structured data,
  two players, mutable input/snapshot isolation, lifecycle, imported types, unused
  runtime omission, project replacement and unsupported authored versions.
- Probes establish two explicit integration gaps for M2/M4: unknown context-field
  reads pass checking, and a declared signal bus nested in a re-exported record
  accepts an invalid payload. Directly exported buses reject that payload.
  Reproductions are retained; M0 does not silently widen types or promise these
  gaps are fixed.

At the M0 checkpoint, identity/schema/options names and fault states in the
decision record were **design contracts**. M0 retained the existing version-one behavior. M1 below implements its
localization and emitted-schema contracts.

## M1 — Localization and translation coverage

- [x] Map stable line and choice text IDs to the existing catalog index. Decide
  how source prose supplies default text and how catalog extraction/updates work
  without overwriting translator edits.
- [x] Add placeholder parsing/validation using the existing `%{name}` convention
  where compatible. Do not conflate a translated dialogue line with the existing
  `I18n.t` string-array result used for other dialogue catalog entries.
- [x] Resolve line and choice translations with the established locale fallback
  chain. Provide per-player locale selection without mutating unrelated players.
- [x] Define behavior for switching locale during reveal or a choice: preserve
  stable selection/line IDs and define whether text is re-presented; never replay
  gameplay effects merely because a translation changes.
- [x] Validate missing/extra placeholders, entry shape, missing keys and source
  drift. Integrate dialogue coverage and source navigation into the existing
  Localization Explorer. Define warning/error policy for incomplete translations.
- [x] Cover character display-name localization without requiring every character
  name to be a catalog key. Plain string names remain valid.

**Acceptance:** two players can show different locales; fallback and missing-entry
behavior are deterministic; translated choices keep their stable IDs. Tests cover
plural values, escaping, Unicode text, missing/extra arguments, changed catalogs,
unsaved edits and runtime replacement. The example includes two locales and a
clearly incomplete translation to demonstrate coverage diagnostics.

### M1 completion record

Implemented in `e773bec4`, with measured allocation cleanup in `370d0725`.
The [dialogue guide](../dialogue.md#localize-lines-choices-and-character-names)
and [English/French example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue-localization)
cover source fallback, scalar/plural placeholders, shared character names,
independent locales, no-replay presentation refresh and schema compatibility.

The existing Localization Explorer now includes source coverage/navigation;
read-only extraction preserves translator edits. Watch and unsaved editor changes
use shared analysis. See the [M1 verification and measurements](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/M1.md)
for exact clean compiler identities, validation, remaining costs and native captures.
Installed-editor UI and large-catalog performance remain unqualified; M2's typed
context/argument bindings and the M0 compiler probes are still pending.

## M2 — Typed context, placeholders and local variables

- [x] Bind a script to a declared/imported context shape using ordinary `.tua`
  types and values wherever practical. Carry the shape across imports/re-exports.
- [x] Expose a read-only view of host-owned facts such as player name, inventory,
  quest state and reputation. Reading context must not invoke arbitrary methods
  or mutate the supplied game state.
- [x] Add explicit per-player local variables with declared primitive/serializable
  types, defaults and scoped access. Choose a small assignment/update model; keep
  progression/inventory changes in host actions rather than hidden script writes.
- [x] Define when context is sampled/refreshed and when local updates become
  visible. Specify reset, restart, loop and dispose behavior separately.
- [x] Type placeholder arguments against the bound context/locals, including
  optional values and formatting. Reject wrong paths and incompatible values.
- [x] Define checkpoint eligibility: callbacks, userdata, entity objects and
  external service handles are not serializable dialogue locals.

**Acceptance:** a misspelled context field fails after cross-module import;
completion and hover show the correct fields; writes through read-only context
fail. Two players sharing a document cannot share mutable locals accidentally.
Defaults, nil handling, context replacement and restart behavior have runtime
regressions without introducing a module-wide `any` boundary.

### M2 completion record

Implemented `Save.shape` for headless validated copies, `Dialogue.context` for typed
fact/local definitions, and `@context` / `@bind` for imported scalar placeholders.
The supported API preserves concrete payloads through re-exports and checks readonly
writes. Each player owns its values; explicit replacements refresh presentation
without replaying events. Start resets locals, loops preserve them, cancellation
retains inspection state, and disposal clears retained data and validators.

Context-bound assets emit schema two; existing documents remain compatible.
Shared editor analysis supplies diagnostics, completion, hover and declaration
navigation. See the [context guide](../dialogue.md#typed-context-and-conversation-locals),
[runnable example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue-context),
and [M2 validation record](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/M2.md).
The record preserves a narrower nested-shape generic inference/formatter follow-up;
M2 uses concrete derived signatures and does not claim to repair arbitrary wrappers.
Installed-editor UI and rendered showcase qualification remain outside this slice.

## M3 — Conditions, branches and choice availability

- [x] Define a small typed condition language: literals, field reads, comparisons
  and boolean operations. Set precedence, optional-value rules, short-circuiting
  and depth limits; exclude arbitrary calls and side effects.
- [x] Add conditional branches and distinct visible/enabled choice states.
  Disabled replies can expose a localized reason, such as “Requires 20 coins.”
- [x] Specify choice reevaluation when host facts change. Selection by stable ID
  must revalidate eligibility against the current accepted context; stale UI must
  not authorize a purchase or other action. The host remains authoritative.
- [x] Define an explicit fallback for no available choices, rather than leaving a
  player stuck with an empty selection. Preserve explicit line acknowledgement.
- [x] Add a bounded automatic-transition loop for condition-only paths. Yield or
  enter a documented issue state on budget exhaustion instead of hanging.

**Acceptance:** tests cover short-circuiting, missing/nil fields, changed context
between display and selection, hidden versus disabled options, no-choice fallback,
loops, forward references, deterministic evaluation order and instruction limits.
The editor diagnoses wrong types and invalid targets at authored source locations.

### M3 completion record

Implemented typed, data-only conditions, branch-only labels, visible/enabled choice
metadata, localized disabled reasons and explicit no-choice fallback targets. Shared
context facts power diagnostics and editor path queries, including re-exports.
Choice submission rechecks accepted facts; ordinary refreshes preserve execution
identity and events, while fallback transitions create a new visit. Automatic loops
stop with an inspectable fault and require explicit recovery.

Conditional assets use schema three; legacy/schema-one/schema-two playback remains
supported. Limits and optional-value semantics are documented in the
[condition guide](../dialogue.md#conditions-branches-and-available-choices). The
[example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue-conditions)
and [M3 validation record](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/M3.md)
cover emitted Lua, imported typing, stale UI, failure atomicity and native LÖVE with
JIT off/on. Installed-editor UI and rendered showcase qualification remain M10 work.

The separately requested Animal Crossing-style speech blips are planned under M6
(reveal cues) and M7 (typed profiles and host audio adapter); M3 introduces no audio
dependency or automatic gameplay action.

## M4 — Typed events and registered host actions

**In progress — identity foundation:** timed notifications accept explicit
`[id:...]` with a required logical `@document` ID. Schema 4 validates document-wide
uniqueness, owns metadata, and returns `eventId`, `documentId`, and source coordinates.
Legacy records and schemas remain supported. Identity survives reordering and text
edits; delivery still uses per-player generation/visit tokens. This is not persistent
action execution or deduplication. See the [identity validation record](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/M4-event-identity.md).
**Typed notifications implemented:** `Dialogue.events` reuses `Save.shape` runtime
validation and inferred payload facts. Registry imports and bounded `@emit` JSON
payloads have shared diagnostics, event/top-level argument completion, hover and
contract navigation. Typed readers preserve imported payload types; capture occurs
at player construction. Legacy notifications remain dynamic string events. Context
composition uses the same registry instance.

**Registered synchronous actions implemented:** `Dialogue.actions` captures typed
payload shapes and effect classifications. `@action` has stable identity and explicit
success/failure targets. `runAction()` dispatches one host handler; traversal never
executes it. Missing handlers, errors, invalid results and destination errors fault
with source identity. Cancellation/restart/disposal invalidate stale results.
Payloads and handler maps are captured at construction. Schema 6 composes actions,
context and timed notifications. Async handles remain M7; checkpoint/reconciliation
policy remains M8. There is no automatic retry or exactly-once guarantee. See the
[action validation record](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/M4-actions.md).

Follow-up: general tagged-record sibling-field narrowing is not yet available;
use `events.read.name(notification)` for checked payload access. Nested JSON key
completion can be expanded later; nested payload types already validate. See the
[typed notification validation record](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/M4-typed-events.md).

- [x] Reuse existing declared-event/schema facts where possible to expose a typed
  action/event contract from a `.tua` module. Keep signal buses optional adapters.
- [x] Add validated structured payloads and argument completion; retain existing
  undeclared string-payload events as a compatible, explicitly dynamic path.
- [x] Separate notification events from actions whose completion or outcome affects
  flow. Both remain requests to host code, never executable Lua in script text.
- [x] Give persistent actions stable authored identity. Name plus array position
  is insufficient: names may repeat and declarations may be reordered.
- [x] Preserve `shown`/`revealed` timing, ordering, limits and lifecycle tokens.
  Decide payload capture time so handlers do not observe accidental later mutations.
- [x] Define unknown-action, handler-error and invalid-result behavior. Include
  source identity in issues and distinguish presentation-only actions from
  consequential gameplay actions for resume policy.

**Acceptance:** wrong imported event names/payloads fail statically where declared;
legacy events still work. Emitted-runtime tests cover typed records, repeated
names, mutation isolation, reentrant handlers, handler failure, cancellation and
stable identity after harmless text edits. Completion/hover/definition use shared
core facts rather than an extension-only event index.

## M5 — Character expressions and portrait variants

Implemented: `@expression name` selects typed shared character metadata. Schema 7
snapshots expose expression/portrait/portraitAnimation; omission resets to the
character's default portrait. Constructors capture selected strings; hosts own all
sprite playback and image resources. Imported/re-exported expression diagnostics,
completion, hover and declaration navigation use shared core facts. Static-image
and spriteSheet adapter recipes live in the dialogue guide. Animation IDs are host
metadata; their existence is checked by the host's adapter, not by dialogue analysis.
See the [M5 validation record](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/M5-expressions.md).


- [x] Extend shared character records with named expression metadata. Preserve the
  existing optional single `portrait` field as the simple/default case.
- [x] Allow an utterance to select a declared expression. Validate names across
  imported/re-exported character modules and support expression completion.
- [x] Carry the selected expression and resolved asset identifier to the renderer;
  define missing/default behavior and per-line reset versus explicit persistence.
- [x] Provide an optional static-image and `spriteSheet` adapter recipe. Keep
  animation playback state per presentation instance and GPU resources host-owned.

**Acceptance:** two speakers and two simultaneous players can show different
expressions without shared animation state; omitted/unknown expressions behave as
specified; old portrait-only characters still work. Asset navigation remains
static and does not load images during analysis.

## M6 — Text presentation and reveal metadata

**Complete on the feature branch:** optional typed `Dialogue.reveal` consumes resolved
snapshots, segments Unicode 15.1 extended graphemes, exposes inclusive UTF-8 cluster
ranges, and emits at most one eligible speech cue per update. Finish/clear/refresh
invalidate or suppress stale delivery; reduced motion reveals silently. The host owns
rendering, shaping and audio. The official 1,187-case grapheme corpus is exercised on
emitted-runtime tests. Schema-8 typed documents now support rich presentation runs,
pauses/rates, pronunciation hints and selected-template origin mappings. `.tdialogue`
rich authoring, schema-8 emission, catalog checks and mark-name editor queries
are implemented. See the [authoring qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/M6-authoring.md).
See the [reveal validation record](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/M6-reveal.md).


**Styled-text contract and runtime:** the
[contract](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/M6-text-contract.md)
defines explicit rich-text opt-in, renderer-neutral runs, source mappings, bounded
marks and reveal timing. A separate unexpanded template resolver preserves literal
placeholder insertion; existing string resolvers remain compatible. Typed runtime
usage and explicit `@text rich` authoring are available.

- [x] Define a renderer-neutral representation for styled text, emphasis, pauses,
  reveal speed and pronunciation hints, with a plain-text fallback.
- [x] Establish logical text ranges and source mappings through translation,
  placeholder expansion and escaping. Keep substituted values as text, not markup
  that can inject commands, events or styles.
- [x] Specify nesting, unknown marks, duration bounds and malformed-markup errors.
  Keep presentation hints separate from consequential events/actions.
- [x] Support grapheme-aware reveal in the example adapter. Specify the mapping
  from logical text spans to renderer/layout clusters; do not split combining
  sequences or assume byte offsets equal displayed characters.
- [x] Define finish-reveal, skip and reduced-motion behavior, including whether
  presentation pauses are skipped. Preserve an exact host report for `revealed`.
- [x] Expose optional speech-blip reveal cues for Animal Crossing-style character
  chatter. Derive them from actually revealed grapheme/word boundaries, not bytes
  or render frames. Include stable generation/visit/presentation identity so a host
  can reject stale cues. Whitespace is silent; punctuation may pause or suppress a
  cue. Locale changes, restored presentations and finish-reveal must not replay the
  whole line as a burst. Keep these presentation cues separate from gameplay events.
- [x] Document font coverage, shaping and bidirectional-layout responsibilities.
  Accepting Unicode strings is not proof of multilingual rendering support.

**Implemented on `feature/tdialogue-rich-text` (not merged):** the shared Rust
text-field parser and bounded Lua runtime agree on a shared parity corpus. The
runtime supports typed rich documents and localized template callbacks; the
`.tdialogue` directive emits schema 8 with shared diagnostics and editor queries.

**Implementation checklist:**

- [x] Implement and test bounded raw text-field tokenization and fallback templates.
- [x] Integrate the opt-in directive, versioned document emission and shared editor facts.
- [x] Add typed `I18n.dialogueTemplate` selection with locale/plural metadata.
- [x] Connect template resolution to dialogue players and build owned presentation
  runs and origin mappings for schema-8 typed documents.
- [x] Integrate typed-document rates/pauses with reveal and verify lifecycle semantics.
- [x] Qualify styled/localized examples and the acceptance matrix below.

**Acceptance:** tests cover combining marks, emoji sequences, non-Latin text,
escaped markup, expanded placeholders, localized span order, pause/skip and invalid
nesting. Plain renderers can ignore styles and still display correct text. Styled
text cannot execute host actions through a translated or substituted value.

## M7 — Voice and asynchronous actions

**Action-handle foundation implemented on the feature branch:** handlers may return
typed `Dialogue.actionHandle` values. Single settlement, one-visit ownership,
explicit result consumption and lifecycle invalidation preserve bounded execution.
Snapshots expose status and source identity. Host timeouts are explicit rejection;
authored waits, suspension, skip policy and voice/speech work remain below.
See the [action-handle qualification](https://github.com/Kyonru/tua-lang/blob/main/benchmarks/dialogue/M7-action-handles.md).

- [ ] Attach optional voice/cue identifiers and pronunciation metadata to lines,
  with explicit subtitle and voice locale/fallback policy. Reuse `audioSystem`
  or host audio through an adapter; do not create another audio engine.
- [ ] Support optional typed character speech profiles shared across scripts, with
  a named voice/sample-set identifier, pitch range, variation, gain, minimum cue
  interval and character/word cadence. Allow a per-line override or silence. Keep
  audio asset loading, sample selection/synthesis, mixing and playback in the host
  adapter; the core stores data and emits cues without requiring audio or LÖVE.
- [ ] Provide an optional LÖVE/`audioSystem` speech-blip adapter example with original
  or appropriately licensed samples. Demonstrate per-character voices, bounded
  polyphony/voices, source reuse, rate limiting and deterministic variation using
  a private seed (never consume gameplay RNG). Do not bundle proprietary sounds
  or assume a phoneme mapping transfers unchanged between spoken languages.
- [ ] Define speech-blip ownership for concurrent players, pause, skip, line advance,
  cancellation and disposal. Coalesce/drop excess cues when a long frame reveals
  many units; never queue an unbounded catch-up burst. Offer independent chatter
  volume/mute controls, retain readable subtitles when silent, and suppress chatter
  during recorded voice by default unless explicitly mixed by the host.
- [x] Add host-managed asynchronous action handles with explicit completion,
  failure and cancellation. Define single settlement and handling of synchronous
  completion, immediate failure and late callbacks.
- [ ] Add waiting flow for declared actions or authored durations. Determine which
  waits use dialogue time, how concurrent waits are bounded, and how completion
  resumes without recursive unbounded execution.
- [ ] Specify line-advance and skip policy: finish text, stop/continue voice, cancel
  cancellable actions, and refuse or defer skipping non-cancellable actions.
- [ ] Define suspension separately from cancellation. Menus or host suspension
  must not silently let paused dialogue timers finish; external work behavior is
  explicit and stale completions cannot resume a replaced player.
- [x] Expose pending-action status and source identity for diagnostics without
  coupling gameplay to inspector availability.

**Acceptance:** fake host handles test success, error, timeout policy, double
completion, cancel-before-complete, completion-after-reset and interleaved players.
Speech-blip tests also cover spaces/punctuation, combining marks and emoji, slow
and accelerated reveal, long-frame catch-up, finish-reveal, locale refresh, mute,
private-RNG reproducibility and concurrent character voices without duplicate cues.
A native voice example verifies ownership and cleanup, missing audio fallback,
muting, skipping and pause/resume. No required threads, coroutines, wall clock or
LÖVE dependency enters the core player.

## M8 — Save/resume and bounded conversation history

- [ ] Define a versioned, validated JSON-shaped checkpoint: document identity and
  compatible content version, stable position/choice IDs, locals, locale, logical
  timers, relevant event-delivery/action status, and deterministic execution state.
- [ ] Specify what is deliberately not saved: closures, GPU/audio objects, opaque
  host handles, absolute wall-clock timestamps and the whole gameplay context.
  Rebind host context, handlers and presentation after restoring.
- [ ] Define compatible text edits versus incompatible graph/schema changes.
  Reject incompatible checkpoints or require explicit migrations; never silently
  jump to a similarly numbered node. Reuse `Save` validation/migration concepts.
- [ ] Choose safe checkpoint boundaries for pending actions. Initially reject or
  defer checkpoints with non-resumable work; resumable actions need an explicit
  host policy and durable identity, not replay-on-load by default.
- [ ] Provide durable idempotency keys or an acknowledgement protocol for
  consequential actions. Explain the crash window between an external effect and
  recording delivery: exactly-once rewards require host-side atomic persistence
  or idempotency, not merely an in-memory “fired” flag.
- [ ] Add an optional bounded transcript with text IDs, displayed text/locale,
  speaker identity and selected replies. Define capacity, eviction, redaction,
  save inclusion and whether history preserves original displayed translations.
- [ ] Provide an optional `Save.contract` recipe; leave storage and write timing
  to the host. Keep dialogue restart, checkpoint restore and transcript browsing
  distinct operations.

**Acceptance:** checkpoint round-trips across line/choice/wait boundaries, schema
changes, changed documents, corrupt data and locale changes are deterministic.
Test crashes around an idempotent host action and show that restore does not
silently duplicate rewards. History remains bounded; records do not retain large
context objects. Core export/import works without a filesystem or LÖVE.

## M9 — Authoring, diagnostics and preview

- [ ] Extend completion and navigation to labels, choice targets, context fields,
  placeholders, actions, expressions and voice references using their shared facts.
- [ ] Add project-wide rename/reference edits for supported symbols across `.tua`,
  `.tdialogue` and catalogs. Reject ambiguous/dynamic references and conflicts;
  preserve stable text/action identity unless that identity is deliberately renamed.
- [ ] Add a dedicated dialogue formatter that preserves prose whitespace,
  escapes, IDs, metadata attachment and meaning. Do not run the Lua formatter on
  scripts. Verify idempotence and parse/semantic round-trip properties.
- [ ] Diagnose dangling targets, duplicate identities, malformed conditions and
  provably unreachable paths. Distinguish proven impossibility from dynamic
  context; allow declared externally started entry points so they are not falsely
  marked unreachable.
- [ ] Add a static conversation graph/text preview with source navigation and
  explicit mock-context input. Analysis must not execute project handlers or
  asset loaders. Keep any opt-in runtime trace clearly separate from static views.
- [ ] Show actionable runtime issue data: current node/action, wait status,
  cancelled/stale completion, budget exhaustion and missing translation fallback.
  Keep tracing bounded and optional; omit sensitive payloads by default.
- [ ] Preserve fresh results under incomplete edits, unsaved imports, catalog
  changes, branch edits, watch rebuilds and deleted/recreated modules.

**Acceptance:** CLI/LSP/editor agree on the same fixtures. Tests cover cross-file
rename conflicts, unsaved buffers, formatter semantic stability and diagnostic
ranges. Installed-editor checks exercise actual completion/hover/rename/preview;
synthetic protocol timing is not presented as perceived UI latency.

## M10 — LÖVE showcase, documentation and consuming-game qualification

- [ ] Organize dialogue-related files in a dedicated cleanup change. Group internal
  runtime helpers under a dialogue directory (for example, `vendor/runtime/dialogue/`)
  and consolidate the dialogue examples under `examples/dialogue/`, with a shared
  README/index and focused, independently runnable scenarios plus the LÖVE showcase.
  Preserve public `Dialogue` APIs, explicit runtime imports and project runtime
  replacements; retain a compatibility entry module where needed. Update runtime
  bundling/dependency discovery, emitted paths, example configurations, test fixtures,
  build scripts and documentation links together. Verify opt-in helper inclusion,
  packaged output and module loading under Lua 5.1/LuaJIT and LÖVE, including hosts
  whose module search path omits `?/init.lua`. Keep file moves separate from gameplay
  or dialogue behavior changes; shared examples must not require unrelated features.
- [ ] Use [SYSL-Text](https://github.com/sysl-dev/SYSL-Text) as presentation inspiration:
  demonstrate per-presenter defaults, named style mappings for font/color/shadow and
  optional wave/shake effects, plus layout metrics, wrapping/scrolling and character
  sound profiles. Keep resources and visual effects in the LÖVE adapter, honor reduced
  motion, and route gameplay through declared host actions rather than executable
  inline function tags. Evaluate inline icons/images separately with explicit layout,
  localization and accessible plain-text fallback rules before adding core syntax.
- [ ] Add one runnable LÖVE showcase with interchangeable presentation adapters:
  conversation panel, NPC speech bubble, and non-pausing radio message.
- [ ] Demonstrate shared characters/expressions, localized placeholders, conditional
  replies, typed actions, both timing anchors, styled reveal, voice/waits, save/resume
  and bounded history. Use the same player contracts rather than three engines.
- [ ] Include explicit controls for instant reveal, text-speed adjustment, reduced
  motion, subtitles, voice mute and keyboard-only choice selection. Expose focus
  and scrolling clearly; do not claim screen-reader or full shaping support
  without a corresponding implementation and verification.
- [ ] Demonstrate separate optional queues with existing `BoundedQueue`, including
  overflow, channel priority and interruption policy. Keep queue policy in the host.
- [ ] Maintain a headless counterpart and explain small adoption paths, rather
  than requiring every game to adopt every showcase feature.
- [ ] Update `docs/dialogue.md`, the example README and relevant existing feature
  pages. Update `docs/llms.txt` and regenerate the embedded contract when APIs change;
  add changelog entries and accurate feature-example/navigation links.
- [ ] After upstream APIs pass, integrate the relevant adapters in the zombie game
  in separate changes. Read its instructions, preserve unrelated work, use the
  rebuilt compiler/runtime, and keep game-specific assets and gameplay out of Tua.
- [ ] Run native replay coverage at small and large window sizes, multiple players,
  sustained gameplay announcements, menus, repeated resets and mode changes.
  Verify input ownership, pause policy, asset cleanup and no duplicate rewards.
- [ ] Record compiler/revision identity, interpreter/LÖVE versions, JIT status,
  fixtures/seeds and screenshots/video. Compare update cost, allocation and retained
  memory on matching workloads; do not label simulation timing as rendered FPS.

**Acceptance:** all three presentations use the same documented engine API and
remain customizable. Headless and native checks agree on outcomes. The consuming
game retains conversation priority, bounded messages, reset/disposal and gameplay
behavior. Documentation examples are checked/built/executed, not just illustrative
unverified snippets. Record unavailable checks explicitly.

## Shared verification and delivery gate

For each milestone:

1. Add focused parser, semantic, imported-type and runtime regressions before
   widening the implementation. Inspect emitted Lua and source provenance.
2. Verify helper-present/helper-absent output, legacy syntax, project runtime
   replacement, nested/reentrant handlers and multiple independent players.
3. Exercise applicable CLI check/build/watch/cache invalidation and LSP unsaved
   overlays. New diagnostics must use the shared analysis path.
4. Format Tua, authored Lua and Rust separately. Run example checks/builds and
   native LÖVE tests when behavior depends on rendering/audio/input.
5. Follow `skills/tua-verification/SKILL.md`: focused checks during iteration,
   owning suites and `make verify` for cross-cutting work; docs build and relevant
   editor checks before completion. Report failures, reruns and skipped checks
   honestly rather than treating a partial gate as a clean full pass.
6. Record CPU/allocation/editor measurements when claiming an improvement, with
   warm-up, repetitions, JIT/GC policy and instrumentation status. Avoid absolute
   timing assertions and do not optimize away validation or lifecycle semantics.
7. Update this checklist and the owning public docs, then create coherent unsigned
   commits (`--no-gpg-sign`). Keep unrelated compiler fixes, upstream feature work
   and game integration separate. No push, publication or release bump is implied.

No milestone is complete with only a parser, a runtime-only API, or editor-only
behavior. Its typed contract, emitted execution, applicable authoring support,
compatibility checks, documentation and runnable example must agree.

## First implementation slice

M0–M6 are complete; continue with **M7 — Voice and asynchronous actions**.
Reuse declared schemas, typed context/path facts, stable execution identities and
bounded transition behavior. Keep gameplay mutations in explicit host actions;
never execute arbitrary script expressions or replace the host renderer/audio stack.

## Historical delivery checkpoints

<details markdown="1">
<summary>Initial workstream and its verification records (historical)</summary>

These records describe their original checkpoints. In particular, statements
about pending features, feature branches or unpublished work below are historical;
the baseline and milestone checklist above govern the remaining work.

The initial `feature/tdialogue` branches were merged with user approval: Tua
`4eae2033`, game `5f59118`. Follow-up development is on main for the release after
0.11.0. No release version changes, push or publication are authorized by this plan.
Initial base: `1a1a42cb`.

The engine is renderer-independent. `.tdialogue` is the proposed Tua-owned
format, inspired by Love-Dialogue, not a compatibility claim. Normal conversations
wait for explicit advance by default. Temporary messages are an explicit policy.
Game rendering, commands, input and pause ownership belong to the host.

## Progress

- [x] Isolated upstream and game worktrees/branches.
- [x] Initial headless runtime: validated owned documents, line/choice/end nodes,
  independent sessions, explicit start/advance/choose, cancellation/disposal and
  owned presentation snapshots. No required LÖVE globals.
- [x] Typed imported contract and opt-in emitted runtime integration; shared editor
  queries and emitted-Lua regressions.
- [x] Renderer seam in the game, persistent conversations, separate temporary
  policy, independent UI clock and lifecycle.
- [x] Shared `.tdialogue` parser/document validation and source locations.
- [x] Build/watch integration, dependency invalidation and mapped diagnostics.
- [x] Game demo authored as a compiled script with native renderer verification.
- [ ] Localization IDs/catalog integration and rich-text placeholder handling.
- [x] Editor grammar, diagnostics and shared-character completion/navigation.
- [ ] Translation coverage, cross-language rename and script formatting.
- [ ] Conditions, local variables, registered commands, waits/cancellation,
  deterministic instruction limits and independent-session integration tests.
- [ ] Native rendered examples and full cross-repository verification.

The feature-branch API accepts structured data through
`local Dialogue = require("tuadialogue")` and `Dialogue.new(document)`. This ordinary
import carries the shared compiler contract and opts into runtime emission; no
global builtin is required. The API is still under development for the next release. Each document has an entry ID and a node map. Lines
point to the next node; choices reference targets by stable option IDs. End nodes
complete playback. No reveal or expiry policy is hidden inside the core.

The first implementation deliberately returns owned snapshot copies. Do not claim
zero-allocation playback or call snapshot per glyph/frame without measurement.
Later compiler-owned immutable documents may be shared safely between sessions;
current sessions defensively own their input. Unknown document schema fields and
schema versioning must be finalized with the typed public contract.

Validation for the first slice: the bundled plain-Lua runtime suite passes on Lua
5.4.8; dialogue lifecycle tests pass in native LÖVE 11.5 / LuaJIT 2.1.1700008891
with JIT off and on. Explicit Lua formatting uses the shared release formatter
and checks one-pass stability. No emitted compiler integration or rendered game
behavior is claimed yet. Full `make verify` is required when those surfaces change.

## Typed integration

The shared contract checks line/choice/end input records and method arguments.
`start()` uses the entry node and `start("id")` selects a named node. Imported and
re-exported snapshots preserve completion/hover/signatures and reject invalid
assignments. Only the hidden colon receiver uses `any`, following existing runtime
contracts; document and presentation payloads remain typed.

Snapshots contain an optional presentation node. When present, `text` and `options`
are always available, defaulting to empty values; speaker and next remain optional.
This stable presentation shape is distinct from the validated input node union.
The constructor defensively copies documents, and snapshots remain owned copies.

The optional `start` argument derives from a function declaration, which preserves
arity metadata that callback-type annotations currently lack. General discriminated
record-union narrowing and nested optional-record syntax need separate compiler
follow-up probes; this API does not claim to fix those broader limitations.

Run `examples/dialogue` with the feature compiler for an executable headless host.
CLI regressions cover omission when unused, project replacements, valid emission,
wrong argument/field types and invalid node kinds. The emitted runtime runs the
lifecycle oracle; `.tdialogue` parsing and game renderer integration remain pending.

Typed integration validation: `make verify` passed (2,099 core tests, one existing
ignored test; 504 LSP tests; CLI suites, Clippy, language evaluations and extension
checks). The headless example builds and runs from its `build` directory. The
emitted runtime passes the lifecycle oracle in LÖVE 11.5 / LuaJIT 2.1.1700008891
with JIT off/on; standalone runtime tests pass on Lua 5.4.8. Explicit formatting
checks cover the runtime, declaration contract, test and example.

The first LSP feature smoke exceeded its 1,000 ms timing limit (1,042 ms) while
full verification was running. Repeating it after that workload finished passed.
This is not a latency benchmark or proof of installed-editor UI behavior. No game
renderer, `.tdialogue` parser, release installation, push or merge is included in
this slice.


## Game renderer integration

The isolated game feature branch now consumes the typed player through a small
session adapter. Normal conversations wait for explicit advance or choice;
temporary messages use a separate hold/fade clock and resume after a modal
conversation. The game retains its renderer, theme controls, shimmer, word entry,
shadows and elastic sizing. Pause-menu ownership remains independent from the
conversation's gameplay block. Reset and disposal release presentation resources.

The game integration passes its 46 emitted-Lua regressions, including persistence,
choice dispatch, independent clocks, temporary channel isolation, zero-hold timing,
resize, graphics state and owned/borrowed resource lifecycle. A 210-frame native
LÖVE 11.5 / LuaJIT 2.1.1700008891 replay (JIT disabled) exercises real input,
640×480 and 1280×720 rendering, pause/resume, blocked firing, mode replacement and
repeated reset. These are functional checks, not performance measurements.

This proves the renderer seam against the production emitted runtime. The fixture
still constructs structured data in Tua; the parser, localization, custom editor
language and richer execution nodes remain pending. The existing headless example
is unchanged, and no game source/assets have been copied into Tua fixtures. Both
feature branches remain unmerged pending the user's explicit approval.


## Shared authoring frontend

The version-one subset now has a shared `tua-core` parser and Lua-data emitter.
It supports version headers, speaker declarations, labels, explicit text IDs,
narration, consecutive lines, choices, forward references/cycles and terminal
lines. The typed source model retains declaration/reference/text spans; emitted
Lua has per-line source provenance. Unknown constructs and incomplete flow fail
with TL5012 rather than producing partial output. Input uses the shared source
size limit and parsing never evaluates project code.

`docs/dialogue.md` owns the subset contract, including whitespace and escaping.
`examples/dialogue/intro.tdialogue` and the Rust `dialogue_compile` development
harness exercise the frontend; `asset_host.lua` feeds its output into the existing
player. They do not claim automatic asset discovery. The example remains outside
`src` until project/build/watch integration is implemented. Translation IDs are
retained, but catalogs, interpolation and cross-document uniqueness are pending.

The next step is project integration: opt-in discovery, typed imported generated
modules, dependency invalidation, mapped diagnostics and last-valid development
output. That must reuse this parser through shared analysis; no second parser
belongs in the CLI or extension. Then connect catalogs and shared editor queries.

Final `make verify` passes: 2,107 core tests (one existing ignored test), 504 LSP
tests, CLI suites, Clippy, language evaluations and extension checks.
Frontend validation: eight parser regressions (including Unicode/property and
input-limit coverage) and four emitted-data regressions pass. The multi-line
fixture is parsed explicitly as Lua 5.1; the actual bundled player executes the
compiled asset on Lua 5.4.8 and native LÖVE 11.5 / LuaJIT 2.1.1700008891 with JIT
off and on. The authored example and the documentation's exact syntax example
compile successfully. The feature tour checks 52 files, builds 93 outputs and
executes its emitted dialogue module. Lua example formatting, Rust formatting,
documentation build, feature coverage and JSON/whitespace checks pass. These are
correctness checks, not performance measurements or installed-editor validation.

## Project integration

Dialogue scripts under the configured source root opt in through `.tdialogue`.
The shared source model, VFS and module graph preserve typed document exports,
including forwarding modules. The CLI checks and emits them through the existing
build/watch/cache/source-map paths. Invalid edits keep the prior valid output;
deletions clean up generated output and maps. Text IDs are unique per project.
Watch conservatively revalidates all dialogue assets after a dialogue change to
handle cross-file ID conflicts. Importing data alone does not bundle the player.

`examples/dialogue/src/intro.tdialogue` now runs through ordinary project builds.
The language server indexes disk assets and preserves their kind in module graphs;
Tua imports support hover/completion. Dedicated script grammar, unsaved dialogue
editing, localization and richer execution remain pending. Contract field locations
anchor to the asset start because they have no authored Lua field declaration.

The game fixture migration is complete; see the verification checkpoint below.
Localization and dedicated dialogue editing are the next milestones.

Project integration validation: the workspace Rust suites pass (2,108 core tests,
one existing ignored test; 505 LSP tests; all CLI suites). The first `make verify`
reached Clippy with a needless borrow; after removing it, the affected watch test
and all remaining verification targets pass, including Clippy, contract/schema
checks, model-language evaluations, extension checks (289 tests), JSON and
whitespace checks. Documentation and the LSP feature smoke pass. The dialogue
example passes formatting, check/build and execution on Lua 5.4.8. Its
project-emitted data/player also complete 1,000 sessions each with JIT off/on in
LÖVE 11.5 / LuaJIT 2.1.1700008891. Native artifacts and compiler/output hashes are
recorded locally in `/tmp/tdialogue-project-native/provenance.json`. These are
correctness checks, not performance measurements. Installed-editor UI validation
remains pending.

## Game authored-script checkpoint

Game feature commit `5f59118` replaces the structured demo with a `.tdialogue`
asset loaded through a typed import. The session adapter and renderer are
unchanged. A short final utterance waits for acknowledgement before completing;
queued temporary text resumes afterward. No private game source was copied into
Tua fixtures.

The game passes check (166 sources), 202 typed specs, build (251 outputs), all
46 Lua regressions, and its required formatting gate. Native LÖVE 11.5.0 /
LuaJIT 2.1.1700008891 with JIT disabled passes a 210-frame dialogue replay and a
150-frame practice replay. Captured rendering at 640×480 and 1280×720 was inspected.
The clean release compiler is `abf6cab7`, SHA-256
`a996e7e160b8d452c5f2502d298f13ef587bd0fd60a1eafdecc68e1c25a98188`.
Game evidence lives in its `docs/audits/2026-11-23/dialogue-script-validation/`.
These are functional checks, not performance benchmarks. Both feature branches
remain unmerged, with no package version changes or publication.

## Automatic helpers and bounded game channels

`Dialogue` and `BoundedQueue` now use the shared builtin-module path: references
inject runtime imports, local bindings shadow them, and explicit imports remain
compatible. Both reuse their existing typed contracts, including generic queue
payloads and cross-module editor queries. No alternate runtime is introduced.

The game composes the existing bounded FIFO into separate conversation and message
channels, with eight and 32 waiting slots respectively, in addition to active
items. Overflow rejects the new item with `false`. Conversation priority, reveal
clocks, queue inspection and reset/disposal belong to the game adapter. The core
player's direct `start()` lifecycle remains unchanged. Localization and dedicated
script editing remain the next authoring milestones.

Validation for automatic helpers: final `make verify` and docs build pass (2,111
core tests, one existing ignored test; 506 LSP tests; 167 CLI integration tests;
289 extension checks). The first full run caught lost default-catalog sharing;
a shared lazy-module predicate and a regression now preserve sharing without
leaking optional helper types into unrelated files. Explicit imports and project
runtime replacements remain covered. The headless and feature-tour examples
check, build and execute without handwritten helper imports.

The game passes 202 typed specs, all 46 Lua regressions and a 210-frame native
LÖVE queue replay, including bounded overflow, FIFO ordering, suspended clocks,
queued farewell progression and reset/disposal. Its source/compiler evidence is
in `docs/audits/2026-11-23/dialogue-queue-validation/`. Installed-extension bundle
updates and releases are not part of this checkpoint.

## Shared character authoring

Ordinary `.tua` modules now export reusable character records. Scripts declare
`@characters Cast "dialogue.characters"` and reference `Cast.Guide`. Shared core
analysis resolves and checks exported members without executing project code;
emission uses ordinary runtime imports rather than baking in display names.
Module dependencies participate in build/watch invalidation. The VS Code
extension registers `.tdialogue` with a dedicated grammar and LSP selector;
completion, hover, definition navigation and unsaved overlays are implemented.
Lua formatting is suppressed for scripts. Cross-language rename, localization,
portrait delivery in snapshots and richer script execution remain follow-ups.

Shared-character validation: 2,115 core tests (one existing ignored), five emitted
asset tests, 507 LSP tests, 167 CLI integration tests and the other CLI suites pass.
Watch regressions cover changed character types, deletion and recreation; editor
regressions cover re-exports, import rebinding, unsaved buffers and completion edit
ranges. Clippy, model-contract/evaluation gates, 291 extension checks and activation
smoke, LSP feature smoke, documentation build, JSON and formatting checks pass.
The first `make verify` stopped at two existing Rot runtime tests with missing or
unexpected generated output; all four Rot tests pass on an isolated serial rerun.
The remaining verification stages were then completed separately. The activation
fixture was updated to expect dialogue selectors and its complete gate rerun.
No installed VS Code UI validation is claimed. The shared-character headless
example passes formatting, check/build and emitted Lua execution.

</details>
