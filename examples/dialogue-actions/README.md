# Registered dialogue actions

Run `tua check`, `tua build`, then `(cd build && lua main.lua)` from this directory.
The example purchases an item through explicit synchronous host dispatch, then
shows actions composed with context, branches, and timed typed notifications.
It also returns a typed action handle, settles it later, and explicitly consumes
the result without invoking dialogue transitions from a callback.
See [the dialogue guide](../../docs/dialogue.md#run-a-registered-host-action).
Generated `build/` output is not committed.
