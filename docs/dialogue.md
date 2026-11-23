---
icon: lucide/messages-square
---

# Dialogue

The optional `tuadialogue` player handles conversation flow independently of
rendering, input and wall-clock time. It is available in development for the release after
0.11.0. The `.tdialogue` frontend integrates with project checking, building and
watching. Shared character modules support script completion, hover and definition
navigation in the updated Tua extension. Translations remain pending.

## Use the player

Use the typed `Dialogue` builtin in Tua:

```tua
local player = Dialogue.new({
  entry = "hello",
  nodes = {
    hello = { kind = "line", text = "Welcome aboard.", next = "finish" },
    finish = { kind = "end" },
  },
})
player:start()
local view = player:snapshot()
local node = view.node
if node then print(node.text) end
player:advance()
player:dispose()
```

Referencing `Dialogue` supplies type checking and editor assistance and injects
`local Dialogue = require("tuadialogue")` into emitted Lua. Unused files do not
include the runtime. Local bindings named `Dialogue` take precedence. Ordinary
Lua callers and existing Tua code can still use the explicit import. A project module with the same name can replace the bundled
runtime. Documents and snapshots are defensively copied. `start()` selects the
entry node; `start("label")` selects a named node. `advance()` acknowledges a line;
`choose("option-id")` selects a choice by stable ID. `cancel()` ends the current
session; `dispose()` is terminal for that player.

A snapshot contains `state`, `generation`, `visit`, optional `nodeId` and optional `node`.
A present node always has `text` and `options`, defaulting to an empty string and
array. Its `speaker` and `next` fields remain optional. The host owns text reveal,
layout, choices, pause policy and any optional temporary-message timing. Ordinary
conversations never expire automatically.

## Author a script

The first supported subset requires an explicit version header:

```text
@dialogue 1
@character Guide "Guide"

[start]
// @id intro.welcome
Guide: Ready to sail?
// @id intro.yes
-> Yes [target:depart]
// @id intro.repeat
-> Ask again [target:start]

[depart]
// @id intro.goodbye
: Good hunting! (end)
```

The first label is the entry. Speakers are declared before labels. An utterance
uses `Speaker: text`; `: text` is narration without a speaker. Each line and
choice needs a `// @id identifier` annotation. Blank lines and ordinary `//`
comments may separate the annotation from its text. An ID cannot cross a label,
speaker declaration or another ID annotation.

Identifiers start with an ASCII letter or underscore and then use ASCII letters,
digits, underscores, dots or hyphens. Text and speaker display names accept
Unicode. Text IDs must be unique across all valid dialogue documents in the project. Keep IDs stable when wording or order
changes. Labels and speaker IDs have their own namespaces. `else` and `endif`
are reserved labels for future structured conditions.

Consecutive utterances advance in order within a section. Choices attach to the
preceding utterance and follow its acknowledgement. A section must finish with
choices or a line ending in `(end)`; sections do not implicitly fall through.
Choices can target later labels or form loops. A terminal utterance cannot have
choices, and another utterance after choices or an ending needs a new label.
Named label starts remain available through `player:start("label")`.

## Share characters between scripts

Define characters as exports of an ordinary typed module, such as
`src/dialogue/characters.tua`:

```tua
type Character = { name: string, portrait: string? }
local Guide: Character = { name = "Harbor guide", portrait = "portraits/guide.png" }
local Captain: Character = { name = "Captain" }
return { Guide = Guide, Captain = Captain }
```

Import that module before the first label in any number of scripts:

```text
@dialogue 1
@characters Cast "dialogue.characters"

[start]
// @id harbor.welcome
Cast.Guide: Welcome aboard. (end)
```

`Cast` is a script-local alias; `Guide` is the module's exported member. Each used
member must be a record with a required string `name`. Ordinary Tua inference,
type aliases and module re-exports apply. Missing modules, unknown members,
optional names and non-string names are `TL5012` errors at the script reference.
Unrelated module exports are allowed. Alias names must be simple identifiers;
module names use extensionless dot-separated project paths. Multiple aliases are
allowed, but cannot collide with local speaker declarations. Existing inline
`@character Guide "Guide"` declarations remain supported.

The updated VS Code extension recognizes `.tdialogue`, highlights script syntax,
completes character members after `Cast.`, shows their inferred types and comments
on hover, and navigates to the exported declaration. These queries also work with
unsaved character edits and unfinished speaker references. Character modules get
normal Tua editing support. Script formatting and cross-language rename are not
implemented; the extension does not apply the Lua formatter to dialogue prose.

Analysis never executes character modules. Emitted scripts load them using normal
Lua `require` and read each character's `name` when the document module loads.
A module can initialize a typed name dynamically. Updating a character name does
not mutate existing player snapshots or invalidate Lua's loaded-module cache.
The optional `portrait` field must be a string (or nil). The emitted node copies
it alongside the display name (`speaker`); snapshots retain both. A portrait is
an opaque asset identifier: the host decides how to load and draw it. No images
are loaded by analysis or the headless player. Other character fields stay in
the character module.
Renderer customization stays with the host.

## Timed custom events and presentation duration

Attach metadata immediately after an utterance, before its choices or the next
utterance. Metadata does not need a text ID:

```text
Cast.Guide: Welcome aboard. (end)
@timing hold 3 fade 0.35
@event shown 0.2 portrait.pulse
@event revealed 0.25 sound.play "welcome"
```

`@event <anchor> <seconds> <name> ["payload"]` accepts `shown` (time since line
entry) and `revealed` (time since the host reports that reveal finished). Delays
are finite, nonnegative seconds. Names use the identifier grammar; the optional
quoted string payload defaults to `""` and supports the same escapes as text.
There are at most 64 events per line. Events contain data, never executable Lua.
Unknown names are for the host to handle or ignore; no built-in sound or graphics
command is implied.

After updating your renderer, call `player:update(dt, fullyRevealed)`. It returns
an array of due `{ name, payload, generation, visit, nodeId }` records. Register
handlers in your host and dispatch these records yourself. The first `true`
sample starts the revealed clock at zero; that frame's `dt` is excluded from this
clock. Call `update(0, true)` when an input instantly finishes reveal. Once reveal
has been reported, its clock continues even if a later sample is false. Pause
both the renderer and player updates together to suspend timing.

Events fire once per line visit. Due events are ordered by their scheduled time,
then declaration order for ties. Advancing, choosing, restarting, cancelling or
disposing discards pending events from the old visit. `visit` increases on every
node entry, including loops; `generation` distinguishes sessions. If a handler
changes the session, compare those tokens before dispatching the remainder of an
already-returned batch. Players own independent clocks and document copies.

`@timing hold <seconds> fade <seconds>` is optional, unique per line, and adds
`hold`/`fade` to its snapshot. Both durations must be finite and nonnegative.
These are presentation hints: a temporary-message host may hold after reveal,
then fade; ordinary conversations can continue waiting for acknowledgement.
The player never automatically advances or pauses gameplay. Portrait rendering,
reveal speed, event handlers and duration policy remain customizable host concerns.

Structured documents can supply the same line fields: `portrait?: string`,
`hold?: number`, `fade?: number`, and
`events?: { anchor: "shown" | "revealed", delay: number, name: string, payload: string }[]`.
The runtime validates them and copies them before use. Invalid update arguments
(negative/nonfinite time or a nonboolean reveal flag) raise an error.

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

Interpolation, rich-text effects, expressions, conditions, assignments and
commands are not implemented in this subset. Unescaped braces and brackets in
text are rejected so planned constructs are not silently treated as working
features. Translations and placeholder validation remain pending; stable text IDs
and source text are retained in the parsed model for that integration.

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
