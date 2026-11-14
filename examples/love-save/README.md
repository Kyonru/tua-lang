# Save contract example

This small LÖVE game persists a score with a typed `Save` contract and a
revision-polled store binding. Its v1 migration converts `points` to `score`.

Run `tua build` here, then run the `build` directory with LÖVE. Space increases
the score, S checkpoints immediately, R accepts recovery from an older valid
slot, and D discards both damaged slot files after an error. The window shows
the current issue message. `love.quit` flushes and reports failure to stdout.

The stable `t.identity` in `conf.tua` puts saves in LÖVE's game save directory.
Do not use the example identity for a real game.
