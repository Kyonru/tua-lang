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
- [ ] Shared `.tdialogue` parser/document validation and source locations.
- [ ] Build/watch integration, dependency invalidation and mapped diagnostics.
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
