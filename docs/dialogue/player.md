---
icon: lucide/clock
---

# Player lifecycle and timing

Create and control players, including temporary messages, wait nodes, suspension and input policy. The host decides when to advance and whether dialogue pauses gameplay.

[Dialogue overview](index.md)

## Player API at a glance

| Call | Purpose |
| --- | --- |
| `Dialogue.new(document, options?)` | Validate and copy a document; optionally select a localization resolver |
| `player:setLocale("fr")` | Refresh localized presentation without starting a new visit |
| `player:start()` | Start at the document's first label |
| `player:start("done")` | Start/restart at a named entry point |
| `player:snapshot()` | Get a defensive copy of current presentation state |
| `player:update(dt, fullyRevealed)` | Advance event clocks and return due events |
| `player:advance()` | Acknowledge a line; returns whether it advanced |
| `player:choose("intro.yes")` | Select a choice by its text ID; returns whether it matched |
| `player:cancel()` | End the current session; the player can be started again |
| `player:dispose()` | Release player state; this player cannot be restarted |

A snapshot contains `state`, `generation`, `visit`, optional `nodeId` and optional
`node`. Always guard `node` before accessing it. A present node has `text` and
`options`, defaulting to an empty string and array. `speaker`, `portrait`, `next`,
`hold`, `fade` and `events` are optional. `generation` distinguishes sessions;
`visit` increases on each node entry, including loops.
Snapshots also expose optional `documentId` and `locale`, plus `presentationRevision`
and `revealComplete`. Lines preserve `textId`; choices preserve their existing `id`.

Referencing the `Dialogue` builtin automatically includes `tuadialogue` only when
used. Local bindings named `Dialogue` take precedence. Ordinary Lua can use
`require("tuadialogue")`; a project module with that name can replace the bundled
runtime.

### Create a document without a script file

Procedurally generated dialogue can use the same player:

```tua
local player = Dialogue.new({
  entry = "hello",
  nodes = {
    hello = {
      kind = "line",
      text = "Welcome aboard.",
      speaker = "Guide",
      next = "finish",
      events = {
        { anchor = "shown", delay = 0, name = "greeting", payload = "hello" },
      },
    },
    finish = { kind = "end" },
  },
})

player:start()
for _, event in ipairs(player:update(0, false)) do
  print(event.name, event.payload)
end
player:advance()
player:dispose()
```

Structured line nodes also accept `portrait`, `hold` and `fade`. Event payloads
must be strings in structured documents, including `""` when empty. The runtime
validates and copies these fields. Negative/nonfinite update times and nonboolean
reveal flags are errors.

## Conversations and temporary messages

The same player can support a story conversation, a radio message during combat,
a tutorial prompt or a short notification. **Pause policy belongs to your host.**
Creating a `Dialogue` player never pauses the game by itself.

| Presentation | Typical host policy |
| --- | --- |
| Story conversation | Pause gameplay; wait for the player's acknowledgement |
| Radio conversation | Keep gameplay running; route dialogue input separately |
| Notification | Keep gameplay running; reveal, hold, fade, then advance |

A line can suggest a notification lifetime:

```text
@timing hold 3 fade 0.35
```

A host following this hint uses **reveal → hold for 3 seconds → fade for 0.35
seconds**. Read `node.hold` and `node.fade` from the snapshot. Both are optional;
your host chooses defaults when absent. Ordinary conversations may ignore them
and wait for input instead.

`@timing` does not advance a line or implement fading. There may be one directive
per line; both values must be finite, nonnegative seconds. Zero is valid. If a
host dismisses the line before a late event becomes due, that event is discarded.

## Wait and suspend dialogue

Use an authored wait when flow should pause before another label:

```text
@dialogue 1
[delay]
@wait 0.5 [target:greeting]
[greeting]
// @id greeting
: Welcome back. (end)
```

A wait occupies its own label, with one explicit target and a finite duration from
0 to 86,400 seconds. It cannot share the label with utterances, branches or actions.
Scripts containing waits emit schema 9, or schema 10 when they also declare audio metadata. Earlier scripts keep their previous schema
selection; older replacement runtimes must add schema-9 support to run wait scripts.
Typed documents can use `{ kind = "wait", duration = 0.5, next = "greeting" }`.

After entering a wait, `snapshot().state` is `"waiting"` and `snapshot().node` is
absent. `snapshot().wait` contains `duration`, `elapsed`, `remaining`, and optional
source coordinates. The snapshot's node ID and visit identify that wait.

Call `player:update(dt, revealed)` with **dialogue-time seconds** to progress it.
The reveal flag is irrelevant during a wait. Each update completes at most one wait
and discards excess time; it does not apply that time to the next wait, line events,
or host action. A zero-duration wait still needs an update, so a zero-duration cycle
cannot run unboundedly. Existing automatic branches retain their transition budget.
Errors thrown while entering the destination fault once with `wait_transition_error`;
bounded branch faults retain their existing diagnostic. Updates do not retry. `advance()` and `choose()` cannot skip a wait.

### Pause without cancelling

```tua
player:suspend()
-- Menu frames may continue calling update; dialogue clocks stay frozen.
player:update(10, false)
player:resume()
```

Both methods return whether they changed suspension. Suspension applies to active
lines, choices, actions and waits. It preserves state, visit, generation, presentation,
elapsed time and queued action handles; `snapshot().suspended` reports the flag.

While suspended, `update()` returns no events and advances no clocks.
`advance()`, `choose()`, and `runAction()` return false. Resuming does not consume an
action or account for elapsed wall time. If context refresh hid every choice while suspended, resume reconciles that deferred fallback
using the existing transition budget. If its presentation resolver throws, resume
throws and leaves the player suspended at the choice so the host can retry explicitly.
If the game stopped calling update entirely, its first resumed update should use
the current frame's dialogue time, not the time spent in a menu.

External host operations continue unless the host pauses them itself. They may
settle their handles during suspension; the next explicit `runAction()` after
resume consumes the result. If a synchronous handler suspends its player before
returning a boolean, that result is queued too, without invoking the handler again.
Cancellation/disposal still invalidate handles. Restart creates a fresh unsuspended
run; cancel and dispose also clear suspension.

When using `Dialogue.reveal`, pass the suspended snapshot to `present()`.
Its subsequent updates freeze fractional reveal progress, pauses and speech timing;
suspension changes invalidate old speech-cue sequence IDs. Pass a fresh snapshot on
resume. Older snapshots without `suspended` are treated as unsuspended.
`finish()` can still reveal text silently, but cannot navigate the suspended player.
The host owns pausing already-playing audio and animation.

Explicit locale/context refreshes remain available while suspended and retain their
existing silent reveal-completion policy. An empty-choice fallback is deferred until
resume; refresh itself cannot navigate the suspended choice. Suspension is not a content lock, an audio
pause command, a save checkpoint, or cancellation of external work.

## Choose an advance and skip policy

Keep finishing text, advancing a line, and cancelling an entire conversation as
separate commands. The core does not assume that unfinished external work can be
stopped, rolled back, or treated as successful.

The copyable typed host policy in
[`examples/dialogue/actions`](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/actions)
uses existing player and handle APIs. It introduces no new runtime method or
`.tdialogue` syntax. Its behavior is:

| Current state | Finish / advance | Skip conversation |
| --- | --- | --- |
| Unfinished line | Finish reveal silently; keep the recording playing. | Cancel the player and stop host audio. |
| Finished line | Advance once and stop the old recording. | Cancel the player and stop host audio. |
| Choice | Require an explicit choice; do not choose automatically. | Cancel without choosing. |
| Authored wait | `advance()` refuses; dialogue time completes the wait. | Cancel the whole conversation, without entering the next node. |
| Action ready to dispatch | Dispatch only through `runAction()`. | Refuse: no cancellation capability has been registered. |
| Pending cancellable action | Do not advance past the action. | Cancel the player; its handle invokes host cleanup once. |
| Pending non-cancellable or untracked action | Wait for explicit settlement. | Refuse; retain the current visit. |
| Settled action | Consume its result through `runAction()` first. | Refuse to discard the result. |
| Suspended player | Resume before navigation. | Refuse this user-facing skip command. |
| Completed/cancelled/disposed | No navigation. | Return inactive. |

Create one policy per player. Before starting the player, bind it once with
`policy.bind(player)`. In the asynchronous handler, return
`policy.begin(request, cleanup)` for work that can genuinely stop, or
`policy.begin(request)` for non-cancellable work. The supplied `request` is the
handler's typed second argument. `policy.skipConversation()` returns an acceptance
boolean and a reason such as `non_cancellable_action`, `consume_action_result`,
`unowned_action`, or `suspended`. The runnable `skip-demo.tua` demonstrates both
kinds of work; emitted lifecycle regressions cover restart and stale capabilities.

Providing cleanup is an explicit **host capability**, not inferred from an action's
`visual`/`consequential` effect classification. Only opt in when stopping is valid.
Stopping does not undo purchases, saves, network writes or other effects already
committed. The policy never resolves a handle artificially, jumps to its success
or failure node, or dispatches an action to discover whether it is cancellable.

Accepted cancellation detaches and invalidates the pending handle before cleanup.
Late completion is ignored. Cleanup errors remain on the handle's
`cancellationError`; they do not turn cancellation into success. A reentrant host
restart wins: the command accepted cancellation of the old visit, but does not
then cancel the replacement session. Check the latest snapshot after callbacks.

This policy is optional host code, not an enforcement layer around `player:cancel()`.
Direct lifecycle cancellation, restart and disposal still invalidate a handle even
when the external operation cannot stop. Keep ownership of that operation in the
host and suppress stale callbacks; invalidating dialogue does not roll back work.
After accepted navigation/cancellation, synchronize the audio adapter immediately
with `audio.update(player:snapshot(), 0)`. Dispose audio and player together.
