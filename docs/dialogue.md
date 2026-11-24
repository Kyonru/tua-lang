---
icon: lucide/messages-square
---

# Dialogue

Write conversations in `.tdialogue` files and play them with the typed `Dialogue`
builtin. Share character definitions between scenes, attach portraits, and schedule
custom events while keeping your own renderer and input controls.

**Availability:** development builds for the release after 0.11.0. Use a compiler
and extension containing the dialogue feature; the 0.11.0 version label alone is
not enough to identify one of these development builds.

| Tua handles | Your game handles |
| --- | --- |
| Script parsing, validation and typed imports | Drawing text, names, portraits and choices |
| Line order, choices and named entry points | Input, text reveal and scrolling |
| Timed event records | Event handlers: sounds, animations or game actions |
| Independent player state and clocks | Whether a conversation pauses gameplay |

Start with [your first conversation](#your-first-conversation), then add
[shared characters](#share-characters-between-scripts),
[timed events](#schedule-custom-events), or
[temporary messages](#conversations-and-temporary-messages).

## Your first conversation

This example runs in a terminal. It needs no LÖVE window, font or image assets.

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
the host calls `choose()`.

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
The [runnable example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue)
also demonstrates loops, shared characters and both event clocks.

## Share characters and portraits { #share-characters-between-scripts }

Use an ordinary typed module when several scenes share a cast.

`src/characters.tua`:

```tua
type Character = { name: string, portrait?: string }

local Guide: Character = {
  name = "Harbor guide",
  portrait = "portraits/guide.png",
}

-- A character can have a name without a portrait.
local Captain: Character = { name = "Captain" }

return { Guide = Guide, Captain = Captain }
```

`src/arrival.tdialogue`:

```text
@dialogue 1
@characters Cast "characters"

[start]
// @id arrival.welcome
Cast.Guide: Welcome to the harbor. (end)
@timing hold 3 fade 0.35
@event shown 0.2 portrait.pulse
@event revealed 0.25 sound.play "welcome"
```

`src/departure.tdialogue` can import the same characters:

```text
@dialogue 1
@characters Cast "characters"

[start]
// @id departure.farewell
Cast.Guide: Safe travels.
// @id departure.ready
Cast.Captain: All aboard! (end)
```

`Cast` is a script-local alias. For a nested file such as
`src/dialogue/characters.tua`, use `@characters Cast "dialogue.characters"`.

In the updated extension, typing `Cast.` at a speaker position suggests character
exports. Hover shows their types and comments; go-to-definition opens the typed
module. These features also work with unsaved character edits.

### Read character metadata in your renderer

```tua
local document = require("arrival")
local player = Dialogue.new(document)
player:start()

local node = player:snapshot().node
if node then
  print(node.speaker or "Narrator")
  print(node.text)
  local portrait = node.portrait
  if portrait then print("Portrait asset:", portrait) end
end

player:dispose()
```

A portrait is an **asset identifier string**, not a loaded image. The example can
print `portraits/guide.png` without that file existing; a graphical host must
provide the asset and load it. Tua does not select a portrait layout, image loader
or animation system.

Each used character must have a required string `name`; `portrait` may be a string
or nil. Other fields remain available in the character module but are not copied
into dialogue snapshots. Type aliases and module re-exports work normally.

Character metadata is read when the emitted module loads. Each player then owns
a document copy. Editing a character does not mutate existing players or reload
Lua's module cache; rebuild and recreate/reload the relevant state in your host.

## Schedule custom events

Events attach to the preceding utterance. Put them before any choices or the next
utterance; they do not need separate text IDs.

The `arrival.tdialogue` example above contains:

```text
@event shown 0.2 portrait.pulse
@event revealed 0.25 sound.play "welcome"
```

| Part | What it controls |
| --- | --- |
| `shown` or `revealed` | Which clock the delay uses |
| `0.2` or `0.25` | Delay in seconds; finite and nonnegative |
| `portrait.pulse` or `sound.play` | A name your host handles |
| `"welcome"` | Optional string payload; defaults to an empty string |

Neither event name has built-in behavior. The player returns data; your host
chooses what to do with it. Dialogue files never execute arbitrary Lua.

### Choose the right timing anchor

| Anchor | Clock starts when | Useful for |
| --- | --- | --- |
| `shown` | The player enters the line | An effect while text is appearing |
| `revealed` | The host first reports reveal completion | An effect after the full line is readable |

Suppose the line appears at **0.0 seconds** and the host reports reveal completion
at **1.5 seconds**:

| Time | Result |
| --- | --- |
| 0.0 s | Line starts |
| 0.2 s | `portrait.pulse` becomes due |
| 1.5 s | The `revealed` clock starts at zero |
| 1.75 s | `sound.play` becomes due, with payload `"welcome"` |

Delivery happens on an update at or after the scheduled time. The first `true`
reveal sample excludes that frame's `dt` from the revealed clock. If input skips
the reveal animation, call `update(0, true)` immediately to mark completion.

### Advance both clocks explicitly

This complete host uses `arrival.tdialogue` from the previous section. It simulates
reveal completion without needing a renderer:

```tua
local document = require("arrival")
local player = Dialogue.new(document)
player:start()

-- At 0.2 seconds, text is still revealing.
local shown = player:update(0.2, false)
assert(#shown == 1 and shown[1].name == "portrait.pulse")

-- Another 1.3 seconds passes; now the host reports full reveal.
assert(#player:update(1.3, true) == 0)

-- A quarter-second after that report, the second event becomes due.
local revealed = player:update(0.25, true)
assert(#revealed == 1 and revealed[1].payload == "welcome")

-- Already-delivered events do not repeat.
assert(#player:update(1, true) == 0)
player:dispose()
```

In a real update loop, update your renderer first, then pass its reveal-complete
flag to `player:update(dt, fullyRevealed)`. To suspend dialogue, skip updates for
both renderer and player. Once reveal completion is reported, that clock keeps
running even if a later sample is false.

### Dispatch events to your handlers

Here is a host that logs the event instead of playing a sound. Replace the handler
body with your own audio or animation integration:

```tua
local document = require("arrival")
local player = Dialogue.new(document)
local handlers: { [string]: (payload: string) -> nil } = {
  ["sound.play"] = function(payload: string)
    print("Requested sound:", payload)
  end,
}

local function updateDialogue(dt: number, fullyRevealed: boolean)
  for _, event in ipairs(player:update(dt, fullyRevealed)) do
    -- A previous handler might have advanced, restarted or cancelled playback.
    local current = player:snapshot()
    if current.generation ~= event.generation or current.visit ~= event.visit then
      return
    end

    local handler = handlers[event.name]
    if handler then handler(event.payload) end
  end
end

player:start()
updateDialogue(0, true)
updateDialogue(0.25, true)
player:dispose()
```

This host ignores names without a handler. Each fired record contains `name`,
`payload`, `generation`, `visit` and `nodeId`.

There can be up to 64 events per line. Events fire once per visit; revisiting a
line through a choice loop schedules them again. Due events are ordered by their
scheduled time, then declaration order for ties. Leaving a line, restarting,
cancelling or disposing discards its pending events. Already-returned batches
need the lifecycle guard shown above if handlers can change playback.

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

## Authoring rules and common mistakes

- Declare characters/imports before labels. Inline characters and shared imports
  can coexist, but their aliases must not collide.
- Give every utterance and choice a project-unique text ID. Metadata directives
  have no ID of their own. Blank lines/comments may separate an ID from its text;
  another ID, label or character declaration may not.
- Consecutive utterances advance in order. End each section with choices or a
  line marked `(end)`; sections do not fall through implicitly.
- Put events and timing after their utterance, before choices. A terminal line
  can have metadata, but cannot have choices or another utterance in that section.
- Labels, text IDs and event names begin with an ASCII letter or underscore, then
  use letters, digits, underscores, dots or hyphens. Display text accepts Unicode.
  Import aliases are simple identifiers. `else` and `endif` are reserved labels.

| Symptom | Check |
| --- | --- |
| An event never fires | The host must call `update`; check the reveal flag, handler name and whether the line ended early |
| `advance()` does nothing | You may be at a choice; use `choose()` with a stable option ID |
| The final line stays visible | `(end)` still waits for acknowledgement |
| A portrait does not appear | The host must load and render the snapshot's portrait identifier |
| A character reference reports `TL5012` | Check the imported module, exported member, required string name and optional string portrait |
| Edited text still appears unchanged | Rebuild and refresh the running document/player and Lua module cache |

Translations and scalar placeholder validation are supported (see below). The extension supports
script highlighting and character completion, hover and navigation; script
formatting and cross-language rename are not implemented.

## Whitespace and escaping

Indentation before a statement is ignored. One optional ASCII space following
`:` is syntax; additional leading and trailing text whitespace is preserved.
The choice prefix is `-> ` and its target suffix is ` [target:label]`. One space
before a terminal `(end)` is syntax; whitespace after that marker is ignored.
There are no inline comments: `//` inside utterance text is literal text.

Use `\n`, `\t` and `\r` for explicit text breaks or spacing, and `\\` for a
literal backslash. Escaped quotes (`\"`) are required inside quoted speaker
names. Escape literal braces and brackets with `\{`, `\}`, `\[` and `\]`.
`\$`, `\(` and `\)` are also supported; `\(end)` prevents a literal terminal
`(end)` from ending the line. Unknown or unfinished escapes are errors.

Named `%{name}` placeholders are supported by the opt-in localization adapter below.
Rich-text effects, expressions, conditions, assignments and inline executable
commands are not implemented. Named `@event` metadata is
supported as described above; its behavior comes from host handlers. Named
placeholders are validated; other unescaped braces and brackets remain errors.
Without a resolver, the player presents decoded source text literally.

## Localize lines, choices and character names

The runnable [localized dialogue example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue-localization)
contains English and French catalogs, two simultaneous players and an intentionally
missing French farewell. It checks the same API used by graphical hosts.

### 1. Keep source text and stable IDs

IDs are catalog keys; translate their text, never the IDs themselves. An optional
logical document ID is independent of filenames and generated node addresses:

```text
@dialogue 1
@document harbor.welcome
@characters Cast "cast"
[start]
// @id harbor.hello
Cast.Guide: Hello, %{name}! You have %{count} coins.
@event shown 0 greet "once"
@event revealed 0.2 ready "once"
// @id harbor.again
-> Again [target:start]
// @id harbor.leave
-> Leave [target:done]
[done]
// @id harbor.bye
: Goodbye, %{name}. (end)
```

`@document` appears once before labels. Document IDs and text IDs must be unique
within their respective project namespaces. Document IDs are optional for playback;
this milestone does not implement checkpoints.

Use `%{name}` for named scalar arguments. Names are simple ASCII identifiers, not
field paths or executable expressions. With localization enabled, `%%` produces a
literal percent: `%%{name}` displays `%{name}`. A replacement value such as
`"100% %{other}"` is inserted literally and never parsed again. General printf
formatting and `%<name>.d` are not part of this dialogue adapter.

### 2. Configure existing catalogs

Enable `[i18n]` and configure locale files as described in
[Localization](localization.md). In `src/locales/en.lua`:

```lua
return {
  en = {
    cast = { guide = "Guide" },
    harbor = {
      hello = {
        one = "Hello, %{name}! You have %{count} coin.",
        other = "Hello, %{name}! You have %{count} coins.",
      },
      again = "Again",
      leave = "Leave",
      bye = "Goodbye, %{name}.",
    },
  },
}
```

A line or reply must resolve to a scalar string or plural entry with an `other`
form. The older array-shaped `I18n.t` dialogue entries are a different feature and
cannot be used as graph lines. Each plural form must retain the source placeholders;
`count` must be a finite number. Regional locales use the same parent/fallback chain
as `I18n.t`, followed by authored source prose when no catalog entry exists.

For a shared character, keep a plain fallback name and optionally add `nameKey`:

```tua
return { Guide = { name = "Guide", nameKey = "cast.guide" } }
```

Names without `nameKey` remain unchanged. Character keys use the same runtime
resolver; existing catalog tooling covers their catalog entries. The source-text
extraction manifest covers authored lines/replies, not dynamically computed names.

### 3. Select a locale per player

```tua
local scene = require("scene")
assert(Dialogue.schemaVersion >= 1)
local player = Dialogue.new(scene, {
  locale = "fr-CA",
  resolve = I18n.dialogue,
  args = { name = "Zoé", count = 1 },
})
player:start()
local node = player:snapshot().node
if node then print(node.text) end
```

`I18n.dialogue(key, source, args, locale)` is a scalar adapter over the existing
catalog engine. Referencing `I18n` includes its runtime through the normal opt-in
build path; `Dialogue` alone does not import localization. A headless host may
instead provide a function with that signature returning a string.

The player copies scalar `args` at construction. Strings, booleans and finite
numbers are accepted. Missing required arguments and missing/extra translation
placeholders fail explicitly. Extra **argument values** are allowed so one player
can carry arguments for several lines. Typed context bindings and argument refresh
are M2 work. Do not put services or mutable gameplay objects in this argument map.

Resolvers run on node entry and locale changes, never during `snapshot()`.
Presentation is cached; returned snapshots remain defensive copies. A resolver
must not mutate the same player, including disposal; this is rejected to avoid
half-applied transitions. It may use another player. Resolver errors leave the
previous visit/presentation intact. Resolve text without gameplay side effects.

### 4. Refresh presentation without replaying events

```tua
player:setLocale("en")
local view = player:snapshot()
-- When presentationRevision changes, redraw using the refreshed node.
-- If revealComplete is true, display the whole translated line immediately.
```

A changed locale increments `presentationRevision`, preserves `generation`, `visit`,
node/choice IDs and fired events, and replaces text without a new visit. A partially
revealed line becomes fully revealed: if its revealed clock had not started, it
starts at zero at the switch. Existing clocks continue. `setLocale` returns false
for the current locale. It never changes global `I18n` state or another player.
A renderer must honor `revealComplete`; the core does not draw or count glyphs.
On the next node entry, `revealComplete` resets to false. Restart uses the latest
selected locale. Dispose releases resolver, arguments and presentation references.

### 5. Extract and review translations safely

```sh
tua dialogue-extract > /tmp/dialogue-source.json
```

This read-only command parses project scripts and emits a sorted JSON manifest of
IDs, source text, document IDs and source locations. It neither runs project Lua
nor writes catalogs. Compare successive manifests, merge new keys into the Lua
catalogs and review changed prose with translators; never replace an edited
translation with source prose automatically. Removed IDs remain visible in the old
manifest for intentional cleanup. Invalid scripts or duplicate text IDs fail extraction.

**Tua: Show Localization Explorer** includes authored IDs even before any catalog
contains them, displays source prose and opens its script location. Catalog cells
continue to distinguish exact, fallback, missing and dynamic entries. Hover over a
script text ID for source text; definition navigates to its default-locale/fallback
catalog entry when present. Unsaved scripts and catalogs refresh shared analysis.

Incomplete translations and default-locale source drift are warnings (`TL5010`);
`check --deny-warnings` can enforce completeness. Wrong entry shapes, malformed
placeholders and missing/extra placeholders are errors. Each plural form is checked.
Source drift compares plain default-locale text; plural prose requires translator
review against extraction manifests. Dynamic Lua entries are marked dynamic and
validated when resolved at runtime; analysis never executes them.

### Document/runtime compatibility

Context-free compiled assets expose `schemaVersion = 1`, optional `documentId`, and line
`textId`. The current player accepts both legacy unversioned structured documents
and schema 1; unknown explicit versions and unknown schema-1 semantic fields fail
validation. Legacy inputs retain their existing selected-field copy behavior.

Use the paired compiler/runtime when rebuilding. `Dialogue.schemaVersion >= 1`
is the explicit capability guard for localized hosts, including projects replacing
`tuadialogue`. Old runtimes ignore unknown document fields and cannot detect new
semantics themselves. Replacement runtimes must implement these APIs; the compiler
does not execute or certify arbitrary replacement Lua. Data-only imports still do
not bundle the player. These schema fields do not constitute save/resume support.

## Typed context and conversation locals

Use a shared context definition when scripts need player facts or their own counters.
The host supplies facts explicitly; reading a binding never calls a gameplay method.
[Run the complete example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue-context).

### 1. Declare serializable data in Tua

```tua title="src/story/context.tua"
return Dialogue.context({
  facts = Save.shape(Save.record({
    player = Save.record({ name = Save.string(""), coins = Save.number(0) }),
    nickname = Save.optional(Save.string("")),
  })),
  locals = Save.shape(Save.record({ visits = Save.number(0) })),
})
```

`Save.shape` creates a headless validator and copier; it does **not** create a save
slot, access a filesystem, or need LÖVE. Records, dense arrays, optional fields,
strings, finite numbers, booleans and string enums use the existing
[save schema rules](save-contracts.md). Required fields need defaults. Functions,
userdata, metatables, cycles and unknown fields are rejected. Project engine objects
must be projected into plain data first: store an entity ID and copied facts, not
the live entity object. A future checkpoint can serialize these values; M2 does not
implement checkpoint persistence.

Keep this module definitions-only. It may be shared or re-exported through another
Tua module, but must not import a scene which imports it back. Use the direct
`Dialogue.context(...)` and `Save.shape(...)` built-ins to preserve specialized
inference; wrapping them in an untyped or generic factory can lose payload types.

### 2. Bind named placeholders in the script

```text title="src/welcome.tdialogue"
@dialogue 1
@context "story.context"
@bind name context.player.name
@bind nickname context.nickname default "traveler"
@bind visits locals.visits
[start]
// @id welcome.hello
: Hello %{name} (%{nickname}). Visits: %{visits}. (end)
```

Declare one `@context` and all `@bind` directives before labels. `context` reads
host facts; `locals` reads this player's private variables. Bindings are field paths,
not expressions: no calls, indexing, arithmetic or assignment. A placeholder must
have a binding, and its final field must be a string, number or boolean. Optional
fields, including optional parents, require a compatible scalar `default`. `false`
and zero are values; only a missing/nil value uses the fallback.

The compiler checks imported/re-exported paths and fallback types. Editor completion
and hover show fields; definition navigation leads to the context factory declaration.
Limits are 256 bindings and 32 path segments (including the root). `%{name}` and `%%`
use the same literal-safe formatting as localization. Inserted text is never parsed
again as a placeholder, markup or dialogue code.

### 3. Create players and refresh their data

```tua title="src/main.tua"
local context = require("story.context")
local scene = require("welcome")
local facts = { player = { name = "Ada", coins = 3 } }
local player = context.new(scene, facts)
player:start()

facts.player.coins = 8
player:updateContext(facts)
player:setLocals({ visits = 2 })
local view = player:getContext()
local localView = player:getLocals()
```

`context.new(scene, facts, options?)` validates and copies facts and local defaults.
Every player owns its copies. Mutating the original facts, schema defaults, document
or a getter result cannot change another player's state. `getContext()` and
`getLocals()` return owned, recursively read-only typed views. Lua callers can mutate
their returned copies without altering the player.

`updateContext(value)` and `setLocals(value)` accept complete replacements, not
patches. They validate before committing and refresh current text immediately.
Invalid data or a failing resolver leaves the previous state and presentation intact.
These methods do not change visit/generation, execute actions, or replay timed events.
Renderers honor `presentationRevision` and `revealComplete`, as with locale changes.

| Operation | Facts | Locals | Execution |
| --- | --- | --- | --- |
| `new` | Copy supplied facts | Copy captured defaults | Idle |
| `start`, including restart | Keep current facts | Reset to captured defaults | New generation and visit |
| `advance`, `choose`, loops | Keep | Keep | Normal transition/new visit |
| `updateContext`, `setLocals` | Replace selected data | Replace selected data | Refresh presentation only |
| `cancel` | Keep for inspection | Keep for inspection | Discard pending events |
| `dispose` | Release | Release defaults and validators too | Further context access fails |

Localization options remain `{locale, resolve, args?}`. Use `I18n.dialogue` for the
existing catalog integration; it receives bound values alongside optional extra
arguments. Extra arguments cannot override a binding. Custom resolvers return final
formatted text; the player does not interpolate their output a second time. Bindings
work without localization or a catalog runtime.

### Compatibility

Context-bound scripts emit document schema **2**; context-free scripts continue to
emit schema **1**. The authoring header remains `@dialogue 1`. Use the paired compiler
and runtime, or check `Dialogue.schemaVersion >= 2` with a custom runtime replacement.
Create a bound script through its matching `context.new`, not `Dialogue.new`; a
mismatched definition fails at runtime. Existing structured and schema-one documents,
renderers, localization, character portraits and event clocks remain supported.

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

See the [dialogue example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue)
for the structured host and frontend development harness. The format is Tua-owned;
compatibility with Love-Dialogue `.ld` files is not implied.
