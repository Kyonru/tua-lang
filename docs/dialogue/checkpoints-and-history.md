---
icon: lucide/save
---

# Checkpoints, saves and history

Resume a conversation at a supported boundary, integrate durable host actions with typed saves, and retain an optional bounded transcript.

[Dialogue overview](index.md)

## Checkpoint a plain conversation

For plain dialogues, a checkpoint can preserve the current line, choice or wait
without depending on disk or LÖVE. Give the script a stable `@document` ID and
choose a host compatibility key:

```tua
local scene = require("scene")
local options = { checkpointVersion = "harbor-v1", locale = "en" }
local player = Dialogue.new(scene, options)
player:start()
local saved = player:checkpoint()

-- Store `saved` with the host's JSON persistence when appropriate.
local restored = Dialogue.new(scene, options)
restored:restore(saved)
```

The [checkpoint example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/checkpoint)
round-trips line, choice and suspended-wait positions. `checkpoint()` returns an
owned plain record. `restore()` accepts only a fresh idle player; invalid data,
incompatible content or a throwing resolver leaves that player idle. No automatic
branch traversal, action dispatch or event delivery occurs during restore.

Checkpoint version 1 contains the document ID, host content version, exact graph
signature, node/state, elapsed and revealed-event timers, fired-event indices,
locale and suspension. It excludes closures, host arguments, gameplay context,
GPU/audio objects, handles, wall-clock time and partial typewriter progress. Rebind
arguments and resolvers through the fresh player's options. Saved locale wins;
call `setLocale()` afterward for a different language.

Restored lines appear fully revealed and emit no catch-up speech cues. Existing
revealed-event clocks are preserved; if reveal had not finished before saving,
its clock starts at zero on restore. Shown-event clocks retain their saved elapsed
time. Fired flags prevent ordinary duplicate delivery within the restored visit;
newly due notifications still require `update()`. Suspended restores stay suspended.
The snapshot's `restored` flag remains true for this visit (including locale refresh)
and resets on a new node. The audio example suppresses automatic recordings for
such snapshots; a host may offer explicit replay. Runtime identity counters belong
to the fresh player and are not durable idempotency keys.

### Compatibility and limits

The host `checkpointVersion` key is required, nonempty and at most 256 bytes, as is
the document ID. Checkpoint node IDs are limited to 1,024 UTF-8 bytes and locales to
256. Timers must be finite, nonnegative and at most 10¹² seconds. Wait elapsed time
cannot exceed its authored duration. The validator rejects unknown fields,
metatables, malformed event arrays, unsupported versions and incompatible positions.

A collision-free canonical graph signature includes schema, entry, node kinds,
node/text/choice IDs, targets, wait durations and event identities/payloads/timing.
Changing those facts rejects restore rather than jumping to a similarly numbered
node. Plain prose, translated presentation, portraits and voice metadata may change
without invalidating the graph. Bump the host key when a semantic change requires
migration despite an unchanged structure. Migration is an explicit host decision;
there is no automatic remapping in this version.

Graph work is capped at 8,192 node/event/option records, canonical traversal at
16,384 entries and depth 32, string content at 256 KiB, and the final signature at
512 KiB. Delivered-event arrays contain at most 4,096 entries. The graph signature
is data, not a cryptographic integrity check. Apply your storage schema/size limits
before decoding untrusted save files.

Version 1 rejects context documents, branches and conditional choices. Typed
context players use version 2, described below. **Both versions reject all action-
and typed-event-registry documents**, not merely currently pending work. Idle,
completed, cancelled, disposed, faulted and action states cannot be exported.
Version 3 offers explicitly opted-in settled registry boundaries, described below.
The host persistence and durable action recipes are separate. Checkpoints do not
promise exactly-once external effects: crashing between
an effect and its durable acknowledgement still requires host-side atomic
persistence or idempotency.

### Restore typed dialogue locals

A context player without registries exports version-2
checkpoints. The `locals` field retains the definition's concrete type for checking,
completion and hover. It contains an owned copy validated by the original
`Save.shape`; whole gameplay facts remain outside the checkpoint.

```tua
local context = require("context")
local scene = require("context_scene")
local options = { checkpointVersion = "harbor-context-v1" }
local original = context.new(scene, { coins = 10 }, options)
original:start()
original:setLocals({ visits = 3 })
local saved = original:checkpoint()

-- Supply current gameplay facts; restore only the dialogue's local variables.
local resumed = context.new(scene, { coins = 20 }, options)
resumed:restore(saved)
assert(resumed:getLocals().visits == 3)
assert(resumed:getContext().coins == 20)
```

The [typed checkpoint example](https://github.com/Kyonru/tua-lang/blob/main/examples/dialogue/checkpoint/src/context_demo.tua)
includes the shared context and authored script. Shared context definitions retain
checkpoint-local inference through imports and forwarding modules. Constructed
players can also be exported directly, returned by a factory, or exposed as named
fields and re-exported through Tua or Lua facade modules. Their concrete facts,
locals and method signatures remain available to checking and editor queries.

Version 2 supports branches, conditional choices and bindings in the document;
only line/choice/wait positions are checkpoint boundaries. Restore rebuilds the
current presentation with saved locals and freshly supplied facts. It does not
traverse branches, run actions, dispatch notifications, or append history. Future
transitions evaluate conditions against the rebound facts and restored locals.

At a saved choice, at least one reply must remain visible and enabled under the
rebound state. Otherwise restore fails and leaves the fresh player idle—even if
the choice has a fallback or was suspended. The host may update the fresh player's
facts and retry, or explicitly start a new conversation. Restore never silently
changes the saved position. Invalid locals, missing binding values and throwing
resolvers also leave facts, locals and playback state unchanged.

Version-2 graph identity additionally covers compiled branch/choice conditions,
branch targets, binding definitions/fallbacks and local defaults. Plain version-1
graphs are unchanged, and the two versions are not interchangeable. Locals must
pass the current Save shape; optional absent fields, false, finite signed numbers,
arrays and nested records retain their normal Save semantics. Before validation,
local data is bounded to 16,384 entries, depth 32 and 256 KiB of strings, separately
from the graph budget. Cycles, metatables, functions, sparse arrays, invalid UTF-8
and incompatible values are rejected.

The graph includes defaults, **not a full schema fingerprint**. Bump the required
host `checkpointVersion` whenever fact/local schema semantics or host behavior
changes incompatibly, even if defaults stay the same. There is no automatic schema
migration. Facts, closures, host handlers and media are rebound, never serialized.

## Checkpoint conversations with actions or typed events

Opt in with `checkpointEffects = "settled"` when constructing a registry-backed
player. This selects **version 3** for that player; omitting the option (or choosing
`"reject"`) preserves the default registry exclusion. Plain/context-only documents
continue producing v1/v2 data even when the option is set.

```tua
local actions = require("actions")
local scene = require("scene")
local function fresh()
  return actions.new(scene, {
    checkpointVersion = "merchant-v1",
    checkpointEffects = "settled",
    handlers = {
      purchase = function(payload: { item: string }): boolean
        return payload.item == "shell" -- Example decision; host owns real effects.
      end,
    },
  })
end
local player = fresh()
player:start()
player:runAction() -- Explicit host dispatch; reaches the authored outcome line.
local saved = player:checkpoint()
local resumed = fresh()
resumed:restore(saved) -- Presents that line; does not dispatch the purchase again.
player:dispose()
resumed:dispose()
```

The runnable [settled checkpoint example](https://github.com/Kyonru/tua-lang/blob/main/examples/dialogue/actions/src/checkpoint-demo.tua)
counts handler calls and verifies that restore leaves the count unchanged.
The same option works on event-registry and context factories, including contexts
that combine actions and events.

Only line, choice and wait boundaries can be exported. Ready actions, pending
handles, unconsumed handle resolutions and synchronous handler execution are
rejected. V3 requires `actionState = "settled"`, meaning **no action is attached or
executing in this player**. It is not a receipt or proof that external work has
stopped or was durably committed. No
handle, handler, result callback, gameplay fact or transaction ID is serialized.

V3 fingerprints action IDs, names, validated payloads, effect classifications,
success/failure targets and registry presence in addition to the existing graph.
It retains typed-event fired flags and logical timers. Restore emits no notifications;
unfired due events require a later `update()`. New visits may emit their events again.
Typed locals are included only for context players. Rebind current facts, handlers,
resolvers and presentation services when creating the fresh player.

Changing an action or event payload, identity or timing rejects the old checkpoint.
Payload schema and host-handler semantics still require an explicit
`checkpointVersion` change when their meaning changes: graph identity does not
fingerprint arbitrary validators or closures. No automatic v1/v2/v3 migration occurs.

A checkpoint cannot make an effect and its acknowledgement atomic. If the host
commits a purchase then crashes before saving the new dialogue position, restoring
an older position can reach that purchase again. Use the
[durable-action recipe](#recover-consequential-host-actions) or an external
idempotency authority for those effects. The same limitation applies to side effects
performed in response to emitted notifications. Cancel/restart may detach a handle
without stopping its external work; reconcile abandoned consequential operations
through the host policy as well.

When persisting v3 with `Save.contract`, include `actionState` in the host schema
(for example `Save.enum({ "settled" }, "settled")`), and include the concrete local
schema only for a context player. Keep the host envelope version independent from
the checkpoint version. The v2 Save example is deliberately scoped to its own scene.

## Persist checkpoints with Save

The [checkpoint example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/checkpoint)
includes a typed `Save.contract` recipe for a context-backed conversation:

```tua
local sessions = require("saved_session") -- Example host module, not a built-in.
local session = sessions.new(20) -- Rebind the current gameplay facts.
if not session.load() then session.player:start() end

-- Save only when the host chooses an eligible line, choice or wait boundary.
session.save()
session.player:dispose()
```

`load()` restores into the fresh idle player and returns false only when no
checkpoint exists. A corrupt, incompatible or currently unavailable checkpoint
raises an error; present that error instead of silently starting again. Save checks
the envelope schema, then dialogue restore checks graph identity, timers and the
current context. The saved locale and locals are retained; current gameplay facts
are supplied by the host. No action dispatch or transcript replay occurs.

The example's envelope version and concrete local schema belong to the host.
Adapt the `visits` schema alongside your context, and use explicit migrations for
incompatible stored data. Core checkpoint versions and the host compatibility key
have separate roles. The recipe reopens Save before writes to observe uncertain
prior writes; it assumes one writer and does not promise power-loss durability.

Restarting dialogue, restoring a checkpoint and browsing history are separate
operations. Restart does not erase the stored checkpoint, and history is not saved
by this recipe. Storage timing remains a host decision.

## Recover consequential host actions

The [durable dialogue recipe](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/durable)
shows how to recover one local purchase using existing `Dialogue.actions` and
`Save.contract` APIs. It persists an operation ID before dispatch, then saves the
coins/inventory change and its boolean receipt together. Recovery loads that record
and creates a fresh player; the host explicitly calls `runAction()` to consume the
receipt or commit prepared work. A receipt returns the original result, including
`false`, without applying the purchase again.

```tua
local journal = require("journal").new()
local flow = require("session").new()

-- The profile must already have been explicitly initialized by the host.
local recovered = flow.recover()
if recovered then
  assert(recovered.player:runAction())
  local state = journal.load()
  print(state.coins, state.shells)
  -- Acknowledge after the host has consumed the action's outcome.
  recovered.acknowledge()
end
```

These `journal` and `session` modules belong to the example, not new built-ins.
The host calls `flow.begin()` for a deliberate new purchase. Each occurrence gets a
new persistent key; authored action IDs and live generation/visit counters alone
are not durable operation identities. Acknowledgement removes the one outstanding
receipt but retains the sequence counter, preventing stale keys from being reused.
Cancelling playback does not cancel the durable intent; prepared work can be
explicitly abandoned with `journal.abandon(key)`.

A write error does not establish that nothing was saved. The recipe reopens a fresh
Save contract on each operation, reconciles any committed receipt, and stops on
all load issues rather than accepting older data or resetting automatically.
Compatibility and payload checks run before dispatch. The README and runnable tests
cover failures before/after writes, failed read-back, damaged slots, restart before
acknowledgement, stale keys and declined results.

The guarantee is scoped to **single-writer local state and its receipt in one Save
record**. Save does not guarantee power-loss durability, concurrent writers or cloud
conflict resolution. External services must honor the same idempotency key and
provide an authoritative outcome query; a local receipt cannot make an external
reward atomic. Core checkpoints reject action/typed-event registries by default; v3 requires explicit
settled-boundary opt-in and still rejects ready/pending actions. This example's explicit host intent is a separate
recovery protocol, not general serialization of an action-enabled conversation.

## Keep a bounded conversation history

Enable history when constructing a player. It works with plain conversations and
with typed-context, event and action players; it does not save their gameplay state.

```tua
local player = Dialogue.new(scene, {
  locale = "en",
  history = { capacity = 100, maxBytes = 65536 },
})
player:start()

local history = player:history()
for _, entry in ipairs(history.entries) do
  print(entry.speaker or "", entry.text)
end
-- Remove one retained entry, for example before sharing a transcript.
if #history.entries > 0 then
  player:redactHistory(history.entries[1].id)
end
player:clearHistory()
```

History is disabled by default. `capacity` is required when enabled and accepts
1–1,024 entries. `maxBytes` defaults to 65,536 and accepts 1–1,048,576; it counts
**all retained string bytes**, including IDs and locale, not Lua table overhead.
Oldest entries are evicted until both limits fit. A single entry larger than the
byte budget is skipped intact; text and identifiers are never truncated.
`history.dropped` counts evicted and oversized entries. Neither case interrupts
playback. The capacity bound also limits per-entry table overhead.

A line is recorded when it is presented, with its full resolved plain text—even
while a renderer is still revealing characters. A choice is recorded only after
its transition is accepted and its destination presentation resolves successfully.
It precedes the destination line in history. Invalid, disabled and suspended
choices, throwing resolvers and transition-budget faults add no selected reply.
Waits, actions, branches and unselected choices do not produce records.

Each entry has a session-local numeric `id`, `kind` (`"line"` or `"choice"`),
`nodeId` and `text`. Optional fields are `documentId`, `textId`, `choiceId`,
`speaker`, `speakerKey` and `locale`. `speakerKey`, when authored, supplies the
stable localized speaker identity; `speaker` is the displayed name. Selected
replies use their option ID as `textId`. No context, locals, action/event payloads,
rich-text trees, portrait or audio objects are retained. The
[feature example](https://github.com/Kyonru/tua-lang/blob/main/examples/features/src/dialogue.tua)
shows capture, browsing and redaction.

Records preserve the original text and locale. Locale/context refreshes do not
rewrite or duplicate earlier lines. Browsing returns owned records and never
advances the conversation or replays effects. IDs are not reused after clearing;
`redactHistory(id)` removes an entry and returns whether it was still retained.
Redaction does not affect copies previously returned to the host. `clearHistory()`
releases all entries and resets byte/drop counts. Both operations are forbidden
inside text resolver callbacks, like other player mutations.

History remains available after cancellation, completion and `start()` restarts;
call `clearHistory()` at your chosen conversation boundary. `dispose()` releases
it. Disabled or disposed players return an empty history with zero limits/counts.

Checkpoints deliberately **exclude history**. Restore neither imports earlier
records nor appends the restored line again. New lines and replies after restore
are recorded normally. A host may separately persist a returned history snapshot
under its own Save schema, redaction and retention policy; there is no history
import API. Transcript browsing and resuming executable dialogue are separate
operations. Entry IDs are not durable action or reward identifiers.
