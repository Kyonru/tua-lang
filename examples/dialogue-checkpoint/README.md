# Plain-dialogue checkpoints

Run `tua check`, `tua build`, then `(cd build && lua main.lua)`.
The example round-trips a line, choice and suspended wait without disk or LÖVE.

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

This example does not choose storage or autosave timing. The full `Save.contract`
recipe, typed-context resume and durable action reconciliation remain later M8
work. Optional player history is available separately and is excluded from
checkpoints. Persisting fired flags does not make external effects
exactly-once: a crash can happen between delivery and durable acknowledgement.
