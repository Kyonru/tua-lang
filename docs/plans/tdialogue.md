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
- [ ] Typed public contracts and opt-in emitted runtime integration.
- [ ] Renderer seam in the game, persistent conversations, separate temporary
  policy, independent UI clock and lifecycle.
- [ ] Shared `.tdialogue` parser/document validation and source locations.
- [ ] Build/watch integration, dependency invalidation and mapped diagnostics.
- [ ] Localization IDs/catalog integration and rich-text placeholder handling.
- [ ] Editor grammar, diagnostics, completion, navigation and translation coverage.
- [ ] Conditions, local variables, registered commands, waits/cancellation,
  deterministic instruction limits and independent-session integration tests.
- [ ] Native rendered examples and full cross-repository verification.

The initial runtime is internal scaffolding, not a released public API. It accepts
plain structured data through `Dialogue.new(document)`; no builtin is registered
and builds do not yet emit it. Each document has an entry ID and a node map. Lines
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
