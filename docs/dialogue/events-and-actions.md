---
icon: lucide/zap
---

# Events and host actions

Use timed events for notifications and registered actions when dialogue must wait for an explicit host outcome. Neither executes automatically through authored prose.

[Dialogue overview](index.md)

## Schedule custom events

Events attach to the preceding utterance. Put them before any choices or the next
utterance; they do not need separate text IDs.

The [`arrival.tdialogue` example](characters-and-audio.md#share-characters-between-scripts) contains:

```text
@event shown 0.2 portrait.pulse
@event revealed 0.25 sound.play "welcome"
```

| Part | What it controls |
| --- | --- |
| `shown` or `revealed` | Which clock the delay uses |
| `0.2` or `0.25` | Delay in seconds; finite and nonnegative |
| `portrait.pulse` or `sound.play` | A name your host handles |
| `"welcome"` | Optional string payload; defaults to an empty string |

Neither event name has built-in behavior. The player returns data; your host
chooses what to do with it. Dialogue files never execute arbitrary Lua.

### Give repeated events stable identities

Use an explicit event ID when the host needs to distinguish two calls with the
same name. Declare a logical document ID before the first label:

```text
@dialogue 1
@document arrival
[start]
// @id arrival.greeting
: Welcome aboard! (end)
@event shown 0 portrait.pulse [id:arrival.first-pulse] "soft"
@event revealed 0.2 portrait.pulse [id:arrival.second-pulse] "bright"
```

The optional `[id:...]` goes after the event name and before its string payload.
IDs use the same identifier syntax as labels and must be unique across the entire
document. Text IDs and event IDs are separate namespaces. Rewording a line,
reordering events or renaming the script file does not change an explicit ID.
Changing the authored ID means changing the identity; keep document IDs unique
within your project when combining records from multiple scripts.

For identified events, `player:update` additionally returns:

| Field | Meaning |
| --- | --- |
| `documentId` | Logical ID from `@document` |
| `eventId` | Explicit event ID |
| `source` | Owned `{line, column}` pointing to the event directive, both one-based |

Source coordinates are for diagnostics and can change after edits. Use
`documentId` + `eventId` for authored identity, and the existing `generation` +
`visit` tokens for a delivery within one player. Tokens are scoped to each player;
keep the player identity as well when combining concurrent sessions.

The host still decides how to handle an event. IDs do not persist state, provide
exactly-once delivery across saves, or suppress events on a new visit. Cancelling
or advancing discards pending events; locale refresh does not replay fired ones.
Mutating a document, snapshot or returned source record cannot change a player's
captured identity. Events without IDs keep their existing returned record shape.

Assets with event IDs emit schema **4**; use the paired runtime or check
`Dialogue.schemaVersion >= 4` when supplying a replacement. Earlier document
schemas remain supported. Programmatic schema-4 documents use `id` on event
records and require `documentId`; `source` is optional and, when supplied, must
contain only positive integer `line` and `column` fields. Duplicate IDs are rejected
before playback. See the runnable
[event identity example](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/event-identity).

### Declare typed notification payloads

Use a shared event registry when a string payload is too limited. This builds on
`Save.shape`'s headless validation and copying; it does not create a save slot or
require a signal bus.

`events.tua`:

```tua
return Dialogue.events({
  reward = Save.shape(Save.record({
    coins = Save.number(0),
    item = Save.record({ id = Save.string("") }),
    bonus = Save.optional(Save.boolean(false)),
  })),
})
```

In `victory.tdialogue`, import that registry before labels, then use `@emit` for
declared notifications:

```text
@dialogue 1
@document victory
@events "events"
[start]
// @id victory.greeting
: Well done! (end)
@emit shown 0 reward [id:victory.reward] {"coins":10,"item":{"id":"shell"},"bonus":false}
@event revealed 0.2 sound.play "celebration"
```

Payload strings are literal: they are not localized or placeholder-expanded.
The JSON record is data, never executable Lua. Nested records, arrays, strings,
finite numbers and booleans are supported. Omit optional fields instead of writing
`null`. Payloads allow at most 8 KiB of authored JSON, 16 nesting levels and 256
values. Duplicate keys are errors. The existing limit of 64 notifications per line
counts both `@emit` and legacy `@event` entries.

Event names, required/unknown fields and payload types are checked against the
imported module, including re-exports. Completion suggests event names and top-level
JSON argument keys; hover shows their types and definition lookup opens the contract.
Numeric bounds and other `Save.shape` constraints are enforced at runtime before
playback. A registry supports 1–128 event names. Use direct `Dialogue.events(...)`
and `Save.shape(...)` factory calls to retain specialized inference.

Create the player through the registry. Typed readers return an owned, validated
payload for their event, or `nil` for another notification:

```tua
local events = require("events")
local scene = require("victory")
local player = events.new(scene)
player:start()
for _, event in ipairs(player:update(0, false)) do
  local reward = events.read.reward(event)
  if reward then
    print(reward.coins, reward.item.id)
  end
end
player:dispose()
```

Declared records carry `kind` and `name`, both equal to the declared name, plus the
structured `payload` and existing identity/delivery metadata. Legacy notifications
keep a string `payload` and no `kind`. Use the typed reader to access payload fields;
checking `event.kind` alone does not currently narrow sibling payload fields in Tua.
A matching reader validates again and copies its result, so it also rejects a
payload that the host has since corrupted. A wrong kind/name returns `nil`.

Payloads are captured when the player is constructed, not when their delay expires.
Changing context, locale, schema defaults, the original document, a snapshot or an
already-returned payload cannot change a pending notification's captured data.
Registry validators use the original `Save.shape` descriptors, not mutable public
`copy` or `defaults` fields. Each emitted delivery owns its nested payload.

These are notifications: handling them does not wait, branch, acknowledge a reward
or catch host-handler errors. If a handler restarts/cancels a player, use the
existing generation/visit tokens to reject stale records from the returned batch.
Registered actions can select an outcome target; see [settled checkpoints](checkpoints-and-history.md#checkpoint-conversations-with-actions-or-typed-events) for resume policy.

To combine events with context, pass the **same registry instance** in the context
module's `events` field:

```tua
local events = require("events")
return Dialogue.context({
  events = events,
  facts = Save.shape(Save.record({ name = Save.string("") })),
  locals = Save.shape(Save.record({ visits = Save.number(0) })),
})
```

The script declares both `@context "context"` and `@events "events"`. Construct it
with `context.new(scene, facts)`; the same `events.read` functions handle its results.
Registry identity is checked at construction. Plain `Dialogue.new` rejects a
registry-bound script so the host cannot accidentally use its legacy result type.

An `@events` import emits schema **5** and requires `Dialogue.schemaVersion >= 5`.
Earlier schemas still work. Programmatic events in such documents use
`{kind = "reward", name = "reward", id = "victory.reward", anchor = "shown", delay = 0,
payload = {coins = 10, item = {id = "shell"}}}` and set the document's `events` to the
registry. Unknown events and invalid payloads fail during player construction.
See the complete [typed events example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/events)
for multiple event types, legacy notifications, re-exports and context composition.

### Choose the right timing anchor

| Anchor | Clock starts when | Useful for |
| --- | --- | --- |
| `shown` | The player enters the line | An effect while text is appearing |
| `revealed` | The host first reports reveal completion | An effect after the full line is readable |

Suppose the line appears at **0.0 seconds** and the host reports reveal completion
at **1.5 seconds**:

| Time | Result |
| --- | --- |
| 0.0 s | Line starts |
| 0.2 s | `portrait.pulse` becomes due |
| 1.5 s | The `revealed` clock starts at zero |
| 1.75 s | `sound.play` becomes due, with payload `"welcome"` |

Delivery happens on an update at or after the scheduled time. The first `true`
reveal sample excludes that frame's `dt` from the revealed clock. If input skips
the reveal animation, call `update(0, true)` immediately to mark completion.

### Advance both clocks explicitly

This complete host uses `arrival.tdialogue` from the [shared characters example](characters-and-audio.md#share-characters-between-scripts). It simulates
reveal completion without needing a renderer:

```tua
local document = require("arrival")
local player = Dialogue.new(document)
player:start()

-- At 0.2 seconds, text is still revealing.
local shown = player:update(0.2, false)
assert(#shown == 1 and shown[1].name == "portrait.pulse")

-- Another 1.3 seconds passes; now the host reports full reveal.
assert(#player:update(1.3, true) == 0)

-- A quarter-second after that report, the second event becomes due.
local revealed = player:update(0.25, true)
assert(#revealed == 1 and revealed[1].payload == "welcome")

-- Already-delivered events do not repeat.
assert(#player:update(1, true) == 0)
player:dispose()
```

In a real update loop, update your renderer first, then pass its reveal-complete
flag to `player:update(dt, fullyRevealed)`. To suspend dialogue, skip updates for
both renderer and player. Once reveal completion is reported, that clock keeps
running even if a later sample is false.

### Dispatch events to your handlers

Here is a host that logs the event instead of playing a sound. Replace the handler
body with your own audio or animation integration:

```tua
local document = require("arrival")
local player = Dialogue.new(document)
local handlers: { [string]: (payload: string) -> nil } = {
  ["sound.play"] = function(payload: string)
    print("Requested sound:", payload)
  end,
}

local function updateDialogue(dt: number, fullyRevealed: boolean)
  for _, event in ipairs(player:update(dt, fullyRevealed)) do
    -- A previous handler might have advanced, restarted or cancelled playback.
    local current = player:snapshot()
    if current.generation ~= event.generation or current.visit ~= event.visit then
      return
    end

    local handler = handlers[event.name]
    if handler then handler(event.payload) end
  end
end

player:start()
updateDialogue(0, true)
updateDialogue(0.25, true)
player:dispose()
```

This host ignores names without a handler. Each fired record contains `name`,
`payload`, `generation`, `visit` and `nodeId`.

There can be up to 64 events per line. Events fire once per visit; revisiting a
line through a choice loop schedules them again. Due events are ordered by their
scheduled time, then declaration order for ties. Leaving a line, restarting,
cancelling or disposing discards its pending events. Already-returned batches
need the lifecycle guard shown above if handlers can change playback.

## Run a registered host action

Use an action when host code must return an outcome before dialogue continues.
Timed events remain notifications; `update()` never invokes an action handler.
The runnable [dialogue-actions example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/actions)
includes standalone actions and composition with context and typed events.

Declare `actions.tua`:

```tua
return Dialogue.actions({
  purchase = {
    payload = Save.shape(Save.record({ item = Save.string("") })),
    effect = "consequential",
  },
})
```

Reference it from `scene.tdialogue`. Each action occupies its own label and needs
an explicit ID, document ID, JSON record, and both outcome targets:

```text
@dialogue 1
@document shop
@actions "actions"
[buy]
@action purchase [id:shop.purchase] [success:done] [failure:declined] {"item":"shell"}
[done]
// @id done.text
: Purchased. (end)
[declined]
// @id declined.text
: Not available. (end)
```

Register the host handler when constructing the player:

```tua
local actions = require("actions")
local scene = require("scene")
local coins = 10
local player = actions.new(scene, {
  handlers = {
    purchase = function(payload: { item: string }): boolean
      if payload.item ~= "shell" or coins < 5 then return false end
      coins -= 5
      return true
    end,
  },
})
player:start()
-- The host decides when it is safe to perform the pending operation.
if player:snapshot().state == "awaiting_action" then
  player:runAction()
end
```

`true` selects `success`; `false` selects `failure`. The handler may accept a
second request argument with `actionId`, `name`, `effect`, `documentId`, `nodeId`,
`generation`, `visit`, and optional source coordinates. `snapshot().action` exposes
that metadata while waiting; `snapshot().node` is absent. Payload and request
records are owned copies. Registry validation captures payloads at construction,
and each player captures its handler map.

Only `runAction()` executes a handler, at most one per call. Traversal, snapshots,
locale changes, and updates do not dispatch it. A chain of action labels therefore
requires separate calls. A handler returns a boolean or an action handle (below);
coroutine-based handlers are not supported. Reentrant dispatch returns
`false`. Cancellation, restart, or disposal inside the handler invalidates its
result, including any error returned afterward.

A missing handler, thrown error, or invalid result faults the player with
`action_handler_missing`, `action_handler_error`, or `action_result_invalid`.
An error resolving the destination faults with `action_transition_error`.
`snapshot().issue` carries source/action identity. These failures do not retry:
inspect the issue and explicitly cancel, dispose, or restart after reconciling any
side effects. For synchronous handlers, `runAction()` returns whether dispatch
successfully reached its outcome, not the handler's boolean result.

Choose `effect = "presentation"` for cosmetic host work or `"consequential"` for
changes such as spending currency. This classification is metadata, not rollback
or an exactly-once guarantee. A handler can change the world before it fails;
the host owns reconciliation. Pending actions cannot be checkpointed. See [settled checkpoints](checkpoints-and-history.md#checkpoint-conversations-with-actions-or-typed-events).

For context plus actions, pass the registry as `actions` to `Dialogue.context`,
include matching `@context` and `@actions` imports, and supply `handlers` in the
third argument to `context.new`. An optional `events` registry composes the same
way. The script and constructor must reference the same registry instances.

Action names and JSON payloads validate through imported/re-exported registry
facts. Completion, hover and definition navigation cover names and top-level
payload keys. Dynamic Lua documents validate at construction. Action IDs share a
uniqueness namespace with event IDs and remain stable across text edits.
Action imports emit schema **6**, requiring `Dialogue.schemaVersion >= 6`;
older document schemas remain accepted by the new runtime.


### Wait for asynchronous host work

Return a fresh `Dialogue.actionHandle()` from a handler when the host will produce
the result later. Existing scripts and action registries need no syntax changes.

```tua
local pending = Dialogue.actionHandle()
local player = actions.new(scene, {
  handlers = {
    purchase = function(payload: { item: string })
      -- Start host work here; retain pending in its completion callback.
      print(payload.item)
      return pending
    end,
  },
})
player:start()
player:runAction() -- Captures the handle; still awaiting_action.
pending:resolve(true) -- A host callback supplies the outcome.
player:runAction() -- Consumes it and enters the success target.
```

Use a **new handle for every dispatch** in a real handler. A handle can belong to
only one action visit, even after settlement. Returning the same handle from
another player or visit faults with `action_result_invalid`.

- `handle:resolve(true/false)` selects the authored success/failure target.
- `handle:reject(message)` records a host failure; consuming it faults with
  `action_rejected`. Messages must contain 1–4096 bytes.
- `handle:cancel()` invalidates the handle; consuming it faults with
  `action_cancelled`, rather than selecting the failure target.
- Settlement methods return `true` only for the first settlement. Later calls
  return `false`; malformed arguments still raise validation errors.
- `handle:snapshot()` returns an owned record with `status` and an optional
  `cancellationError`.

The first `runAction()` returns `true` when it accepts a fresh handle.
While that handle is pending it returns `false`, without invoking the handler
again. Once settled, the next call consumes it and returns whether the transition
succeeded. Even a handle settled inside the handler waits for this next call.
`update()`, snapshots and completion callbacks never consume it or start the next
action. This bounds execution without threads, a clock, or recursive callbacks.

While waiting, `snapshot().action.status` is `ready` before dispatch, then
`pending`, `resolved`, `rejected`, or `cancelled`. The same record contains
the stable action/source identity. Status inspection has no side effects.

### Cancel external work safely

Pass an optional cleanup callback when creating the handle:

```tua
local pending = Dialogue.actionHandle({
  cancel = function()
    -- Ask the host operation to stop; release its resources here.
    print("cancel host work")
  end,
})
```

Cancellation invalidates the handle **before** cleanup runs. Restart, player
cancellation and disposal detach and cancel the current pending handle; a handle
returned by a handler after reentrant replacement is cancelled too. Late callbacks
cannot advance the old player. Already-resolved/rejected handles do not run cleanup:
their host operation has finished. Completion clears the stored cleanup callback.

Cleanup runs at most once. Its errors are contained and available in the handle's
`cancellationError` (bounded to 4096 bytes); they cannot prevent player disposal.
A cancellation callback may reenter player lifecycle methods, so the caller should
inspect the resulting snapshot rather than assume its state stayed unchanged.

Cancelling dialogue does not guarantee external work stopped or roll back its
effects. With no cleanup callback, the handle is still invalidated, but the host
must manage any ongoing work. Timeouts belong to the host in this slice: call
`reject("timed out")` when its policy expires. The [host skip policy](player.md#choose-an-advance-and-skip-policy) protects non-cancellable work. Pending work cannot be checkpointed.
