# Registered dialogue actions

Run `tua check`, `tua build`, then `(cd build && lua main.lua)` from this directory.
The example purchases an item through explicit synchronous host dispatch, then
shows actions composed with context, branches, and timed typed notifications.
It also returns a typed action handle, settles it later, and explicitly consumes
the result without invoking dialogue transitions from a callback.
See [the dialogue guide](../../docs/dialogue.md#run-a-registered-host-action).
The `wait.tdialogue` script also demonstrates a duration wait and suspension.
Generated `build/` output is not committed.


`skip-policy.tua` is optional host code using existing APIs. Bind one policy per
player before dispatch. Handlers use `policy.begin(request, cleanup)` to opt into
user-facing cancellation; omitting cleanup refuses skipping while pending.
`policy.skipConversation()` cancels the whole conversation, never jumps past an
action or manufactures success. Settled results must be consumed first. Direct
player lifecycle operations still invalidate handles; non-cancellable external
work remains the host's responsibility. See `skip-demo.tua` and the
[policy table](../../docs/dialogue.md#choose-an-advance-and-skip-policy).

## Save after an action has settled

`checkpoint-demo.tua` opts into `checkpointEffects = "settled"`, dispatches one
action, saves its outcome line and restores it into a fresh player. The handler
count stays at one. Registry-backed players use checkpoint v3; plain/context-only
players keep v1/v2. The default still rejects registry checkpoints.

Ready actions, pending or unconsumed resolved handles, and synchronous handler
execution cannot be checkpointed. V3's `actionState = "settled"` means no work is
attached or executing in this player, not that external work has stopped or a
purchase is durably acknowledged. Cancel/restart may detach a handle without
stopping its host operation; reconcile that work through the host policy. Restoring never dispatches
an action or emits notifications; future explicit updates may emit unfired events.
Current handlers and context are rebound, while typed locals and event timers are
restored. Bump `checkpointVersion` when payload schema or host semantics change.

For consequential effects, use the [durable recipe](../dialogue-durable/README.md)
to reconcile the crash window between the effect and saving the new position.
Saving a position alone does not prevent replay from an older checkpoint.

Run `python3 verify-checkpoints.py`; add `--love /path/to/love` for v1/v2/v3 runtime
fixtures with JIT enabled/disabled and separate native save/load processes. Tests
use isolated save identities and delete their save files on successful completion.

In `src/scene.tdialogue`, request completion inside `success:done` or
`failure:declined`, or use Go to Definition to jump to the corresponding label.
The queries also work while editing an unfinished target; they never execute
`purchase`.
