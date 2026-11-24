# Typed dialogue context

Run with the rebuilt compiler:

```sh
tua check
tua build
(cd build && lua main.lua)
```

The example shares an imported context between two players, binds a nested fact,
an optional nickname and a local counter, refreshes facts without repeating a timed
event, loops without resetting locals, then restarts with fresh locals.

`context.tua` uses `Save.shape` only for headless validation/copies: no save slot or
LÖVE is required. `story.tua` demonstrates type-preserving re-export. The renderer
still consumes ordinary snapshots; the host owns input, clocks and rendering.
