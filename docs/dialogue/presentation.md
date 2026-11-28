---
icon: lucide/type
---

# Text presentation and reveal

Reveal a line progressively and render rich presentation metadata with your own fonts, layout and input controls. See the [dialect reference](dialect.md#rich-text-tags) for every inline tag.

[Dialogue overview](index.md)

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
No proprietary sounds, audio engine, wall clock, or gameplay RNG are used. See [voice and chatter profiles](characters-and-audio.md#describe-voices-and-character-chatter) for optional audio integration.

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

Enable rich text using the [dialect reference](dialect.md#rich-text-tags). The same markup can be authored in the editor or supplied by a typed document.

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
