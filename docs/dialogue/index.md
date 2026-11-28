---
icon: lucide/messages-square
---

# Dialogue

Write conversations in `.tdialogue` files and play them with the typed `Dialogue`
builtin. Share character definitions between scenes, attach portraits, and schedule
custom events while keeping your own renderer and input controls.

**Availability:** Tua 0.12.0 and newer. Use matching compiler and extension
versions so dialogue diagnostics, previews and emitted runtime support agree.

| Tua handles | Your game handles |
| --- | --- |
| Script parsing, validation and typed imports | Drawing text, names, portraits and choices |
| Line order, choices and named entry points | Input, text reveal and scrolling |
| Timed event records | Event handlers: sounds, animations or game actions |
| Independent player state and clocks | Whether a conversation pauses gameplay |

**New to dialogue?** Start with [your first playable dialogue](../tutorials/first-dialogue.md):
one question, two replies, two source files. It explains the defaults before
introducing optional capabilities.

## Explore the dialogue guides

| Guide | Use it for |
| --- | --- |
| [Dialect and semantics](dialect.md) | All directives, structural tags, escapes, rich-text marks and named styles |
| [Editor tools and previews](editor.md) | Formatting toolbar, completion, navigation, flow preview and inspection |
| [Player lifecycle and timing](player.md) | Creating players, progressing lines, waiting, suspension and skip policy |
| [Text presentation and reveal](presentation.md) | Typewriter effects, grapheme clusters, rich runs and renderer responsibilities |
| [Characters, portraits and audio](characters-and-audio.md) | Shared casts, expressions, portrait adapters, recorded voice and chatter |
| [Context and conditional flow](conditions.md) | Typed facts, placeholders and conditional branches or replies |
| [Dialogue localization](localization.md) | Catalogs, translated templates and changing locale |
| [Events and host actions](events-and-actions.md) | Timed notifications and synchronous or asynchronous gameplay handlers |
| [Checkpoints, saves and history](checkpoints-and-history.md) | Saving, resuming and retaining a bounded transcript |
| [LÖVE showcase](showcase.md) | Runnable visual reference and reusable host adapters |

## Your first conversation

Prefer a terminal? This walkthrough needs no LÖVE window, font or image assets.
For a window with interactive replies, use the [playable starter](../tutorials/first-dialogue.md).

### 1. Create the project

Use this layout:

```text
tua.toml
src/
  main.tua
  intro.tdialogue
```

`tua.toml`:

```toml
source = "src"
out = "build"
main = "main.tua"
target = "lua51"
```

### 2. Write the conversation

`src/intro.tdialogue`:

```text
@dialogue 1
@character Guide "Guide"

[start]
// @id intro.hello
Guide: Ready?
// @id intro.yes
-> Yes [target:done]
// @id intro.again
-> Again [target:start]

[done]
// @id intro.bye
: Goodbye. (end)
```

| Syntax | Meaning |
| --- | --- |
| `@dialogue 1` | Required format version |
| `@character Guide "Guide"` | A character ID and its display name |
| `[start]` | A named entry point; the first label is the default entry |
| `// @id intro.hello` | Stable text ID, unique across dialogue files in the project |
| `Guide: Ready?` | A character's spoken line |
| `: Goodbye.` | Narration without a speaker |
| `-> Yes [target:done]` | A reply that leads to the `done` label |
| `(end)` | End this path after the final line is acknowledged |

Keep text IDs stable when editing wording. A choice's ID also identifies it when
the host calls `choose()`. Consult the [dialect reference](dialect.md)
for all directives, control tags and rich-text options.

### 3. Play it from Tua

`src/main.tua`:

```tua
local document = require("intro")
local player = Dialogue.new(document)

player:start()
local node = player:snapshot().node
if node then print(node.text) end

-- Acknowledge "Ready?" to enter the choice node.
assert(player:advance())
local choices = player:snapshot().node
if choices then
  for _, option in ipairs(choices.options) do
    print(option.id, option.text)
  end
end

-- Select by stable ID, not the displayed text or array index.
assert(player:choose("intro.yes"))
local farewell = player:snapshot().node
if farewell then print(farewell.text) end

-- Even a line marked (end) waits for acknowledgement.
assert(player:advance())
assert(player:snapshot().state == "completed")
player:dispose()
```

The player does not read the keyboard itself. This host chooses "Yes" directly;
in a game, call the same methods from your input handlers.

### 4. Build and run

From the project directory, with the development compiler on your path:

```sh
tua check
tua build
cd build
lua main.lua
```

The output shows `Ready?`, the two choice IDs and labels, then `Goodbye.`.
The [runnable example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/basics)
also demonstrates loops, shared characters and both event clocks.

## Build and import a script

Place scripts under the configured source directory, for example
`src/scenes/intro.tdialogue`. Their extension opts into compilation without an
additional configuration flag:

```tua
local document = require("scenes.intro")
local player = Dialogue.new(document)
player:start()
```

`tua check` validates scripts and typed imports. `tua build` emits
`scenes/intro.lua` and source maps when enabled. Imports retain the document
contract through Tua re-exports and support Tua hover/completion. Merely importing
data does not bundle the player runtime; referencing `Dialogue` or explicitly
importing `tuadialogue` does.
Avoid source files that produce the same Lua output path. Out-of-place builds
reject dialogue/Lua and dialogue/Tua collisions. In-place builds retain the
existing generated-Lua sibling convention.

`tua watch` validates edits before replacing the last valid generated module.
Deleting a source removes its generated module and maps. A dialogue change
revalidates all project dialogue scripts so duplicate IDs can be detected and
repaired across files. Build caching includes authored scripts. The language server
watches dialogue assets on disk and maintains unsaved dialogue overlays. Shared
character dependencies participate in normal module invalidation, including
watch rechecks after character type changes and deletion. Rebuilding data does not itself
replace a running player's owned document or reload Lua's module cache.

## Compiler frontend

`tua_core::dialogue_asset::parse_dialogue_source(path, source)` returns the typed
speaker/section/line/choice model and source ranges without reading files or
executing project code. `tua_core::emit::emit_dialogue_source(path, source)`
validates that model and returns an ordinary Lua data module with per-line source
provenance. The output is accepted by the same `Dialogue.new(document)` API.
It imports shared character modules when declared; dialogue text is always emitted
as escaped string data. Project-level character type validation uses the shared
analysis snapshot, while the standalone emitter validates script syntax only.

Diagnostics use `TL5012` and one-based Unicode scalar columns with exclusive
range ends. Unknown references, duplicate IDs/declarations, incomplete flow and
unsupported syntax prevent output. Generated internal node IDs use an `@` prefix
that authored identifiers cannot use. The shared parser serves project checking and build/watch emission; it does not
execute dialogue text.

See the [dialogue example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/basics)
for the structured host and frontend development harness. The format is Tua-owned;
compatibility with Love-Dialogue `.ld` files is not implied.
