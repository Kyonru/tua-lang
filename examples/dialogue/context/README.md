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

In `src/scene.tdialogue`, request completion inside a `%{name}` placeholder,
hover to inspect its imported context type, or use Go to Definition to find its
`@bind` declaration. The same queries work in choice and disabled-reason text.
Suggestions recover unfinished placeholders and refresh after unsaved binding or
context edits; percent escapes and rich-mark attributes stay literal.
