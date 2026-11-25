# Your first conversation

One question, two replies, and a farewell in LÖVE. No downloaded assets, character
registry, audio adapters or player options are required. Use LÖVE 11.5 and a
dialogue-enabled Tua development build; the 0.11.0 version label alone does not
establish support.

```sh
# From the repository root:
cd examples/dialogue/first-conversation
tua check
tua build
love build
```

Press **Space** to acknowledge the greeting, **1** or **2** to reply, then **Space**
to finish. **R** starts again; **Escape** closes the window. Text appears immediately.

Read [`intro.tdialogue`](src/intro.tdialogue) first, then [`main.tua`](src/main.tua).
The script defines words and destinations. The host draws a snapshot and passes
input to `advance()` or `choose()`. `(end)` still waits for acknowledgement.
Referencing `Dialogue` includes its runtime automatically.

Try changing Mara's words while keeping the `// @id` values unchanged. Rebuild and
restart; no host changes are needed. To add a third reply, add its destination and
extend the host's numeric-key mapping; the renderer already lists every option.

There is no `love.update` because this conversation has no timed behavior. It has
no typewriter effect, audio, history, save file or gameplay pause policy. Those are
independent additions, not setup requirements. Layout is intentionally small and
intended for short lines; borrow wrapping/scrolling from the showcase for long content.

Follow the [guided learning path](../../../docs/tutorials/first-dialogue.md) next. Prefer a
terminal? Use [basics](../basics/README.md). The [showcase](../showcase/README.md) is
the full presentation sampler, not the starter template.

## Verification

```sh
tua fmt --check
tua fmt --check tests/replay.lua
tua check
tua build
```

From the repository root, this native replay exercises both replies, completion,
restart and input boundaries (adjust the LÖVE path for your machine):

```sh
python3 scripts/love_replay.py --project examples/dialogue/first-conversation \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/dialogue/first-conversation/tests/replay.lua \
  --frames 120 --fps 30 --seed 71 --output /tmp/tua-first-conversation
```

Generated output stays in `build/` and is not committed.
