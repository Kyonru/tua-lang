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

**New to dialogue?** Start with [your first playable dialogue](tutorials/first-dialogue.md):
one question, two replies, two source files. It explains the defaults before
introducing optional capabilities. This page is the detailed reference.

| When you are ready | Read next |
| --- | --- |
| Reuse characters and show portraits | [Shared characters](#share-characters-between-scripts) |
| Personalize words and replies | [Context](#typed-context-and-conversation-locals), then [conditions](#conditions-branches-and-available-choices) |
| Add presentation | [Reveal](#reveal-text-by-grapheme-cluster), [rich text](#rich-presentations-and-timing), then optional [audio](#describe-voices-and-character-chatter) |
| Ship localized content | [Translations](#localize-lines-choices-and-character-names) |
| Trigger gameplay and resume later | [Actions](#run-a-registered-host-action), [checkpoints](#checkpoint-a-plain-conversation) |
| Explore complete UI treatments | [Showcase](#try-the-love-showcase) |

## Your first conversation

Prefer a terminal? This walkthrough needs no LÖVE window, font or image assets.
For a window with interactive replies, use the [playable starter](tutorials/first-dialogue.md).

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
The [runnable example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/basics)
also demonstrates loops, shared characters and both event clocks.

## Try the LÖVE showcase

[Beacon Harbor](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/showcase)
uses the same player for seven presentations: the original cinematic, storybook,
transmission and subtitle styles, plus mythic, tactics and terminal frames. Lorelei
character portraits retain their separate slate-and-sage backgrounds.
The longer story includes a question hub, three outcomes, and a relationship-dependent
ending variant. Portraits can sit left, right, above the panel, or be hidden.
It includes a second independent announcement player, English/French templates,
shared characters, conditional replies, typed actions, chatter, voice, waits,
checkpoint restore and a bounded transcript.

From the repository root:

```sh
cd examples/dialogue/showcase
tua check
tua build
love build
```

Press **1–4** for the original styles, **5–7** for the additional reference-based
styles, and **O** to cycle portrait placement.
**H** opens controls/diagnostics, **Tab** opens the scrollable recent transcript, and
**F2** switches between the longer story and the short feature demonstration.
**F3** opens a rich-text tour with nested emphasis, highlights, underline/strike,
thought/whisper text, colors, motion, scoped reveal speed and pauses. These are
showcase renderer treatments built on the existing rich-text marks.
**F4** opens a separate character-staging conversation: opposite portrait slots,
speaker-specific bubble anchors, portrait-free speech and a per-line position override.
The typed host layout uses stable speaker/text IDs and survives language changes.
**Space** finishes reveal, then advances;
**Up/Down** and **Enter** select replies. **F** changes reveal speed, **M** reduces
motion, **S** toggles subtitles and **V** mutes recorded voice. **L** switches
language, **P** pauses, **R** resets, **Q/N** queue conversations/announcements,
and **F5/F9** capture/restore an in-memory checkpoint. **[ / ]** scroll content (Page Up/Down remain aliases). The recent-conversation
panel also has clickable Older/Newer buttons and a scroll-position indicator.

The F4 staging tour always reserves modal priority. Elsewhere, cinematic, mythic
and tactics modes give the conversation modal priority and suspend radio; the other
styles permit concurrent announcements. The background stays quiet so dialogue is
the focus. These are example host policies. The engine does not decide
whether your game pauses or which channel owns input. Two conversation and four
announcement waiting slots reject overflow. A restore creates a fresh player and
stops stale presentation/audio; it does not replay a settled token action.

The checkpoint demonstration is session-local. It is not a crash-safe gameplay
save: use the [durable-action recipe](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/durable)
when dialogue changes persistent inventory or rewards. Host preferences, transcript
privacy and external effects remain the game's responsibility.

### Borrow one adapter at a time

| Need | Start with |
| --- | --- |
| Plain conversations | `snapshot.node.text`, explicit `advance`/`choose`; [basics](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/basics) |
| Custom styled text | `presenter.tua`: semantic run names, owned fonts, portrait layouts, wrapping and scroll metrics |
| Chatter or recorded voice | `audio.tua` plus an injected backend; [focused audio example](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/audio) |
| Non-pausing messages | A separate player; optionally borrow the bounded host queue |
| Headless testing | `controller.tua` with fake backends, executed by `tests/controller.lua` |

The example's fonts and Sources are created by the LÖVE host and released on
shutdown. Reduced motion disables wave/shake and completes text immediately.
Subtitles and keyboard focus are explicit controls. This is not a screen-reader
bridge or a claim of bidirectional layout/full text shaping.

### Presentation inspiration and inline images

[SYSL-Text](https://github.com/sysl-dev/SYSL-Text) is useful inspiration for named
presentation styles, per-textbox defaults, wrapping/scrolling, animated text and
character sounds. Beacon Harbor demonstrates those ideas with the existing Tua
run/reveal contracts; it does not embed SYSL-Text or execute inline function tags.
Its `wave`, `warning` and `radio` style names are host mappings, not new core tags.
Unknown style names fall back to ordinary readable text. Actions stay declared in
`.tdialogue` and are dispatched through typed handlers.

Inline icons/images are intentionally **not added to the grammar** in this
milestone. Before adopting them, a host should define all of the following:

- Intrinsic size, baseline, line-height contribution, wrapping and missing-asset behavior.
- A localized spoken/plain-text alternative that remains meaningful in history,
  subtitles, translation extraction and renderers without image support.
- Grapheme/reveal and voice timing: whether an icon consumes a reveal step, and
  what happens on instant reveal, skip and restore.
- An allowlisted resource mapping and explicit ownership; no file paths or code
  execution in untrusted prose.

For now, use a character portrait or render a host icon beside an ordinary localized
word such as “token.” A decorative icon can be omitted; a meaningful one must not
replace the only readable description of the action. Any future core syntax needs
separate layout, localization, accessibility and lifecycle tests.

## Navigate conversation flow

In a `.tdialogue` file, request completion inside a target name to see labels in
that file. Hover explains the label; Go to Definition jumps to its declaration.
This works for choice/wait `target` tags, branch `target`/`else` tags, action
`success`/`failure` tags, and `@fallback` names.

```text
-> Continue [target:shop.done]

[shop.done]
// @id shop.farewell
: See you next time. (end)
```

For example, completing `shop.` in the target replaces the whole target name,
including any existing suffix. It leaves the surrounding tag and prose intact.
Suggestions remain available while a target is unfinished and update from unsaved
label edits. Duplicate declarations are ambiguous: navigation and completion do
not choose between them. Text inside comments, spoken prose, and action JSON
payloads is not a flow reference.

These queries only inspect source. They never run conditions or action handlers.
Use **Find References** or **Rename Symbol** on a label or one of its targets to
update that script's flow. Rename accepts the complete label, including dots and
hyphens (for example, `shop.done-end`). It updates all choice, wait, branch,
action and fallback targets without changing prose, text IDs or JSON payloads.
References include the label only when Include Declaration is enabled.

Refactoring requires a syntactically valid script with unique labels and valid
targets. Existing destination labels and reserved names `else`/`endif` are rejected.
Unsaved edits participate in validation. Other scripts have independent labels;
host entry-point strings and saved checkpoints are not migrated by this file-local
rename. Update those external references separately before using the renamed flow.
Graph preview remains planned.

## Find authored voice cues

Inside the JSON `cue` string of an `@voice` directive, request completion to reuse
cue IDs from valid dialogue scripts in the same project. For example, typing
`"cue":"guide.h|ello"` filters on `guide.h` and replaces the entire string content,
leaving the quotes intact. JSON escapes are preserved correctly. Suggestions also
work in an unfinished cue string, and update from unsaved script edits.

Hover identifies the logical cue. **Find References** finds other authored
`@voice` occurrences with the same decoded ID, including different source locales.
All occurrences are uses, so Include Declaration does not change the result.
Malformed scripts are excluded from reference results and suggestion candidates;
separately indexed project roots, comments, prose, event payloads and arbitrary
host strings are outside this reference set. Scope follows the analysis workspace:
open a nested project's file or add its root to the workspace to load its config;
unopened nested configs are not automatically discovered by this query.

These are source references, not an audio catalog. Tua does not infer audio file
paths, verify recordings, provide Go to Definition for a cue, or rename external
asset mappings. The host still resolves locale-specific audio and availability.

## Complete character expressions

After an imported character's utterance, type `@expression ` to see its declared
expressions. Hover shows the expression record, and Go to Definition follows the
character declaration. Forwarded imports and unsaved character edits participate
in these queries.

Completion uses the prefix before the cursor but replaces the whole expression
name, including dots and hyphens. For example, choosing `face.half` at
`@expression face.ha|ppy-id` produces `@expression face.half`.

Expression suggestions follow the same attachment rules as the parser. They are
unavailable after choices, duplicate expressions, invalid preceding directives,
or an utterance without an imported character. Valid timing/events and comments
between the utterance and expression preserve ownership. Unfinished later text
does not prevent queries on the current expression.

## Complete context fields

Use completion after `context.` or `locals.` in `@bind`, `@branch`, `@visible`,
and `@enabled` directives. Fields come from the imported `Dialogue.context`
definition. Hover shows the full path's type, including optionality; Go to Definition
follows its context declaration through forwarded imports.

For example, with the cursor at `context.player.n|ame`, selecting `nickname`
replaces only `name`, producing `context.player.nickname`. Parent paths, trailing
members, operators, defaults and branch tags are preserved. Suggestions remain
available on an incomplete path such as `context.player.` and refresh when the
imported context changes in an unsaved buffer.

Quoted literals, placeholder-binding names, default strings, payloads and branch
targets are not context-field query sites. A target such as `[target:context.name]`
is a flow label even if the context also has a `name` field.

## Complete typed actions and events

After declaring an `@actions` or `@events` module, completion on an `@action` or
`@emit` name uses that module's typed contract. Completion inside a top-level JSON
payload key suggests the selected action/event's fields. Hover shows the full
field type, and Go to Definition follows the imported contract declaration.

When editing `reward` as `re|ward` (where `|` represents the cursor), suggestions
match `re` and replace the entire existing name. Payload keys behave the same way:
`"co|ins"` can become `"count"` without leaving a suffix or replacing the quotes.
Dotted/hyphenated names retain their complete replacement range. Unsaved imported
contracts refresh these queries, including contracts forwarded through another module.

Payload-value strings, nested JSON keys, comments and ordinary prose are not
contract-field completion sites. This is top-level key assistance, not a general
JSON schema editor. These queries inspect types without executing handlers.

## Rename a text identity

With `[i18n] enabled = true`, use Rename Symbol on an authored text ID, its catalog
entry, or a recognized literal localization call. Enter the new final key segment:
renaming `hello` to `welcome` changes `shop.hello` to `shop.welcome` while retaining
its namespace. Find References includes the authored ID and locale entries when
Include Declaration is enabled, plus recognized typed uses.

The edit updates line, choice or disabled-reason IDs, configured locale keys,
`I18n.t` literals and supported typed localization-wrapper calls in the same
project. An authored ID can be renamed before translations exist. Prose, ordinary
strings, flow labels, document IDs and action IDs are preserved, even when they
contain the same text.

This command deliberately changes the text's identity. Duplicate declarations,
existing destination keys and invalid new names are rejected. Finish malformed
dialogue edits first: a script that cannot be parsed could hide another declaration,
so localization rename is unavailable until it parses again. Unsaved script and
catalog edits participate in conflict checks. Computed keys and arbitrary host
strings are not rename targets or inferred references; maintain those mappings
explicitly. The command does not migrate saved checkpoints or external catalogs.

## Diagnose unreachable sections

The checker reports `TL5012` warnings for sections with no path from the first
section or a declared external entry. Declare labels started by game code in the
header, before the first section:

```text
@dialogue 1
@entry victory
[start]
// @id welcome
: Welcome. (end)
[victory]
// @id victory.message
: You did it! (end)
```

`@entry` is repeatable for different labels. Unknown labels, duplicate declarations
and declarations after the first section are errors. It does not change the default
entry, emitted gameplay data, or the host's ability to start another label. Label
completion, hover, definition, references and rename include these declarations.

Analysis follows waits, possible choices, fallback routes and both action outcomes.
It evaluates only context-free conditions using the shared condition rules. Dynamic
context, invalid/nonportable constant expressions and host outcomes remain unknown;
no imports or callbacks execute. A warning is evidence of disconnected authored
flow under these declared roots, not a prediction about arbitrary host calls.
It does not remove code or prevent a build. Dangling targets, duplicate identities
and malformed conditions remain errors. The CLI and editor use the same analysis.

## Format a script

```sh
tua fmt src/scene.tdialogue
tua fmt --check src/scene.tdialogue
```

Project-wide `tua fmt` includes dialogue scripts. Editor **Format Document** and
the read-only format preview use the same dedicated dialogue formatter.

The layout keeps declarations, labels and body lines at column zero, removes
outer blank lines, reduces repeated blank lines to one, and normalizes safe
structural whitespace. It preserves prose spacing (including trailing spaces),
quotes, escapes, JSON payloads, stable IDs, and the order of metadata and choices.
It does not wrap long text or reinterpret it as Lua. Source locations are rebuilt
from the resulting layout when the script is checked or built.

`line_ending` and `final_newline` follow the project's `[format]` configuration;
the defaults are LF and a final newline. Lua indentation, quote-style and width
options do not rewrite dialogue content. Invalid/incomplete scripts produce
syntax diagnostics and no formatting changes. Range and on-type dialogue
formatting remain disabled so partial edits cannot detach metadata.

For an unsaved buffer or tool integration, use
`tua fmt --stdin-filepath src/scene.tdialogue`; it returns formatted text without
writing the file. Add `--check` to test the layout without emitting text.

## Reveal text by grapheme cluster

`Dialogue.reveal(options?)` is an optional, renderer-independent presentation
controller. Give each displayed dialogue player its own controller. It consumes
snapshots after localization and placeholder expansion, reveals complete Unicode
extended grapheme clusters, and optionally returns speech-blip cues. It never
advances the dialogue or plays audio.

The runnable [reveal example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/reveal)
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
Checkpoint restore marks the line complete; honor that snapshot before consuming
updates. For custom restore flows, call `finish()` to avoid catch-up speech.

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
schema 8 or later as described below; translated
and substituted text is not reinterpreted as markup by this controller.

## Rich presentations and timing

Enable rich text with `@text rich` immediately after `@dialogue 1`, before
imports, labels or text IDs. Blank lines and ordinary comments may separate them.
The compiler emits schema 8 (schema 9 with waits, or schema 10 with audio metadata),
with both raw templates and plain fallback templates.

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

The [runnable example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/rich)
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
[event identity example](https://github.com/kyonru/tua-lang/tree/main/examples/dialogue/event-identity).

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
See the complete [typed events example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/events)
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

## Inspect a running dialogue

Enable the [Runtime Inspector](debugging.md#live-runtime-inspector) and restart the
game. The **Dialogue players and actions** group shows sampled state, node/action
IDs, waits and issue codes for `Dialogue.new` and typed context/action/event constructors.
It omits dialogue text, context values, payloads and error messages. Inspection
never runs handlers or advances the conversation. Sampling can miss brief states;
this is not event tracing or a translation coverage audit. Optional **Dialogue
sampled history** retains the latest 20 observed changes per player, with generation
and visit IDs; enable it in **Configure Runtime Inspection** and restart the game.
Public action handles also show ignored completion counters, including attempts
after cancellation. Handle IDs link attached handles to player samples without
recording result values or rejection messages.

**Translation provenance** shows current rich-text resolver metadata for lines,
choices and disabled reasons, capped at 64 fields. It distinguishes requested-locale
matches, locale fallback and authored-source fallback with the direct
`I18n.dialogueTemplate` resolver. Plain resolvers and custom resolvers without
provenance remain unknown. Text and arguments are omitted; inspection never calls
the resolver again. See [sampling limits](debugging.md#live-runtime-inspector).

## Preview the authored conversation

Open a `.tdialogue` file and run **Tua: Preview Dialogue**. Enable experimental
previews when prompted. **Read** opens a conversation reader: speakers keep the same
side and accent across sections, narration sits between their speech bubbles, and
choices appear beneath the line that owns them. Speaker sides are preview conventions;
your game's renderer still controls its own layout.

Choose a destination button to follow a branch. **Back** and the visited-section
trail return to earlier sections, including loops. **Restart from section** clears
the trail and starts from the section you are reading. The section list (a dropdown
in narrow panels) also reaches disconnected sections. History retains up to 100
visits and displays the latest eight; selection survives same-file refresh while
the section labels and order remain unchanged. Structural edits reset the trail.
Missing, ambiguous and out-of-limit destinations stay visible with an explanation.
**Source** and line-number links open authored source locations.

Switch to **Flow** for the conversation map alongside the selected section (stacked
in narrow panels). Selecting a node opens its text; the selected section highlights
its incoming and outgoing routes. Select a numbered connection to focus its source
link below the map. Use the zoom buttons, **Fit**, or drag empty space to pan.
With the map focused, `+`/`-` zoom and `0` resets zoom; Control/Command plus wheel also
zooms. View choice and zoom survive same-file refresh. The map draws up to 40
sections and 80 routes. Missing, ambiguous or undisplayed destinations are omitted
with a notice; consult the reader and Problems for those routes.

This is an authoring walkthrough, not runtime simulation. Following a branch never
updates variables, executes actions, waits in real time, or plays audio. All authored
routes remain inspectable regardless of mock results. Rich text uses a small editor presentation: emphasis, highlight, underline, strike,
thought, whisper, warning, success and radio styles. Other named styles and
speed/pronunciation marks retain hover annotations; pauses show timing labels.
This does not reproduce game shaders, animation, reveal timing or localization.
Plain scripts remain plain, and substituted argument values are never reparsed as
rich markup. Older compilers or exhausted rich-preview limits fall back to plain text.
Rich payloads are bounded to 64 KiB of text/mark data per field and 1 MiB in each
of the authored and mock presentations.

#### Portraits in the reader

Shared characters supply their authored literal name and portrait. An `@expression`
override selects the corresponding literal expression portrait. Table declarations
and local table initializers are inspected through the compiler's export index;
computed fields and game code are never executed. Later runtime mutations are not
simulated. Unsaved character edits refresh the reader.

Portrait strings that are local image paths resolve relative to the owning Tua
project's source directory, including when a parent examples folder is open.
For game-defined IDs, add an explicit editor mapping in workspace settings:

```json
{
  "tua.dialogue.preview.portraits": {
    "mara": "assets/portraits/mara.png",
    "mara.happy": "assets/portraits/mara.happy.png"
  }
}
```

This mapping affects only the editor. It supports PNG, JPEG and WebP, up to 2 MiB
per image, 8 MiB per preview and 32 distinct portrait IDs. Remote URLs, absolute
paths and symlinks outside the source directory are rejected. Unknown IDs, missing
files and decoding failures show a fallback instead of a broken image. Characters
without portraits keep a text-only heading. Use **Reload** after changing an image
if the editor has not yet received its filesystem event.

Expand **Test conditions**, enter explicit JSON in **Mock context**, then choose
**Apply mock context**. The panel distinguishes applied data from unapplied drafts:

```json
{
  "context": { "coins": 20, "unlocked": true },
  "locals": { "visits": 1 },
  "args": { "name": "Ada" }
}
```

Lines, choices and disabled-choice reasons show substituted plain text. When the
result differs, expand **Authored text** to inspect the original template. Arguments
come from optional scalar `args` and typed `@bind` declarations; bindings override
explicit args, defaults apply only to nil, and false is preserved. Replacement values
stay literal, even if they contain placeholders or rich-text marks. Missing arguments
keep the authored field visible with a source-linked error. A binding-resolution
error keeps all text authored while independent condition probes remain available.

Each branch, visibility and availability condition is tested independently. The
preview shows `true`, `false`, or a source-linked error. All routes remain visible:
this does not simulate traversal, action results, waits, or mutations between
sections. For example, an availability condition is tested even when its choice's
visibility condition is false. Missing fields and JSON `null` behave as nil.

Editing the JSON changes only the draft; press **Apply mock context** to apply it.
Source refreshes reuse the last applied JSON. Workspace file changes also refresh
the preview; deletion clears a clean buffer's preview until the file is recreated.
Unsaved text stays authoritative. Closing a buffer refreshes from the reopened
source, while hidden previews defer work until revealed. **Use authored view** stops evaluation
and keeps the draft available. Closing the preview or switching to another dialogue
clears both draft and applied input; they are not saved to project files or settings.

Input accepts `context` and `locals` record roots and optional scalar `args`, with
no arrays. Limits are
64 KiB of JSON, 4,096 values, 32 nesting levels and finite numbers within ±2⁵³.
At most 4,096 conditions are tested per request, with a notice if truncated.
Text output is capped at 4,096 fields and 1 MiB combined, with 64 KiB per field
and 64 KiB of retained argument text. Remaining fields stay authored with a notice.
Numbers use deterministic decimal formatting; Lua's `tostring` may spell them
differently. Supply a preformatted string when exact display spelling matters.
Imported context schemas are not validated against this input. Portraits, audio,
localization and rich styles are not resolved; rich source uses its parsed plain
template. Game handlers never run. See the
[editor reference](lsp.md) for refresh behavior and display limits.

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
[expression example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/expressions)
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

The runnable [localized dialogue example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/localization)
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
plain-dialogue checkpoints additionally require a host compatibility key; see
[checkpoints](#checkpoint-a-plain-conversation).

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
[Run the complete example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/context).

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
and hover show fields; definition navigation from a binding path leads to the context
factory declaration. Inside `%{name}`, completion suggests declared scalar bindings,
hover shows the imported field type and bound path, and Go to Definition jumps to
`@bind name`. This works in plain/rich lines, choices and disabled-choice reasons,
even while the placeholder is unfinished. It replaces only the name, preserving
braces and surrounding prose. Unsaved binding and imported-context edits refresh
the results. Duplicate or malformed headers suppress binding suggestions; escaped
`%%` and rich-mark attributes are not placeholder references.

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
The [conditional shop example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/conditions)
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

See the [dialogue example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/basics)
for the structured host and frontend development harness. The format is Tua-owned;
compatibility with Love-Dialogue `.ld` files is not implied.

## Run a registered host action

Use an action when host code must return an outcome before dialogue continues.
Timed events remain notifications; `update()` never invokes an action handler.
The runnable [dialogue-actions example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/actions)
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
requires separate calls. A handler returns a boolean or an action handle (below);
coroutine-based handlers are not supported. Reentrant dispatch returns
`false`. Cancellation, restart, or disposal inside the handler invalidates its
result, including any error returned afterward.

A missing handler, thrown error, or invalid result faults the player with
`action_handler_missing`, `action_handler_error`, or `action_result_invalid`.
An error resolving the destination faults with `action_transition_error`.
`snapshot().issue` carries source/action identity. These failures do not retry:
inspect the issue and explicitly cancel, dispose, or restart after reconciling any
side effects. For synchronous handlers, `runAction()` returns whether dispatch
successfully reached its outcome, not the handler's boolean result.

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


### Wait for asynchronous host work

Return a fresh `Dialogue.actionHandle()` from a handler when the host will produce
the result later. Existing scripts and action registries need no syntax changes.

```tua
local pending = Dialogue.actionHandle()
local player = actions.new(scene, {
  handlers = {
    purchase = function(payload: { item: string })
      -- Start host work here; retain pending in its completion callback.
      print(payload.item)
      return pending
    end,
  },
})
player:start()
player:runAction() -- Captures the handle; still awaiting_action.
pending:resolve(true) -- A host callback supplies the outcome.
player:runAction() -- Consumes it and enters the success target.
```

Use a **new handle for every dispatch** in a real handler. A handle can belong to
only one action visit, even after settlement. Returning the same handle from
another player or visit faults with `action_result_invalid`.

- `handle:resolve(true/false)` selects the authored success/failure target.
- `handle:reject(message)` records a host failure; consuming it faults with
  `action_rejected`. Messages must contain 1–4096 bytes.
- `handle:cancel()` invalidates the handle; consuming it faults with
  `action_cancelled`, rather than selecting the failure target.
- Settlement methods return `true` only for the first settlement. Later calls
  return `false`; malformed arguments still raise validation errors.
- `handle:snapshot()` returns an owned record with `status` and an optional
  `cancellationError`.

The first `runAction()` returns `true` when it accepts a fresh handle.
While that handle is pending it returns `false`, without invoking the handler
again. Once settled, the next call consumes it and returns whether the transition
succeeded. Even a handle settled inside the handler waits for this next call.
`update()`, snapshots and completion callbacks never consume it or start the next
action. This bounds execution without threads, a clock, or recursive callbacks.

While waiting, `snapshot().action.status` is `ready` before dispatch, then
`pending`, `resolved`, `rejected`, or `cancelled`. The same record contains
the stable action/source identity. Status inspection has no side effects.

### Cancel external work safely

Pass an optional cleanup callback when creating the handle:

```tua
local pending = Dialogue.actionHandle({
  cancel = function()
    -- Ask the host operation to stop; release its resources here.
    print("cancel host work")
  end,
})
```

Cancellation invalidates the handle **before** cleanup runs. Restart, player
cancellation and disposal detach and cancel the current pending handle; a handle
returned by a handler after reentrant replacement is cancelled too. Late callbacks
cannot advance the old player. Already-resolved/rejected handles do not run cleanup:
their host operation has finished. Completion clears the stored cleanup callback.

Cleanup runs at most once. Its errors are contained and available in the handle's
`cancellationError` (bounded to 4096 bytes); they cannot prevent player disposal.
A cancellation callback may reenter player lifecycle methods, so the caller should
inspect the resulting snapshot rather than assume its state stayed unchanged.

Cancelling dialogue does not guarantee external work stopped or roll back its
effects. With no cleanup callback, the handle is still invalidated, but the host
must manage any ongoing work. Timeouts belong to the host in this slice: call
`reject("timed out")` when its policy expires. The optional host skip policy below protects non-cancellable work. Checkpointing pending work remains future work.

## Wait and suspend dialogue

Use an authored wait when flow should pause before another label:

```text
@dialogue 1
[delay]
@wait 0.5 [target:greeting]
[greeting]
// @id greeting
: Welcome back. (end)
```

A wait occupies its own label, with one explicit target and a finite duration from
0 to 86,400 seconds. It cannot share the label with utterances, branches or actions.
Scripts containing waits emit schema 9, or schema 10 when they also declare audio metadata. Earlier scripts keep their previous schema
selection; older replacement runtimes must add schema-9 support to run wait scripts.
Typed documents can use `{ kind = "wait", duration = 0.5, next = "greeting" }`.

After entering a wait, `snapshot().state` is `"waiting"` and `snapshot().node` is
absent. `snapshot().wait` contains `duration`, `elapsed`, `remaining`, and optional
source coordinates. The snapshot's node ID and visit identify that wait.

Call `player:update(dt, revealed)` with **dialogue-time seconds** to progress it.
The reveal flag is irrelevant during a wait. Each update completes at most one wait
and discards excess time; it does not apply that time to the next wait, line events,
or host action. A zero-duration wait still needs an update, so a zero-duration cycle
cannot run unboundedly. Existing automatic branches retain their transition budget.
Errors thrown while entering the destination fault once with `wait_transition_error`;
bounded branch faults retain their existing diagnostic. Updates do not retry. `advance()` and `choose()` cannot skip a wait.

### Pause without cancelling

```tua
player:suspend()
-- Menu frames may continue calling update; dialogue clocks stay frozen.
player:update(10, false)
player:resume()
```

Both methods return whether they changed suspension. Suspension applies to active
lines, choices, actions and waits. It preserves state, visit, generation, presentation,
elapsed time and queued action handles; `snapshot().suspended` reports the flag.

While suspended, `update()` returns no events and advances no clocks.
`advance()`, `choose()`, and `runAction()` return false. Resuming does not consume an
action or account for elapsed wall time. If context refresh hid every choice while suspended, resume reconciles that deferred fallback
using the existing transition budget. If its presentation resolver throws, resume
throws and leaves the player suspended at the choice so the host can retry explicitly.
If the game stopped calling update entirely, its first resumed update should use
the current frame's dialogue time, not the time spent in a menu.

External host operations continue unless the host pauses them itself. They may
settle their handles during suspension; the next explicit `runAction()` after
resume consumes the result. If a synchronous handler suspends its player before
returning a boolean, that result is queued too, without invoking the handler again.
Cancellation/disposal still invalidate handles. Restart creates a fresh unsuspended
run; cancel and dispose also clear suspension.

When using `Dialogue.reveal`, pass the suspended snapshot to `present()`.
Its subsequent updates freeze fractional reveal progress, pauses and speech timing;
suspension changes invalidate old speech-cue sequence IDs. Pass a fresh snapshot on
resume. Older snapshots without `suspended` are treated as unsuspended.
`finish()` can still reveal text silently, but cannot navigate the suspended player.
The host owns pausing already-playing audio and animation.

Explicit locale/context refreshes remain available while suspended and retain their
existing silent reveal-completion policy. An empty-choice fallback is deferred until
resume; refresh itself cannot navigate the suspended choice. Suspension is not a content lock, an audio
pause command, a save checkpoint, or cancellation of external work.


## Describe voices and character chatter

Audio descriptions are optional schema-10 data. Tua validates and owns them; the host
loads assets, selects samples, plays/stops sources, and decides how to handle missing
audio. Subtitles remain available with no audio system or LÖVE dependency.

Declare a shared profile in the character module:

```tua
return {
  Guide = {
    name = "Guide",
    speech = Dialogue.speechProfile({
      sampleSet = "guide.chatter",
      pitchMin = 0.9,
      pitchMax = 1.1,
      variation = 0.2,
      gain = 0.6,
      minCueInterval = 0.05,
      cadence = "character",
    }),
  },
}
```

The constructor provides typed fields and validates numeric bounds at runtime.
Pitch limits must be ordered within 0.25–4; variation and gain are 0–1; the minimum
cue interval is 0–60 seconds. `sampleSet` is a nonempty host identifier, at most
256 UTF-8 bytes. Cadence is `"character"` or `"word"`.

Choose the profile explicitly on a line:

```text
@dialogue 1
@characters Cast "characters"
[start]
// @id greeting
Cast.Guide: Welcome.
@speech character
@voice {"cue":"guide.greeting","sourceLocale":"en","fallback":"silent"}
// @id aside
Cast.Guide: A quiet aside. (end)
@speech off
```

`@speech character` requires an imported character with a valid, required `speech`
profile. Missing or incorrectly typed profiles fail checking; missing dynamic
profiles fail module loading. Use `@speech off` to mark explicit silence, or provide
a complete JSON profile to override the shared one:

```text
@speech {"sampleSet":"guide.soft","pitchMin":0.8,"pitchMax":1.0,"variation":0.1,"gain":0.3,"minCueInterval":0.2,"cadence":"word"}
```

Each directive follows its utterance before choices and occurs at most once per line.
Omission means no speech preference; metadata never carries over from another line.
Explicit selection preserves earlier scripts' schema selection. Scripts with either
audio directive emit schema 10, which also supports prior waits, rich text, actions
and expressions. Older replacement runtimes need schema-10 support for these scripts.

Typed line records accept `speech = profile`, `speech = "off"`, and
`voice = { cue = "guide.greeting", sourceLocale = "en", fallback = "silent" }`.
Snapshots expose owned `node.speech` and `node.voice` records. Mutating a source
profile or returned snapshot cannot change a running player's descriptions.

### Voice locale and fallback policy

A voice cue is a logical asset identifier (1–256 bytes), not a path that Tua opens.
`sourceLocale` names its source recording locale (1–64 bytes). Identifiers may not
contain ASCII control characters; locale tags are opaque identifiers, not normalized.

The voice snapshot adds `locale`: the player's requested locale, or `sourceLocale`
when no locale was selected. This remains independent of subtitle catalog fallback.
The host first attempts the cue in that requested locale:

- `fallback = "silent"`: if unavailable, show subtitles without recorded voice.
- `fallback = "source"`: if unavailable, try the source-locale recording; otherwise
  remain silent.

The host determines availability and applies the policy; Tua does not claim a cue
exists or start playback. Locale refresh updates the descriptor without emitting an
audio command. Use player generation/visit/presentation identity to avoid replaying
a recording unintentionally.

`mixSpeech` is optional and defaults to false. Adapters should suppress chatter
**while recorded voice is actually playing**, unless mixing is explicitly enabled.
A missing recording does not by itself suppress available chatter. `@speech off`
always requests no chatter; it does not disable a separately declared recorded cue.
Rich `[pronounce="hint"]…[/pronounce]` runs provide existing pronunciation metadata;
hints are not synthesized or interpreted as commands by the core.

### Adapter responsibilities

Profiles are descriptors in this slice: they do not automatically configure
`Dialogue.reveal` or produce sound. Its existing cues remain grapheme-based.
Adapters must map cadence (including language-appropriate word segmentation), apply
profile intervals/gain/pitch variation, and use private deterministic RNG rather
than gameplay RNG. They also own volume/mute, bounded polyphony, missing-asset
fallback, audio suspension, skip policy and lifecycle cleanup. The optional
[dialogue audio example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/audio)
implements these responsibilities with typed host code and ordinary LÖVE Sources.
It includes two character voices, fixed source pools, a private pitch RNG, recording
locale fallback, mute, pause/resume, silent finish and lifecycle cleanup. Replace
its original synthetic recording fixture with your licensed voice acting.

Run `tua build` then `love build` inside `examples/dialogue/audio`. Space finishes
text while keeping the recording, then advances and stops it. P pauses/resumes,
M mutes chatter, V mutes recordings, and R restarts. This is a copyable host policy,
not a new required audio engine. Its ASCII word-boundary callback is replaceable
for other languages; pronunciation metadata is not synthesized speech. For pending actions, use the explicit [host skip policy](#choose-an-advance-and-skip-policy) instead of bypassing their state.

The [character presentation example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/expressions)
includes shared profiles, voice locale metadata, overrides, silence and plain-line reset.


## Choose an advance and skip policy

Keep finishing text, advancing a line, and cancelling an entire conversation as
separate commands. The core does not assume that unfinished external work can be
stopped, rolled back, or treated as successful.

The copyable typed host policy in
[`examples/dialogue/actions`](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/actions)
uses existing player and handle APIs. It introduces no new runtime method or
`.tdialogue` syntax. Its behavior is:

| Current state | Finish / advance | Skip conversation |
| --- | --- | --- |
| Unfinished line | Finish reveal silently; keep the recording playing. | Cancel the player and stop host audio. |
| Finished line | Advance once and stop the old recording. | Cancel the player and stop host audio. |
| Choice | Require an explicit choice; do not choose automatically. | Cancel without choosing. |
| Authored wait | `advance()` refuses; dialogue time completes the wait. | Cancel the whole conversation, without entering the next node. |
| Action ready to dispatch | Dispatch only through `runAction()`. | Refuse: no cancellation capability has been registered. |
| Pending cancellable action | Do not advance past the action. | Cancel the player; its handle invokes host cleanup once. |
| Pending non-cancellable or untracked action | Wait for explicit settlement. | Refuse; retain the current visit. |
| Settled action | Consume its result through `runAction()` first. | Refuse to discard the result. |
| Suspended player | Resume before navigation. | Refuse this user-facing skip command. |
| Completed/cancelled/disposed | No navigation. | Return inactive. |

Create one policy per player. Before starting the player, bind it once with
`policy.bind(player)`. In the asynchronous handler, return
`policy.begin(request, cleanup)` for work that can genuinely stop, or
`policy.begin(request)` for non-cancellable work. The supplied `request` is the
handler's typed second argument. `policy.skipConversation()` returns an acceptance
boolean and a reason such as `non_cancellable_action`, `consume_action_result`,
`unowned_action`, or `suspended`. The runnable `skip-demo.tua` demonstrates both
kinds of work; emitted lifecycle regressions cover restart and stale capabilities.

Providing cleanup is an explicit **host capability**, not inferred from an action's
`visual`/`consequential` effect classification. Only opt in when stopping is valid.
Stopping does not undo purchases, saves, network writes or other effects already
committed. The policy never resolves a handle artificially, jumps to its success
or failure node, or dispatches an action to discover whether it is cancellable.

Accepted cancellation detaches and invalidates the pending handle before cleanup.
Late completion is ignored. Cleanup errors remain on the handle's
`cancellationError`; they do not turn cancellation into success. A reentrant host
restart wins: the command accepted cancellation of the old visit, but does not
then cancel the replacement session. Check the latest snapshot after callbacks.

This policy is optional host code, not an enforcement layer around `player:cancel()`.
Direct lifecycle cancellation, restart and disposal still invalidate a handle even
when the external operation cannot stop. Keep ownership of that operation in the
host and suppress stale callbacks; invalidating dialogue does not roll back work.
After accepted navigation/cancellation, synchronize the audio adapter immediately
with `audio.update(player:snapshot(), 0)`. Dispose audio and player together.

## Checkpoint a plain conversation

For plain dialogues, a checkpoint can preserve the current line, choice or wait
without depending on disk or LÖVE. Give the script a stable `@document` ID and
choose a host compatibility key:

```tua
local scene = require("scene")
local options = { checkpointVersion = "harbor-v1", locale = "en" }
local player = Dialogue.new(scene, options)
player:start()
local saved = player:checkpoint()

-- Store `saved` with the host's JSON persistence when appropriate.
local restored = Dialogue.new(scene, options)
restored:restore(saved)
```

The [checkpoint example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/checkpoint)
round-trips line, choice and suspended-wait positions. `checkpoint()` returns an
owned plain record. `restore()` accepts only a fresh idle player; invalid data,
incompatible content or a throwing resolver leaves that player idle. No automatic
branch traversal, action dispatch or event delivery occurs during restore.

Checkpoint version 1 contains the document ID, host content version, exact graph
signature, node/state, elapsed and revealed-event timers, fired-event indices,
locale and suspension. It excludes closures, host arguments, gameplay context,
GPU/audio objects, handles, wall-clock time and partial typewriter progress. Rebind
arguments and resolvers through the fresh player's options. Saved locale wins;
call `setLocale()` afterward for a different language.

Restored lines appear fully revealed and emit no catch-up speech cues. Existing
revealed-event clocks are preserved; if reveal had not finished before saving,
its clock starts at zero on restore. Shown-event clocks retain their saved elapsed
time. Fired flags prevent ordinary duplicate delivery within the restored visit;
newly due notifications still require `update()`. Suspended restores stay suspended.
The snapshot's `restored` flag remains true for this visit (including locale refresh)
and resets on a new node. The audio example suppresses automatic recordings for
such snapshots; a host may offer explicit replay. Runtime identity counters belong
to the fresh player and are not durable idempotency keys.

### Compatibility and limits

The host `checkpointVersion` key is required, nonempty and at most 256 bytes, as is
the document ID. Checkpoint node IDs are limited to 1,024 UTF-8 bytes and locales to
256. Timers must be finite, nonnegative and at most 10¹² seconds. Wait elapsed time
cannot exceed its authored duration. The validator rejects unknown fields,
metatables, malformed event arrays, unsupported versions and incompatible positions.

A collision-free canonical graph signature includes schema, entry, node kinds,
node/text/choice IDs, targets, wait durations and event identities/payloads/timing.
Changing those facts rejects restore rather than jumping to a similarly numbered
node. Plain prose, translated presentation, portraits and voice metadata may change
without invalidating the graph. Bump the host key when a semantic change requires
migration despite an unchanged structure. Migration is an explicit host decision;
there is no automatic remapping in this version.

Graph work is capped at 8,192 node/event/option records, canonical traversal at
16,384 entries and depth 32, string content at 256 KiB, and the final signature at
512 KiB. Delivered-event arrays contain at most 4,096 entries. The graph signature
is data, not a cryptographic integrity check. Apply your storage schema/size limits
before decoding untrusted save files.

Version 1 rejects context documents, branches and conditional choices. Typed
context players use version 2, described below. **Both versions reject all action-
and typed-event-registry documents**, not merely currently pending work. Idle,
completed, cancelled, disposed, faulted and action states cannot be exported.
Version 3 offers explicitly opted-in settled registry boundaries, described below.
The host persistence and durable action recipes are separate. Checkpoints do not
promise exactly-once external effects: crashing between
an effect and its durable acknowledgement still requires host-side atomic
persistence or idempotency.

### Restore typed dialogue locals

A context player without registries exports version-2
checkpoints. The `locals` field retains the definition's concrete type for checking,
completion and hover. It contains an owned copy validated by the original
`Save.shape`; whole gameplay facts remain outside the checkpoint.

```tua
local context = require("context")
local scene = require("context_scene")
local options = { checkpointVersion = "harbor-context-v1" }
local original = context.new(scene, { coins = 10 }, options)
original:start()
original:setLocals({ visits = 3 })
local saved = original:checkpoint()

-- Supply current gameplay facts; restore only the dialogue's local variables.
local resumed = context.new(scene, { coins = 20 }, options)
resumed:restore(saved)
assert(resumed:getLocals().visits == 3)
assert(resumed:getContext().coins == 20)
```

The [typed checkpoint example](https://github.com/Kyonru/tua-lang/blob/main/examples/dialogue/checkpoint/src/context_demo.tua)
includes the shared context and authored script. Shared context definitions retain
checkpoint-local inference through imports and forwarding modules. Constructed
players can also be exported directly, returned by a factory, or exposed as named
fields and re-exported through Tua or Lua facade modules. Their concrete facts,
locals and method signatures remain available to checking and editor queries.

Version 2 supports branches, conditional choices and bindings in the document;
only line/choice/wait positions are checkpoint boundaries. Restore rebuilds the
current presentation with saved locals and freshly supplied facts. It does not
traverse branches, run actions, dispatch notifications, or append history. Future
transitions evaluate conditions against the rebound facts and restored locals.

At a saved choice, at least one reply must remain visible and enabled under the
rebound state. Otherwise restore fails and leaves the fresh player idle—even if
the choice has a fallback or was suspended. The host may update the fresh player's
facts and retry, or explicitly start a new conversation. Restore never silently
changes the saved position. Invalid locals, missing binding values and throwing
resolvers also leave facts, locals and playback state unchanged.

Version-2 graph identity additionally covers compiled branch/choice conditions,
branch targets, binding definitions/fallbacks and local defaults. Plain version-1
graphs are unchanged, and the two versions are not interchangeable. Locals must
pass the current Save shape; optional absent fields, false, finite signed numbers,
arrays and nested records retain their normal Save semantics. Before validation,
local data is bounded to 16,384 entries, depth 32 and 256 KiB of strings, separately
from the graph budget. Cycles, metatables, functions, sparse arrays, invalid UTF-8
and incompatible values are rejected.

The graph includes defaults, **not a full schema fingerprint**. Bump the required
host `checkpointVersion` whenever fact/local schema semantics or host behavior
changes incompatibly, even if defaults stay the same. There is no automatic schema
migration. Facts, closures, host handlers and media are rebound, never serialized.


## Checkpoint conversations with actions or typed events

Opt in with `checkpointEffects = "settled"` when constructing a registry-backed
player. This selects **version 3** for that player; omitting the option (or choosing
`"reject"`) preserves the default registry exclusion. Plain/context-only documents
continue producing v1/v2 data even when the option is set.

```tua
local actions = require("actions")
local scene = require("scene")
local function fresh()
  return actions.new(scene, {
    checkpointVersion = "merchant-v1",
    checkpointEffects = "settled",
    handlers = {
      purchase = function(payload: { item: string }): boolean
        return payload.item == "shell" -- Example decision; host owns real effects.
      end,
    },
  })
end
local player = fresh()
player:start()
player:runAction() -- Explicit host dispatch; reaches the authored outcome line.
local saved = player:checkpoint()
local resumed = fresh()
resumed:restore(saved) -- Presents that line; does not dispatch the purchase again.
player:dispose()
resumed:dispose()
```

The runnable [settled checkpoint example](https://github.com/Kyonru/tua-lang/blob/main/examples/dialogue/actions/src/checkpoint-demo.tua)
counts handler calls and verifies that restore leaves the count unchanged.
The same option works on event-registry and context factories, including contexts
that combine actions and events.

Only line, choice and wait boundaries can be exported. Ready actions, pending
handles, unconsumed handle resolutions and synchronous handler execution are
rejected. V3 requires `actionState = "settled"`, meaning **no action is attached or
executing in this player**. It is not a receipt or proof that external work has
stopped or was durably committed. No
handle, handler, result callback, gameplay fact or transaction ID is serialized.

V3 fingerprints action IDs, names, validated payloads, effect classifications,
success/failure targets and registry presence in addition to the existing graph.
It retains typed-event fired flags and logical timers. Restore emits no notifications;
unfired due events require a later `update()`. New visits may emit their events again.
Typed locals are included only for context players. Rebind current facts, handlers,
resolvers and presentation services when creating the fresh player.

Changing an action or event payload, identity or timing rejects the old checkpoint.
Payload schema and host-handler semantics still require an explicit
`checkpointVersion` change when their meaning changes: graph identity does not
fingerprint arbitrary validators or closures. No automatic v1/v2/v3 migration occurs.

A checkpoint cannot make an effect and its acknowledgement atomic. If the host
commits a purchase then crashes before saving the new dialogue position, restoring
an older position can reach that purchase again. Use the
[durable-action recipe](#recover-consequential-host-actions) or an external
idempotency authority for those effects. The same limitation applies to side effects
performed in response to emitted notifications. Cancel/restart may detach a handle
without stopping its external work; reconcile abandoned consequential operations
through the host policy as well.

When persisting v3 with `Save.contract`, include `actionState` in the host schema
(for example `Save.enum({ "settled" }, "settled")`), and include the concrete local
schema only for a context player. Keep the host envelope version independent from
the checkpoint version. The v2 Save example is deliberately scoped to its own scene.

## Persist checkpoints with Save

The [checkpoint example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/checkpoint)
includes a typed `Save.contract` recipe for a context-backed conversation:

```tua
local sessions = require("saved_session") -- Example host module, not a built-in.
local session = sessions.new(20) -- Rebind the current gameplay facts.
if not session.load() then session.player:start() end

-- Save only when the host chooses an eligible line, choice or wait boundary.
session.save()
session.player:dispose()
```

`load()` restores into the fresh idle player and returns false only when no
checkpoint exists. A corrupt, incompatible or currently unavailable checkpoint
raises an error; present that error instead of silently starting again. Save checks
the envelope schema, then dialogue restore checks graph identity, timers and the
current context. The saved locale and locals are retained; current gameplay facts
are supplied by the host. No action dispatch or transcript replay occurs.

The example's envelope version and concrete local schema belong to the host.
Adapt the `visits` schema alongside your context, and use explicit migrations for
incompatible stored data. Core checkpoint versions and the host compatibility key
have separate roles. The recipe reopens Save before writes to observe uncertain
prior writes; it assumes one writer and does not promise power-loss durability.

Restarting dialogue, restoring a checkpoint and browsing history are separate
operations. Restart does not erase the stored checkpoint, and history is not saved
by this recipe. Storage timing remains a host decision.

## Recover consequential host actions

The [durable dialogue recipe](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/durable)
shows how to recover one local purchase using existing `Dialogue.actions` and
`Save.contract` APIs. It persists an operation ID before dispatch, then saves the
coins/inventory change and its boolean receipt together. Recovery loads that record
and creates a fresh player; the host explicitly calls `runAction()` to consume the
receipt or commit prepared work. A receipt returns the original result, including
`false`, without applying the purchase again.

```tua
local journal = require("journal").new()
local flow = require("session").new()

-- The profile must already have been explicitly initialized by the host.
local recovered = flow.recover()
if recovered then
  assert(recovered.player:runAction())
  local state = journal.load()
  print(state.coins, state.shells)
  -- Acknowledge after the host has consumed the action's outcome.
  recovered.acknowledge()
end
```

These `journal` and `session` modules belong to the example, not new built-ins.
The host calls `flow.begin()` for a deliberate new purchase. Each occurrence gets a
new persistent key; authored action IDs and live generation/visit counters alone
are not durable operation identities. Acknowledgement removes the one outstanding
receipt but retains the sequence counter, preventing stale keys from being reused.
Cancelling playback does not cancel the durable intent; prepared work can be
explicitly abandoned with `journal.abandon(key)`.

A write error does not establish that nothing was saved. The recipe reopens a fresh
Save contract on each operation, reconciles any committed receipt, and stops on
all load issues rather than accepting older data or resetting automatically.
Compatibility and payload checks run before dispatch. The README and runnable tests
cover failures before/after writes, failed read-back, damaged slots, restart before
acknowledgement, stale keys and declined results.

The guarantee is scoped to **single-writer local state and its receipt in one Save
record**. Save does not guarantee power-loss durability, concurrent writers or cloud
conflict resolution. External services must honor the same idempotency key and
provide an authoritative outcome query; a local receipt cannot make an external
reward atomic. Core checkpoints reject action/typed-event registries by default; v3 requires explicit
settled-boundary opt-in and still rejects ready/pending actions. This example's explicit host intent is a separate
recovery protocol, not general serialization of an action-enabled conversation.

## Keep a bounded conversation history

Enable history when constructing a player. It works with plain conversations and
with typed-context, event and action players; it does not save their gameplay state.

```tua
local player = Dialogue.new(scene, {
  locale = "en",
  history = { capacity = 100, maxBytes = 65536 },
})
player:start()

local history = player:history()
for _, entry in ipairs(history.entries) do
  print(entry.speaker or "", entry.text)
end
-- Remove one retained entry, for example before sharing a transcript.
if #history.entries > 0 then
  player:redactHistory(history.entries[1].id)
end
player:clearHistory()
```

History is disabled by default. `capacity` is required when enabled and accepts
1–1,024 entries. `maxBytes` defaults to 65,536 and accepts 1–1,048,576; it counts
**all retained string bytes**, including IDs and locale, not Lua table overhead.
Oldest entries are evicted until both limits fit. A single entry larger than the
byte budget is skipped intact; text and identifiers are never truncated.
`history.dropped` counts evicted and oversized entries. Neither case interrupts
playback. The capacity bound also limits per-entry table overhead.

A line is recorded when it is presented, with its full resolved plain text—even
while a renderer is still revealing characters. A choice is recorded only after
its transition is accepted and its destination presentation resolves successfully.
It precedes the destination line in history. Invalid, disabled and suspended
choices, throwing resolvers and transition-budget faults add no selected reply.
Waits, actions, branches and unselected choices do not produce records.

Each entry has a session-local numeric `id`, `kind` (`"line"` or `"choice"`),
`nodeId` and `text`. Optional fields are `documentId`, `textId`, `choiceId`,
`speaker`, `speakerKey` and `locale`. `speakerKey`, when authored, supplies the
stable localized speaker identity; `speaker` is the displayed name. Selected
replies use their option ID as `textId`. No context, locals, action/event payloads,
rich-text trees, portrait or audio objects are retained. The
[feature example](https://github.com/Kyonru/tua-lang/blob/main/examples/features/src/dialogue.tua)
shows capture, browsing and redaction.

Records preserve the original text and locale. Locale/context refreshes do not
rewrite or duplicate earlier lines. Browsing returns owned records and never
advances the conversation or replays effects. IDs are not reused after clearing;
`redactHistory(id)` removes an entry and returns whether it was still retained.
Redaction does not affect copies previously returned to the host. `clearHistory()`
releases all entries and resets byte/drop counts. Both operations are forbidden
inside text resolver callbacks, like other player mutations.

History remains available after cancellation, completion and `start()` restarts;
call `clearHistory()` at your chosen conversation boundary. `dispose()` releases
it. Disabled or disposed players return an empty history with zero limits/counts.

Checkpoints deliberately **exclude history**. Restore neither imports earlier
records nor appends the restored line again. New lines and replies after restore
are recorded normally. A host may separately persist a returned history snapshot
under its own Save schema, redaction and retention policy; there is no history
import API. Transcript browsing and resuming executable dialogue are separate
operations. Entry IDs are not durable action or reward identifiers.
