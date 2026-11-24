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

## Reveal text by grapheme cluster

`Dialogue.reveal(options?)` is an optional, renderer-independent presentation
controller. Give each displayed dialogue player its own controller. It consumes
snapshots after localization and placeholder expansion, reveals complete Unicode
extended grapheme clusters, and optionally returns speech-blip cues. It never
advances the dialogue or plays audio.

The runnable [reveal example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue-reveal)
uses combining accents, emoji sequences, flags, Indic text, and CJK text, plus a
literal placeholder value that resembles a directive.

```tua
local player = Dialogue.new(require("scene"))
local reveal = Dialogue.reveal({
  rate = 30,
  speech = true,
  minCueInterval = 0.05,
})
player:start()

local function update(dt: number)
  reveal:present(player:snapshot())
  local view = reveal:update(dt)
  -- Draw view.text, or map view.visible to clusters in your full-text layout.
  -- Consume view.cues here if your host has a speech-blip audio adapter.
  local events = player:update(dt, view.complete)
  return { view = view, events = events }
end

local function finishReveal()
  local view = reveal:finish()
  -- Report the exact host completion now; do not add a second frame of time.
  local events = player:update(0, true)
  return { view = view, events = events }
end
```

| Option | Default | Meaning |
| --- | --- | --- |
| `rate` | `30` | Grapheme clusters per second; greater than zero, at most 1000 |
| `speech` | `false` | Emit optional presentation cues |
| `minCueInterval` | `0.05` | Minimum seconds between cues; 0–60 |
| `reducedMotion` | `false` | Immediately show the complete line, silently |

Options are captured at creation. `dt` must be finite and nonnegative. Input text
must be valid UTF-8 and at most 65,536 bytes; invalid input leaves the previous
presentation intact. Bounds apply to this optional controller, not existing
plain dialogue playback.

| Method | Behavior |
| --- | --- |
| `present(snapshot)` | Accept a new line identity; return whether the active presentation changed |
| `update(dt)` | Reveal clusters and return a view with at most one new cue |
| `snapshot()` | Return the current view without emitting cues |
| `clusters()` | Return an owned cluster list for the whole accepted line |
| `finish()` | Reveal everything immediately, emit no cues, invalidate earlier cues |
| `clear()` | Release retained text/clusters and invalidate earlier cues |

A view contains `text`, `visible`, `total`, `complete`, `cues`, and identity fields.
`visible` and `total` count grapheme clusters, not bytes, code points, or shaped
glyphs. Clusters contain `index`, `text`, inclusive one-based UTF-8 `startByte` and
`endByte`, and `speakable`. Fetch `clusters()` once per new presentation rather
than copying the full list on every frame. Returned views/lists are owned copies.

Selection is keyed by the snapshot's `generation`, `visit`, and
`presentationRevision`. Re-presenting the same snapshot preserves progress.
Changing text without changing that identity is an error. Locale/context refreshes
already marked `revealComplete` appear fully revealed without a speech burst.
Likewise, after a future/custom restore, honor a completed snapshot or call
`finish()` before consuming updates. This is not a checkpoint API.

Pass snapshots after line changes, cancellation, and disposal. Non-line states
clear the controller. A user skip calls `finish()` and reports `update(0, true)`
to the dialogue player; it does not automatically select a reply or advance a
line. Rich presentations can add authored pauses and scoped reveal rates, described below.

### Use speech cues without catch-up bursts

Each cue identifies the controller `instance`, its `sequence`, the dialogue's
`generation`, `visit`, and `presentationRevision`, plus cluster `index`, `text`,
`startByte`, and `endByte`. Before playing a delayed cue, compare both the controller
instance/sequence and the current dialogue identity, and require an active line.
Finishing, clearing, or accepting another presentation invalidates older sequence
tokens. Use separate controllers for simultaneous players; tokens are runtime-local,
not save-file identities.

Only newly revealed clusters containing a Unicode letter or number are eligible.
Whitespace, punctuation, and emoji without letters/numbers are silent. A long frame
coalesces eligible clusters to the last one, returning at most one cue. Cues skipped
by the interval limit are dropped, never queued. `snapshot()`, `finish()`, locale
refresh, and reduced-motion presentation cannot replay the line as a burst.

Cues describe presentation, not gameplay events or phonemes. The host owns sample
selection, voice identity, volume/mute, source reuse, polyphony and audio scheduling.
No proprietary sounds, audio engine, wall clock, or gameplay RNG are used. The
optional per-character audio-profile/adapter work remains in M7.

### Unicode and layout responsibilities

Segmentation follows the pinned Unicode 15.1 extended-grapheme rules in
[UAX #29 revision 43](https://www.unicode.org/reports/tr29/tr29-43.html), tested against
its official conformance corpus. It is not word segmentation, font shaping, or
bidirectional layout. A grapheme cluster can produce several glyphs, and a shaped
glyph may cover several logical clusters.

For complex scripts, shape the full resolved text with an appropriate font/layout
engine and map logical cluster byte ranges to that layout's clusters. The `text`
prefix is a simple fallback; redrawing a growing prefix can change joining or
ligature forms. The host owns font coverage, fallback fonts, line breaking, shaping,
logical/visual order and bidirectional interaction. Unicode acceptance alone does
not establish correct multilingual rendering.

Plain reveal remains compatible with serialized dialogue schema 7. Custom replacement
runtimes must implement `Dialogue.reveal` to use it. Rich presentations require
schema 8 as described below; translated
and substituted text is not reinterpreted as markup by this controller.

## Rich presentations and timing

Enable rich text with `@text rich` immediately after `@dialogue 1`, before
imports, labels or text IDs. Blank lines and ordinary comments may separate them.
The compiler emits schema 8 with both raw templates and plain fallback templates.

```text
@dialogue 1
@text rich
@document greeting
[start]
// @id greeting.hello
: [em]Hello %{name}[/em][pause=0.25][speed=15]![/speed] (end)
```

Use `[em]…[/em]`, `[style=warning]…[/style]`, and
`[pronounce="hint"]…[/pronounce]` for presentation metadata. Lines also support
`[speed=15]…[/speed]` and `[pause=0.25]`; choices and disabled reasons reject
timing marks. Existing plain scripts keep their syntax and schema behavior.

The editor provides mark-name completion, hover descriptions and highlighting.
Shared diagnostics check nesting, mark parameters and field restrictions.
Static translation catalogs are checked using the source field's rich-text rules;
dynamic templates and final grapheme alignment are validated at runtime.
Translations contain raw markup templates; placeholder values remain literal text.
`tua dialogue-extract` adds `format: "rich"` and `plainTemplate` to rich entries,
while `text` retains the authored template for translation.

The [runnable example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue-rich)
uses an authored script and a typed document. Typed schema-8 documents are also
supported directly:

```tua
local player = Dialogue.new({
  schemaVersion = 8,
  entry = "hello",
  nodes = {
    hello = {
      kind = "line",
      textId = "hello",
      text = "Hello %{name}!",
      richText = "[em]Hello %{name}[/em][pause=0.25][speed=15]![/speed]",
      next = "done",
    },
    done = { kind = "end" },
  },
}, { args = { name = "Ada" } })
player:start()
local reveal = Dialogue.reveal({ rate = 30 })
reveal:present(player:snapshot())
local view = reveal:update(0.1)
player:update(0.1, view.complete)
```

`text` is the matching mark-free **template**, with named placeholders preserved
and literal percent characters encoded as `%%`. `richText` is parsed and owned at
construction, before any session runs. Mismatched fallbacks are rejected. Choice
options accept `richText`; disabled reasons accept `richReason` with matching
`reason`/`reasonId`. Character display names remain plain.

| Mark | Meaning |
| --- | --- |
| `[em]text[/em]` | Semantic emphasis |
| `[style=warning]text[/style]` | Host-defined style key; ASCII identifier, at most 64 bytes |
| `[speed=15]text[/speed]` | Scoped rate in graphemes/second, greater than zero and at most 1,000 |
| `[pause=0.25]` | Pause at this boundary, from zero through 60 seconds |
| `[pronounce="hint"]text[/pronounce]` | Opaque speech hint; host audio behavior is optional |

Marks nest strictly; unknown/mismatched marks and empty paired regions fail.
Pronunciation marks cannot nest. Timing marks are line-only. Numbers use ordinary
unsigned decimals (`15` or `0.25`), without exponents. Escaped brackets are literal:
a Tua string `"\\[em\\]"` displays `[em]`. Placeholder values are inserted after
parsing and are never interpreted as marks, actions or other placeholders.

With configured catalogs, pass `locale = "fr"` and
`resolveTemplate = I18n.dialogueTemplate`. A custom callback has the same signature
and returns `{template, locale?, variant?}`. Rich fields use this callback while
plain fields/names use `resolve`, if supplied. A legacy `resolve` alone receives
the stripped template and returns literal final text with no rich presentation.
Without a resolver, rich fields expand their authored template using owned args or
context bindings. Failed resolution/validation leaves the previous player state,
locale and presentation intact. Callbacks cannot mutate the same player.

The owned `node.presentation` contains:

- `text`: final plain text, equal to `node.text`.
- `runs`: ordered inclusive `firstCluster`/`lastCluster` ranges with `emphasis`,
  `styles`, optional `rate` and optional `pronunciation`.
- `pauses`: ordered `afterCluster`/`seconds` pairs, including leading boundary 0.
- `origins`: final inclusive UTF-8 `startByte`/`endByte`, selected-template
  `templateStartByte`/`templateEndByte`, and optional placeholder `argument`.
- Optional actual `resolvedLocale` and plural `variant`.

Choice labels use `option.presentation`; disabled reasons use
`option.reasonPresentation`. Empty substitutions have `endByte = startByte - 1`.
Source mappings identify bytes in the selected template, not positions in the
original-language prose. They do not invent source filenames for dynamic callbacks.
Runs index logical graphemes, not visual glyphs. Marks cannot divide a final
combined grapheme, including across substitutions. Renderer/layout shaping and
bidirectional ordering remain host responsibilities.

Pass the snapshot to `Dialogue.reveal` normally. Leading pauses delay the first
cluster; trailing pauses keep `complete` false after all text is visible. Remaining
time carries across pauses and rate changes in the same update. `finish()` and
reduced motion skip pauses silently; after manual finish immediately call
`player:update(0, true)`. Locale/context refreshes that force complete text also skip
pauses. Speech cues remain coalesced to at most one per update. New timing requires
a new presentation identity; invalid reveal input preserves the previous view.

Limits per field: 65,536 UTF-8 template/resolved bytes, 4,096 tokens, 16 nested
marks, 256 pauses and 120 seconds of total pauses. Pronunciation hints are at most
256 bytes. Plain renderers may ignore all styles and draw `node.text`; no fonts,
images, shaders or audio resources are loaded by the dialogue runtime.

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
or nil. Named `expressions` are described below. Other fields remain available in the character module but are not copied
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

### Give repeated events stable identities

Use an explicit event ID when the host needs to distinguish two calls with the
same name. Declare a logical document ID before the first label:

```text
@dialogue 1
@document arrival
[start]
// @id arrival.greeting
: Welcome aboard! (end)
@event shown 0 portrait.pulse [id:arrival.first-pulse] "soft"
@event revealed 0.2 portrait.pulse [id:arrival.second-pulse] "bright"
```

The optional `[id:...]` goes after the event name and before its string payload.
IDs use the same identifier syntax as labels and must be unique across the entire
document. Text IDs and event IDs are separate namespaces. Rewording a line,
reordering events or renaming the script file does not change an explicit ID.
Changing the authored ID means changing the identity; keep document IDs unique
within your project when combining records from multiple scripts.

For identified events, `player:update` additionally returns:

| Field | Meaning |
| --- | --- |
| `documentId` | Logical ID from `@document` |
| `eventId` | Explicit event ID |
| `source` | Owned `{line, column}` pointing to the event directive, both one-based |

Source coordinates are for diagnostics and can change after edits. Use
`documentId` + `eventId` for authored identity, and the existing `generation` +
`visit` tokens for a delivery within one player. Tokens are scoped to each player;
keep the player identity as well when combining concurrent sessions.

The host still decides how to handle an event. IDs do not persist state, provide
exactly-once delivery across saves, or suppress events on a new visit. Cancelling
or advancing discards pending events; locale refresh does not replay fired ones.
Mutating a document, snapshot or returned source record cannot change a player's
captured identity. Events without IDs keep their existing returned record shape.

Assets with event IDs emit schema **4**; use the paired runtime or check
`Dialogue.schemaVersion >= 4` when supplying a replacement. Earlier document
schemas remain supported. Programmatic schema-4 documents use `id` on event
records and require `documentId`; `source` is optional and, when supplied, must
contain only positive integer `line` and `column` fields. Duplicate IDs are rejected
before playback. See the runnable
[event identity example](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue-event-identity).

### Declare typed notification payloads

Use a shared event registry when a string payload is too limited. This builds on
`Save.shape`'s headless validation and copying; it does not create a save slot or
require a signal bus.

`events.tua`:

```tua
return Dialogue.events({
  reward = Save.shape(Save.record({
    coins = Save.number(0),
    item = Save.record({ id = Save.string("") }),
    bonus = Save.optional(Save.boolean(false)),
  })),
})
```

In `victory.tdialogue`, import that registry before labels, then use `@emit` for
declared notifications:

```text
@dialogue 1
@document victory
@events "events"
[start]
// @id victory.greeting
: Well done! (end)
@emit shown 0 reward [id:victory.reward] {"coins":10,"item":{"id":"shell"},"bonus":false}
@event revealed 0.2 sound.play "celebration"
```

Payload strings are literal: they are not localized or placeholder-expanded.
The JSON record is data, never executable Lua. Nested records, arrays, strings,
finite numbers and booleans are supported. Omit optional fields instead of writing
`null`. Payloads allow at most 8 KiB of authored JSON, 16 nesting levels and 256
values. Duplicate keys are errors. The existing limit of 64 notifications per line
counts both `@emit` and legacy `@event` entries.

Event names, required/unknown fields and payload types are checked against the
imported module, including re-exports. Completion suggests event names and top-level
JSON argument keys; hover shows their types and definition lookup opens the contract.
Numeric bounds and other `Save.shape` constraints are enforced at runtime before
playback. A registry supports 1–128 event names. Use direct `Dialogue.events(...)`
and `Save.shape(...)` factory calls to retain specialized inference.

Create the player through the registry. Typed readers return an owned, validated
payload for their event, or `nil` for another notification:

```tua
local events = require("events")
local scene = require("victory")
local player = events.new(scene)
player:start()
for _, event in ipairs(player:update(0, false)) do
  local reward = events.read.reward(event)
  if reward then
    print(reward.coins, reward.item.id)
  end
end
player:dispose()
```

Declared records carry `kind` and `name`, both equal to the declared name, plus the
structured `payload` and existing identity/delivery metadata. Legacy notifications
keep a string `payload` and no `kind`. Use the typed reader to access payload fields;
checking `event.kind` alone does not currently narrow sibling payload fields in Tua.
A matching reader validates again and copies its result, so it also rejects a
payload that the host has since corrupted. A wrong kind/name returns `nil`.

Payloads are captured when the player is constructed, not when their delay expires.
Changing context, locale, schema defaults, the original document, a snapshot or an
already-returned payload cannot change a pending notification's captured data.
Registry validators use the original `Save.shape` descriptors, not mutable public
`copy` or `defaults` fields. Each emitted delivery owns its nested payload.

These are notifications: handling them does not wait, branch, acknowledge a reward
or catch host-handler errors. If a handler restarts/cancels a player, use the
existing generation/visit tokens to reject stale records from the returned batch.
Registered actions can select an outcome target; persistent resume policy remains planned work.

To combine events with context, pass the **same registry instance** in the context
module's `events` field:

```tua
local events = require("events")
return Dialogue.context({
  events = events,
  facts = Save.shape(Save.record({ name = Save.string("") })),
  locals = Save.shape(Save.record({ visits = Save.number(0) })),
})
```

The script declares both `@context "context"` and `@events "events"`. Construct it
with `context.new(scene, facts)`; the same `events.read` functions handle its results.
Registry identity is checked at construction. Plain `Dialogue.new` rejects a
registry-bound script so the host cannot accidentally use its legacy result type.

An `@events` import emits schema **5** and requires `Dialogue.schemaVersion >= 5`.
Earlier schemas still work. Programmatic events in such documents use
`{kind = "reward", name = "reward", id = "victory.reward", anchor = "shown", delay = 0,
payload = {coins = 10, item = {id = "shell"}}}` and set the document's `events` to the
registry. Unknown events and invalid payloads fail during player construction.
See the complete [typed events example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue-events)
for multiple event types, legacy notifications, re-exports and context composition.

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
Rich-text effects, assignments and inline executable commands are not implemented.
Character expressions and conditional branches use their explicit directives. Named `@event` metadata is
supported as described above; its behavior comes from host handlers. Named
placeholders are validated; other unescaped braces and brackets remain errors.
Without a resolver, the player presents decoded source text literally.

## Select character expressions

Declare named expression records on a shared character. Each expression needs a
nonempty `portrait` asset ID and may name an `animation` for the host's adapter.
Keep the record fields visible in its type (or use inference) so the editor can
suggest the names. A widened dictionary cannot describe a closed set of names.

```tua
return {
  Guide = {
    name = "Guide",
    portrait = "portraits/guide.png",
    expressions = {
      happy = { portrait = "portraits/guide-happy.png" },
      talking = { portrait = "portraits/guide-sheet.png", animation = "talk" },
    },
  },
}
```

Place `@expression` after its utterance and before any choices. It works on an
imported character, including a re-exported character; narration and local
`@character` declarations have no expression registry.

```text
@dialogue 1
@characters Cast "characters"
[start]
// @id greeting.happy
Cast.Guide: Welcome!
@expression happy
// @id greeting.default
Cast.Guide: Back to my default portrait. (end)
```

`snapshot().node` exposes `expression`, the resolved `portrait`, and optional
`portraitAnimation`. Omission resets the next line to the character's default
portrait, or no portrait if there is none. Selection never persists implicitly
between lines, speakers, or players. Repeat the directive when another line needs
the same expression. Locale changes preserve selection on the current line.

Unknown names and incompatible selected metadata types produce authoring diagnostics.
Nonempty-string constraints validate at construction. Dynamic Lua documents also
validate there and reject unknown names rather than silently falling back. The selected portrait and animation strings are copied into each
player; later edits to the shared record or a returned snapshot cannot change an
existing player. No sprite timer or GPU resource is stored in the shared cast.

Expression name completion works while typing a directive. Hover shows its metadata
type; go-to-definition opens the owning character declaration, including re-exports.
These queries inspect source facts and do not load images. Asset IDs remain opaque
strings: the host resolves them and handles missing files or unsupported animations.
The compiler does not prove that an animation name exists in an arbitrary host
registry. Existing sprite-sheet calls retain their own animation-name checking.

Scripts selecting expressions emit schema **7**, requiring
`Dialogue.schemaVersion >= 7`. Portrait-only scripts keep their previous schema,
and the new runtime accepts older schemas. See the runnable
[expression example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue-expressions)
for two speakers, two players, and reset behavior.

### Optional static-image adapter

Load/cache images in the host, then use the resolved asset ID. This LÖVE recipe
assumes the named image exists; adapt missing-asset handling to your game:

```tua
local images = {
  ["portraits/guide-happy.png"] = love.graphics.newImage("portraits/guide-happy.png"),
}

local node = player:snapshot().node
if node then
  local portrait = node.portrait
  if portrait then
    local image = images[portrait]
    if image then love.graphics.draw(image, 24, 24) end
  end
end
```

Share cached image resources if useful. Keep layout, tint, and transitions local to
each dialogue presentation. Image lifetime and cleanup remain the host's responsibility.

### Optional animated adapter with `spriteSheet`

Use an ordinary [sprite sheet](sprites.md) and create a **separate sprite player
for each dialogue presentation**. The following recipe assumes a sheet containing
four 32 × 32 frames in its first row; it is not a required dialogue runtime:

```tua
local GuideSheet = spriteSheet({
  image = "portraits/guide-sheet.png",
  frame = { width = 32, height = 32 },
  animations = { talk = { from = 0, to = 3, row = 0, fps = 8 } },
})

local leftPortrait = GuideSheet:player("talk")
local rightPortrait = GuideSheet:player("talk")

-- In the left presentation's update, after a new snapshot is accepted:
local node = player:snapshot().node
if node and node.portrait == "portraits/guide-sheet.png" then
  if node.portraitAnimation == "talk" then
    leftPortrait:play("talk", true)
  else
    leftPortrait:stop()
  end
end

-- In love.update(dt), advance each active presentation independently.
leftPortrait:update(0.1)
rightPortrait:update(0.05)
-- In love.draw(), draw only the active animated presentations.
leftPortrait:draw(24, 24)
rightPortrait:draw(200, 24)
```

Call selection/restart once when `(generation, visit, presentationRevision)` changes,
not on every frame; otherwise `play(..., true)` continually restarts the animation.
Hide or discard the previous sprite player when the new snapshot has no animated
portrait, or when the dialogue cancels/disposes. Switching to a static expression
must also stop showing the old animation. The host may choose whether a locale-only
presentation revision restarts animation. Never cache sprite players on characters;
only cache the sheet/image resources. This keeps simultaneous conversations independent.

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

### Select a template before substitution

`I18n.dialogueTemplate(key, source, args, locale)` selects from the same catalogs
and validates placeholder names, but leaves `%{name}` and `%%` unexpanded. This is
useful when a presentation layer must process a template before inserting literal
argument values. It returns an owned record with `template`, optional `locale`,
and optional `variant` (the plural form actually used).

```tua
local selected = I18n.dialogueTemplate(
  "harbor.hello",
  "Hello, %{name}! You have %{count} coins.",
  { name = "Ada", count = 1 },
  "fr-CA"
)
-- selected.template still contains %{name} and %{count}.
-- With the example catalogs: locale is "fr", variant is "one".
print(selected.template)
```

The `args` map is used for plural selection; other placeholder values may be
absent until a later expansion step. Plural entries require a finite `count`.
The result reports the actual catalog locale after fallback, not the requested
locale. Authored-source fallback has neither locale nor variant; scalar catalog
entries have no variant. A missing plural category reports `"other"` when that
form is used. Calls do not change the global locale or expose mutable catalog data.

Use `I18n.dialogue` with `Dialogue.new(..., { resolve = ... })`: that callback must
return a final string. `dialogueTemplate` returns a record and is **not** a drop-in
resolver. Use it as `resolveTemplate` for rich schema-8 fields instead. This selector
does not validate markup itself; the dialogue player validates the selected template.

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

## Conditions, branches and available choices

Conditions read the latest **accepted** context and locals. Mutating the original
host table has no effect until `updateContext` or `setLocals` accepts a replacement.
The [conditional shop example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue-conditions)
builds and runs without graphics or audio.

### Route through a branch label

For this section, define the shop's facts in their own module:

```tua title="src/shop/context.tua"
return Dialogue.context({
  facts = Save.shape(Save.record({
    coins = Save.number(0),
    unlocked = Save.boolean(false),
    secretUnlocked = Save.boolean(false),
    bonus = Save.optional(Save.number(0)),
  })),
  locals = Save.shape(Save.record({ visits = Save.number(0) })),
})
```

```text
@dialogue 1
@context "shop.context"
[gate]
@branch context.unlocked and context.coins >= 20 [target:shop] [else:closed]
[shop]
// @id shop.welcome
: Welcome! (end)
[closed]
// @id shop.closed
: Come back later. (end)
```

A branch is the only content of its label. Both destinations are required; forward
references are allowed and checked. It advances automatically, without adding a
blank utterance or requiring input. Ordinary lines still wait for acknowledgement.

The small expression language supports `context.field` / `locals.field`, finite
numbers (including negative numbers), double-quoted JSON strings, `true`, `false`,
`nil`, parentheses, `not`, `and`, `or`, `==`, `~=`, `!=`, `<`, `<=`, `>` and `>=`.
Calls, indexing, arithmetic and assignments are rejected. Analysis never runs the
imported context module.

Precedence, highest first: parentheses, `not`, comparisons, `and`, `or`. Boolean
operators accept booleans or nil and return booleans; nil means false, while numbers
and strings are not implicitly truthy. `and`/`or` evaluate left to right and skip
unneeded operands. Comparisons require matching scalar types; ordered comparisons
are numeric only. This avoids language/locale-dependent string ordering.

Optional paths yield nil if an intermediate value is absent. Equality can test nil;
an ordered comparison with a missing value is false. For example,
`context.bonus != nil and context.bonus > 0` and `context.bonus > 0` both reject a
missing bonus. Use `not (context.coins >= 20)`, rather than `not context.coins >= 20`.
Unknown paths and wrong operand types are diagnosed even in an unreachable operand.
Completion, hover and definition navigation work on condition paths across imports.

### Hide or disable a reply

```text
[shop]
// @id shop.prompt
: What would you like?
// @id shop.buy
-> Buy [target:bought]
@enabled context.coins >= 20
// @id shop.buy.reason
@reason Requires 20 coins.
// @id shop.secret
-> Secret offer [target:bought]
@visible context.secretUnlocked
// @id shop.leave
-> Leave [target:done]
@fallback closed
```

Declare `@visible` / `@enabled` immediately after their choice; each may occur once.
An omitted condition means true. A hidden reply is absent from snapshot options.
A visible, disabled reply has `enabled = false` and cannot be chosen. Its optional
`reason` is localized using the preceding stable `// @id`; it uses the same bindings,
translation extraction and coverage checks as line/choice text. Enabled replies omit
the reason. Schema-three snapshots expose `enabled = true` for available replies;
older snapshots may omit it, so compatible renderers treat only `false` as disabled.

Conditional choice groups require `@fallback label`. If no **visible and enabled**
reply remains, entering or refreshing the group automatically follows that target.
To let players read disabled reasons, include an enabled exit reply. This is an
explicit branch, not an implicit selection of the first disabled option.

`choose(id)` rechecks the current accepted facts and returns false for a hidden,
disabled or unknown reply. An old UI snapshot never grants permission. The host must
still validate and apply gameplay actions such as spending money; dialogue navigation
does not perform purchases. Updating facts refreshes the current group without
replaying events or restarting the visit unless its fallback changes the current
node. A failing resolver leaves the previous facts and presentation intact.

### Bound automatic work and inspect faults

```tua
local player = context.new(scene, facts, { transitionBudget = 32 })
player:start()
local view = player:snapshot()
if view.issue then
  print(view.issue.code, view.issue.nodeId)
end
```

The default budget is 256 automatic edges per transition/refresh; `transitionBudget`
accepts integers from 1 to 4,096. Branch and no-choice fallback edges consume it.
A loop that exceeds the budget enters `faulted` with an owned issue record:
`{code = "transition_budget_exceeded", message, nodeId}`. No partial line events
are emitted. `advance` and `choose` return false; `update` returns no events.
Changing facts or locale does not restart a faulted player. Inspect it, then use
`start` to retry explicitly, `cancel`, or `dispose` to clean up. Restart resets locals
under the existing M2 rules; faults do not become save/checkpoint support.

Conditions are limited to 8 KiB of authored expression text, 256 lexical tokens,
32 AST levels and at most 256 emitted instructions. Runtime documents validate the
instruction/depth limits too. The evaluator reads owned plain records and never uses
`load`, `loadstring`, a host method, or a wall clock. Each automatic edge evaluates
bounded data, and a refresh resolves its route once.

Assets using conditional flow or reasons emit schema **3**, requiring
`Dialogue.schemaVersion >= 3`. Adding event IDs upgrades the asset to schema **4**
and requires `Dialogue.schemaVersion >= 4`. The authoring header remains
`@dialogue 1`. Without event IDs, context-only assets still emit schema 2 and plain
assets still emit schema 1. Existing line, event, localization and context APIs
remain compatible.

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

## Run a registered host action

Use an action when host code must return an outcome before dialogue continues.
Timed events remain notifications; `update()` never invokes an action handler.
The runnable [dialogue-actions example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue-actions)
includes standalone actions and composition with context and typed events.

Declare `actions.tua`:

```tua
return Dialogue.actions({
  purchase = {
    payload = Save.shape(Save.record({ item = Save.string("") })),
    effect = "consequential",
  },
})
```

Reference it from `scene.tdialogue`. Each action occupies its own label and needs
an explicit ID, document ID, JSON record, and both outcome targets:

```text
@dialogue 1
@document shop
@actions "actions"
[buy]
@action purchase [id:shop.purchase] [success:done] [failure:declined] {"item":"shell"}
[done]
// @id done.text
: Purchased. (end)
[declined]
// @id declined.text
: Not available. (end)
```

Register the host handler when constructing the player:

```tua
local actions = require("actions")
local scene = require("scene")
local coins = 10
local player = actions.new(scene, {
  handlers = {
    purchase = function(payload: { item: string }): boolean
      if payload.item ~= "shell" or coins < 5 then return false end
      coins -= 5
      return true
    end,
  },
})
player:start()
-- The host decides when it is safe to perform the pending operation.
if player:snapshot().state == "awaiting_action" then
  player:runAction()
end
```

`true` selects `success`; `false` selects `failure`. The handler may accept a
second request argument with `actionId`, `name`, `effect`, `documentId`, `nodeId`,
`generation`, `visit`, and optional source coordinates. `snapshot().action` exposes
that metadata while waiting; `snapshot().node` is absent. Payload and request
records are owned copies. Registry validation captures payloads at construction,
and each player captures its handler map.

Only `runAction()` executes a handler, at most one per call. Traversal, snapshots,
locale changes, and updates do not dispatch it. A chain of action labels therefore
requires separate calls. Handlers must return synchronously; asynchronous handles
and coroutine-based actions are not supported yet. Reentrant dispatch returns
`false`. Cancellation, restart, or disposal inside the handler invalidates its
result, including any error returned afterward.

A missing handler, thrown error, or non-boolean result faults the player with
`action_handler_missing`, `action_handler_error`, or `action_result_invalid`.
An error resolving the destination faults with `action_transition_error`.
`snapshot().issue` carries source/action identity. These failures do not retry:
inspect the issue and explicitly cancel, dispose, or restart after reconciling any
side effects. `runAction()` returns whether dispatch successfully reached its
outcome, not the handler's boolean result.

Choose `effect = "presentation"` for cosmetic host work or `"consequential"` for
changes such as spending currency. This classification is metadata, not rollback
or an exactly-once guarantee. A handler can change the world before it fails;
the host owns reconciliation. Save/resume of pending actions remains future work.

For context plus actions, pass the registry as `actions` to `Dialogue.context`,
include matching `@context` and `@actions` imports, and supply `handlers` in the
third argument to `context.new`. An optional `events` registry composes the same
way. The script and constructor must reference the same registry instances.

Action names and JSON payloads validate through imported/re-exported registry
facts. Completion, hover and definition navigation cover names and top-level
payload keys. Dynamic Lua documents validate at construction. Action IDs share a
uniqueness namespace with event IDs and remain stable across text edits.
Action imports emit schema **6**, requiring `Dialogue.schemaVersion >= 6`;
older document schemas remain accepted by the new runtime.
