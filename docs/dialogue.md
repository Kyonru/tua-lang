---
icon: lucide/messages-square
---

# Dialogue

The optional `tuadialogue` player handles conversation flow independently of
rendering, input and timing. It is available on the dialogue feature branch for
the release after 0.11.0. The `.tdialogue` frontend integrates with project checking, building and
watching. Dedicated dialogue editor support and translations remain pending.

## Use the player

Import the runtime explicitly in a Tua module:

```tua
local Dialogue = require("tuadialogue")
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

The import supplies type checking, editor assistance for the Tua API, and opt-in
runtime emission. A project module with the same name can replace the bundled
runtime. Documents and snapshots are defensively copied. `start()` selects the
entry node; `start("label")` selects a named node. `advance()` acknowledges a line;
`choose("option-id")` selects a choice by stable ID. `cancel()` ends the current
session; `dispose()` is terminal for that player.

A snapshot contains `state`, `generation`, optional `nodeId` and optional `node`.
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
local Dialogue = require("tuadialogue")
local document = require("scenes.intro")
local player = Dialogue.new(document)
player:start()
```

`tua check` validates scripts and typed imports. `tua build` emits
`scenes/intro.lua` and source maps when enabled. Imports retain the document
contract through Tua re-exports and support Tua hover/completion. Merely importing
data does not bundle the player runtime; `require("tuadialogue")` does.
Avoid source files that produce the same Lua output path. Out-of-place builds
reject dialogue/Lua and dialogue/Tua collisions. In-place builds retain the
existing generated-Lua sibling convention.

`tua watch` validates edits before replacing the last valid generated module.
Deleting a source removes its generated module and maps. A dialogue change
revalidates all project dialogue scripts so duplicate IDs can be detected and
repaired across files. Build caching includes authored scripts. The language server
watches dialogue assets on disk; dedicated script highlighting, completions and
unsaved dialogue editing remain a later milestone. Rebuilding data does not itself
replace a running player's owned document or reload Lua's module cache.

## Compiler frontend

`tua_core::dialogue_asset::parse_dialogue_source(path, source)` returns the typed
speaker/section/line/choice model and source ranges without reading files or
executing project code. `tua_core::emit::emit_dialogue_source(path, source)`
validates that model and returns an ordinary Lua data module with per-line source
provenance. The output is accepted by the same `Dialogue.new(document)` API.
It contains no runtime import or executable dialogue text.

Diagnostics use `TL5012` and one-based Unicode scalar columns with exclusive
range ends. Unknown references, duplicate IDs/declarations, incomplete flow and
unsupported syntax prevent output. Generated internal node IDs use an `@` prefix
that authored identifiers cannot use. The shared parser serves project checking and build/watch emission; it does not
execute dialogue text.

See the [dialogue example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue)
for the structured host and frontend development harness. The format is Tua-owned;
compatibility with Love-Dialogue `.ld` files is not implied.
