# Tua dialogue implementation workstream

Branch: `feature/tdialogue`. Target: the release after 0.11.0. Do not merge,
publish, change release versioning or modify the release worktree without the
user's explicit merge approval. Base: `1a1a42cb`.

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
- [ ] Editor grammar, diagnostics, completion, navigation and translation coverage.
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
