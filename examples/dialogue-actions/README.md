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
