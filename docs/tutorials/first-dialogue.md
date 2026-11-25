---
icon: lucide/message-circle
---

# Your first playable dialogue

Start with a conversation you can read and answer. This guide uses two source files
and LÖVE's built-in font. You do not need portraits, a character registry, audio,
translations or a custom UI framework.

Use LÖVE 11.5 and a compiler/extension containing the dialogue feature, available
in development builds for the release after 0.11.0. The version number alone is
not sufficient to identify those builds. Prefer a terminal? The
[headless walkthrough](../dialogue.md#your-first-conversation) uses the same player.

For these systems in one evolving game, continue with [between-wave dialogue and language](arena-game/conversation-and-language.md).

## 1. Run a conversation

From a checkout of the Tua repository:

```sh
cd examples/dialogue/first-conversation
tua check
tua build
love build
```

Press **Space**, choose **1** or **2**, then press **Space** to acknowledge the
farewell. **R** restarts; **Escape** quits. That is the whole example.

The [runnable source](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/first-conversation)
is also a starting point for your own project. To create it manually, use:

```text
tua.toml
src/
  intro.tdialogue
  main.tua
```

`tua.toml`:

```toml
source = "src"
out = "build"
main = "main.tua"
target = "love11-luajit"
```

## 2. Change the words

`src/intro.tdialogue`:

```text
@dialogue 1
@character Guide "Mara"

[start]
// @id intro.hello
Guide: The ferry is ready. Will you come with us?
// @id intro.yes
-> I'm ready. [target:aboard]
// @id intro.no
-> I'll take the next one. [target:later]

[aboard]
// @id intro.welcome
Guide: Welcome aboard. (end)

[later]
// @id intro.farewell
Guide: Then I'll keep a light on for you. (end)
```

`Guide` is the character's script identifier; `Mara` is the displayed name. Each
`[label]` names a destination. A `->` line is a reply, and its `target` selects
where the story continues. `(end)` finishes after the last line is acknowledged.
Keep `// @id` values stable when editing words: they identify text and replies.

Change the greeting, rebuild and restart. Drawing and input need no changes.

## 3. Connect drawing and input

`src/main.tua`:

```tua
-- The script owns the conversation; this file owns drawing and input.
local document = require("intro")
local player = Dialogue.new(document)
local font = love.graphics.newFont(20)
player:start()

function love.draw()
  local width, height = love.graphics.getDimensions()
  love.graphics.clear(0.08, 0.12, 0.14)
  love.graphics.setFont(font)
  love.graphics.setColor(0.94, 0.92, 0.85)
  local snapshot = player:snapshot()
  local node = snapshot.node
  if snapshot.state == "completed" then
    love.graphics.print("Until the next crossing.", 32, 48)
  elseif node then
    love.graphics.print(node.speaker or "Your reply", 32, 48)
    love.graphics.printf(node.text, 32, 92, width - 64)
    for index, option in ipairs(node.options) do
      love.graphics.printf(tostring(index) .. "  " .. option.text, 32, 100 + index * 56, width - 64)
    end
  end
  love.graphics.setColor(0.65, 0.76, 0.75)
  love.graphics.printf("SPACE  Continue     1 / 2  Reply     R  Restart     ESC  Quit",
    32, height - 64, width - 64)
end

function love.keypressed(key: string)
  if key == "escape" then love.event.quit()
  elseif key == "r" then player:start()
  else
    local snapshot = player:snapshot()
    local node = snapshot.node
    if snapshot.state == "awaiting_advance" and key == "space" then
      player:advance()
    elseif snapshot.state == "awaiting_choice" and node then
      local index = key == "1" and 1 or (key == "2" and 2 or 0)
      local option = node.options[index]
      -- Input selects a row; the player receives its stable authored ID.
      if option then player:choose(option.id) end
    end
  end
end

function love.quit()
  player:dispose()
  font:release()
end
```

Only three player operations drive this conversation:

| Operation | When the host uses it |
| --- | --- |
| `snapshot()` | Read the current state, text and replies for drawing |
| `advance()` | Acknowledge a spoken line |
| `choose(option.id)` | Select an authored reply |

`start()` begins at the first label and can restart the player. `dispose()` releases
it when the host shuts down. Guard the optional `node` and check `state` before
handling input: pressing Space during a choice should not silently choose for the player.

The numbered keys are this example's input policy. Add a third script reply and
extend the numeric-key mapping to support it. The host already draws all options.
For long paragraphs or many choices, add scrolling instead of relying on this
starter's fixed spacing.

## Defaults you can rely on

`Dialogue.new(document)` needs no options for a plain conversation.

| With this starter | Add only when you need it |
| --- | --- |
| The first label is the starting point | `start("label")` for another entry |
| The snapshot contains the whole line; this host draws it immediately | [Reveal text progressively](../dialogue.md#reveal-text-by-grapheme-cluster) |
| Lines wait for `advance()`; replies wait for `choose()` | [Auto-advance or skip policy](../dialogue.md#choose-an-advance-and-skip-policy) |
| Authored source text is available without catalogs | [Translations](../dialogue.md#localize-lines-choices-and-character-names) |
| No history is retained by default | [Bounded history](../dialogue.md#keep-a-bounded-conversation-history) |
| No audio or gameplay pause is imposed | Your audio adapter and host pause policy |
| No save file is created | [Checkpoints](../dialogue.md#checkpoint-a-plain-conversation) and persistence |

Referencing `Dialogue` automatically includes its runtime. You do not register a
plugin or require an internal runtime module in typed source. The ordinary
`require("intro")` imports the compiled script.

There is no update callback here because this script has no timed behavior. Add
`player:update(dt, fullyRevealed)` when using timed events/waits, and dispatch the
returned event records. [Timing](../dialogue.md#advance-both-clocks-explicitly) explains
when to report reveal completion; calling `update` does not render or play sounds.

## Grow one feature at a time

You can stop after any step. These are independent capabilities, not prerequisites.

| Next need | Small next step | Runnable example |
| --- | --- | --- |
| Reuse a cast | [Share characters](../dialogue.md#share-characters-between-scripts), then add portrait metadata | [basics](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/basics) |
| React to gameplay | [Typed context](../dialogue.md#typed-context-and-conversation-locals), then conditional replies | [context](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/context), [conditions](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/conditions) |
| Improve presentation | Reveal first; add rich styles or audio independently | [reveal](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/reveal), [rich](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/rich), [audio](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/audio) |
| Reach another language | Keep IDs stable, add catalogs and select a locale | [localization](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/localization) |
| Change game state | Distinguish notification events from acknowledged actions | [events](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/events), [actions](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/actions) |
| Resume later | Save a plain checkpoint before adding consequential rewards | [checkpoint](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/checkpoint), then [durable](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/durable) |

For visual inspiration, open [Beacon Harbor](../dialogue.md#try-the-love-showcase).
It combines these features and offers interchangeable renderers. Borrow one adapter
when you need it; you do not need to copy its controller to use dialogue.

## If something does not work

- **Unknown `Dialogue` or `.tdialogue` import:** confirm that both the compiler and
  editor extension contain the feature. Run `tua check` from the example directory.
- **The final line stays visible:** `(end)` still waits for `advance()`; press Space.
- **A reply does not work:** acknowledge the greeting first, then press 1 or 2.
- **Edited text is unchanged:** rebuild, close the running game and launch `love build` again.

Keep the [dialogue reference](../dialogue.md) nearby for syntax, timing, lifecycle and
editor tools once the first conversation works.
