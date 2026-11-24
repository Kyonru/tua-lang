# Dialogue checkpoints

Run `tua check`, `tua build`, then `(cd build && lua main.lua)`.
The example round-trips a line, choice and suspended wait without disk or LÖVE,
then restores typed locals with current gameplay facts in `context_demo.tua`.

Opt in with a stable document ID (`@document`) and host-selected
`checkpointVersion`. `player:checkpoint()` returns owned JSON-shaped data;
`freshPlayer:restore(data)` validates it before changing a fresh, idle player.
Restore shows the current line fully revealed, retains timers and delivered-event
flags, and marks the snapshot `restored = true`. The audio example honors that
marker instead of replaying voice recordings. Restore never calls an action.

Version 1 supports plain documents only. Typed context, bindings, event/action
registries, branches and conditional choices are deliberately rejected. Completed,
cancelled, faulted, idle and action states are not checkpoint boundaries. These
limits prevent this first slice from claiming safe persistence of arbitrary effects.

Text/portrait/voice edits are compatible if the logical graph is unchanged.
Changes to node/text/choice identities, targets, waits or events are rejected,
even if the host compatibility key is unchanged. Bump the key for semantic changes
that cannot be detected from graph structure. There is no automatic migration.

Host arguments and resolvers are rebound when constructing the fresh player;
the saved locale is restored. No closure, source, external handle, wall-clock time,
game context or reveal-controller progress is saved. Core generation/visit tokens
are live-session identities, not durable reward or network idempotency keys.

`persistence.tua` and `saved_session.tua` provide an optional `Save.contract`
recipe for the typed-context scene. Storage calls remain explicit; the example
never autosaves or substitutes defaults after a load error. Optional player history
is separate and excluded from checkpoints. For consequential actions, see the
[durable action recipe](../dialogue-durable/README.md); persisted fired flags alone
do not make external effects exactly-once.


`context_demo.tua` uses a shared context definition and authored branches/conditions.
Its v2 checkpoint retains typed locals but not facts. Restoring the choice with
zero coins fails transactionally; updating the fresh player's facts allows retry.
The host compatibility key must change for incompatible schema semantics even if
local defaults stay identical. Import shared context definitions and construct
players in the consumer to retain checkpoint-local inference; additional forwarding
of already-created players has a known editor-analysis inference limitation.

## Persist and resume a conversation

In a LÖVE host, use the native Save backend (or pass an explicit backend for tests):

```tua
local sessions = require("saved_session")
local session = sessions.new(20) -- Current gameplay coins, not saved facts.
if not session.load() then
  session.player:start() -- Explicit new conversation when no checkpoint exists.
end

-- Call at a host-selected line, choice or wait boundary.
session.save()
session.player:dispose()
```

`load()` requires a fresh idle player. It returns false only for no save or an
explicit envelope without a checkpoint. Errors propagate to the host for its error
UI; do not catch them and silently restart. `save()` reopens the contract first,
so uncertain prior writes are observed and damaged slots block further writes.
This is a single-writer example, not a concurrency or power-loss guarantee.

The host envelope has its own version (1), independent of the dialogue checkpoint
version (2). Save validates record shape; restore also checks content key, graph,
logical timers, locals and choices against current facts. Required schema defaults
are for explicit defaults construction, not repairs of missing fields. The concrete
`visits` schema must evolve with the shared context. Change the host compatibility
key for incompatible semantic changes; author explicit Save migrations if needed.
The saved locale wins over the newly constructed player's locale.

The small restore adapter explicitly narrows `state`: Save enums currently infer
`string`, whereas the player requires a literal union. It does not cast to `any`
or bypass runtime validation. For a plain v1 document, omit `locals` from the Save
schema and use a plain player; do not pass this v2 envelope to it.

Restarting (`player:start()`), loading (`session.load()`), and reading history are
separate operations. Restart does not delete an existing save. History is never
serialized or replayed by this recipe. No effects are dispatched by loading.

Run `python3 verify.py` for typed checks and emitted-Lua fault tests. Add
`--love /path/to/love` to exercise separate native processes across line, choice
and suspended-wait boundaries, with JIT on and off. Each native run has an isolated
save identity and removes its test save files after success.
