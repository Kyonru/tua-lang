---
icon: lucide/tags
---

# Dialogue dialect and semantics

The `.tdialogue` format is a declarative, line-oriented dialect for conversation
data. Use directives to declare facts and metadata, labels to organize flow,
and inline marks to describe text presentation. Dialogue text never executes Lua.

Start with the [first conversation](index.md#your-first-conversation) if you
have not written a script before. This page is the reference for the complete
version-one authoring syntax, including optional features. Each entry gives a
definition and an example. Short code blocks are script fragments: add the required
header, imports, text IDs and destination labels as appropriate. The script below
is a complete example.

## Script anatomy

```text
@dialogue 1
@text rich
@character Guide "Harbor guide"

[start]
// @id harbor.welcome
Guide: Welcome to the [em]harbor[/em].
// @id harbor.proceed
-> [style=highlight]Continue[/style] [target:done]

[done]
// @id harbor.farewell
: [pause=0.25]Safe travels. (end)
```

The header declares the format and opts into rich text. The first label, `start`,
is the default entry. After the host acknowledges the line, the player presents
one choice, whose stable ID is `harbor.proceed`. Selecting it visits `done`.
Acknowledging the final line completes the conversation. The player does not
fall through to the next label by source order.

## Header directives

Place declarations before the first label. Module names in quotes refer to Tua
or Lua modules in the configured source directory, not arbitrary executable text.

### `@dialogue` — Script version

**Definition:** Required first directive of every script. The authoring version
is `1`, independently of the emitted document schema version.

**Example:**

```text
@dialogue 1
```

### `@text rich` — Rich-text mode

**Definition:** Enables semantic marks and the formatting toolbar for the entire
script. Optional, but must follow `@dialogue 1` before imports or text IDs.

**Example:**

```text
@dialogue 1
@text rich
```

### `@document` — Document identity

**Definition:** A stable logical ID, independent of the source filename. Required
for explicit event and action IDs; also useful for checkpoints. At most one.

**Example:**

```text
@document harbor
```

### `@entry` — External entry point

**Definition:** Declares an additional section reached from host code for
reachability analysis. Repeatable; it does not change the default entry or
restrict `player:start(label)`.

**Example:**

```text
@entry victory
@entry defeat
```

### `@character` — Local speaker

**Definition:** Declares a speaker ID with a plain display name. Repeat for
several local speakers.

**Example:**

```text
@character Guide "Harbor guide"
```

Use `Guide:` before a spoken line. The displayed name is “Harbor guide.”

### `@characters` — Shared cast

**Definition:** Imports a shared character module under a script-local alias.
Repeat with distinct aliases to use several casts.

**Example:**

```text
@characters Cast "characters"
```

If that module exports `Guide`, use `Cast.Guide:` before a spoken line.

### `@context` — Typed conversation data

**Definition:** Imports a typed context definition. Required for field bindings
and conditional flow. At most one per script.

**Example:**

```text
@context "story.context"
```

### `@bind` — Named placeholder binding

**Definition:** Binds a simple placeholder name to a scalar field path rooted at
`context` or `locals`. An optional field path requires a compatible string,
finite number, or boolean `default`. Repeat with distinct binding names.

**Example:**

```text
@bind name context.player.name
@bind nickname context.nickname default "traveler"
@bind visits locals.visits
```

The prose `Welcome, %{nickname}.` uses the bound nickname or “traveler” when the
field is missing.

### `@events` — Typed event registry

**Definition:** Imports a registry of typed notification names and their payloads.
Required for `@emit`. At most one per script.

**Example:**

```text
@events "story.events"
```

### `@actions` — Typed action registry

**Definition:** Imports a registry of typed host actions and their payloads.
Required for `@action`. At most one per script.

**Example:**

```text
@actions "story.actions"
```

See [shared characters](characters-and-audio.md#share-characters-between-scripts),
[typed context](conditions.md#typed-context-and-conversation-locals), and
[events and actions](events-and-actions.md) for the matching Tua definitions.

## Sections, lines and choices

### `[label]` — Section { #label-section }

**Definition:** Declares a unique section label within the current file. The
first label is the default entry.

**Example:**

```text
[start]
```

### `//` — Comment

**Definition:** A whole-line comment ignored by playback. Inside an utterance,
`//` is ordinary prose rather than a comment.

**Example:**

```text
// The guide introduces the harbor here.
```

### `// @id` — Text identity

**Definition:** Assigns a stable, project-unique ID to the following utterance,
choice, or disabled reason. Keep IDs stable when editing the wording.

**Example:**

```text
// @id harbor.hello
Guide: Hello!
```

### `Speaker:` — Utterance

**Definition:** Spoken text associated with a declared local speaker or a shared
character export. The player resolves the speaker name and metadata.

**Example:**

```text
// @id harbor.guide
Guide: Hello!
// @id harbor.captain
Cast.Captain: Welcome aboard!
```

### `:` — Narration

**Definition:** An utterance without a speaker, useful for narration or scene text.

**Example:**

```text
// @id harbor.weather
: It is raining.
```

### `->` — Choice

**Definition:** A reply attached to the preceding utterance. Its
`[target:label]` identifies the next section, and its stable text ID is passed to
`player:choose(id)`.

**Example:**

```text
// @id harbor.question
: Are you ready?
// @id harbor.ready
-> Continue [target:next]
```

### `(end)` — Terminal utterance

**Definition:** Ends this path when the host acknowledges the line. It does not
complete the conversation automatically while the final text is visible.

**Example:**

```text
// @id harbor.goodbye
: Goodbye. (end)
```

### `@wait` — Timed wait

**Definition:** A wait-only section. The duration is finite and between 0 and
86,400 seconds. `player:update` advances the clock and follows the target;
ordinary advancement or choice input does not skip it.

**Example:**

```text
[delay]
@wait 0.5 [target:next]
```

### `@branch` — Conditional route

**Definition:** A branch-only section that automatically selects a destination
using a typed condition. Requires `@context` and both true and false targets.

**Example:**

```text
[gate]
@branch context.ready [target:yes] [else:no]
```

### `@action` — Host action

**Definition:** An action-only section that awaits an explicit call to
`player:runAction()` and a host outcome. Requires `@actions`, `@document`,
a stable action ID, two destination tags, and a matching JSON payload record.

**Example:**

```text
[buy]
@action purchase [id:shop.purchase] [success:done] [failure:declined] {"item":"shell"}
```

All targets are labels in the same script. Forward references are allowed.
Consecutive utterances advance in order. A prose section must end with a
choice group or terminal utterance; a wait, branch or action occupies its own
section and cannot contain utterances. See [player timing](player.md) and
[registered host actions](events-and-actions.md#run-a-registered-host-action).

## Choice conditions and reasons

A choice may have the following metadata when `@context` is declared. Conditions
read accepted facts/locals; they do not mutate data or call host functions.

### `@visible` — Choice visibility

**Definition:** A condition attached to a choice. When false, that choice is
omitted from the snapshot.

**Example:**

```text
// @id shop.secret
-> Secret offer [target:bought]
@visible context.unlocked
```

### `@enabled` — Choice availability

**Definition:** A condition attached to a choice. When false, the choice remains
visible but cannot be selected.

**Example:**

```text
// @id shop.buy
-> Buy [target:bought]
@enabled context.coins >= 20
```

### `@reason` — Disabled-choice explanation

**Definition:** Localizable text explaining why a choice is unavailable. Follows
the choice and has its own unique `// @id`. Shown only when the choice is disabled.

**Example:**

```text
// @id shop.buy
-> Buy [target:bought]
@enabled context.coins >= 20
// @id shop.buy.reason
@reason Requires 20 coins.
```

### `@fallback` — No available choices

**Definition:** Routes a conditional choice group to a label when no visible
and enabled reply remains. Required after a conditional group.

**Example:**

```text
// @id shop.buy
-> Buy [target:bought]
@enabled context.coins >= 20
@fallback closed
```

Conditions support field paths, scalar literals (`true`, `false`, `nil`, finite
numbers and double-quoted JSON strings), parentheses, `not`, `and`, `or`, and
comparisons (`==`, `~=`, `!=`, `<`, `<=`, `>`, `>=`). Arithmetic, indexing, calls
and assignment are not supported. See [conditional flow](conditions.md#conditions-branches-and-available-choices)
for precedence, optional values, type rules and a complete shop example.

## Utterance metadata

These directives attach to the preceding utterance, before choices or the next
utterance. They do not need text IDs. Metadata may follow a terminal utterance.

### `@expression` — Character expression

**Definition:** Selects an expression declared on the imported speaker, at most
once per utterance. The renderer chooses how to display it.

**Example:**

```text
// @id harbor.concern
Cast.Guide: Something is wrong.
@expression worried
```

### `@timing` — Hold and fade hints

**Definition:** Specifies finite, nonnegative hold and fade durations in seconds,
at most once per line. The host may honor the hints; they do not implement
automatic advancement or fading.

**Example:**

```text
// @id harbor.notice
: A ship approaches. (end)
@timing hold 3 fade 0.35
```

### `@event` — Named notification

**Definition:** Schedules a legacy notification with a finite, nonnegative delay.
`shown` starts timing when entering the line; `revealed` starts when the host
first reports full text reveal. Optional quoted string payloads default to empty.

**Example:**

```text
@event shown 0.2 portrait.pulse
@event revealed 0.25 sound.play "welcome"
@event shown 0 pulse [id:harbor.pulse] "soft"
```

The last form supplies an explicit event identity, requires `@document`, and
places `[id:…]` before the optional payload.

### `@emit` — Typed notification

**Definition:** Schedules a notification validated against an imported
`@events` registry. Requires `@document`, an explicit event ID and a matching JSON
record. Accepts the same `shown` and `revealed` anchors as `@event`.

**Example:**

```text
@emit shown 0 reward [id:harbor.reward] {"coins":10}
```

### `@speech` — Character chatter

**Definition:** Describes the chatter profile for a line. `character` selects the
imported speaker's required shared profile, `off` requests no chatter, and a
complete JSON profile overrides the shared one. The host owns the audio playback.

**Examples (alternative directives for one line):**

```text
@speech character
```

```text
@speech off
```

```text
@speech {"sampleSet":"guide.soft","pitchMin":0.8,"pitchMax":1.0,"variation":0.1,"gain":0.3,"minCueInterval":0.2,"cadence":"word"}
```

Explicit silence does not disable a separately authored recorded voice cue.

### `@voice` — Recorded voice cue

**Definition:** Describes a logical recorded voice ID, source locale and fallback
policy (`silent` or `source`). Optional `mixSpeech` controls whether chatter can
play alongside a recording. Does not load or play audio itself.

**Example:**

```text
@voice {"cue":"guide.hello","sourceLocale":"en","fallback":"silent"}
```

A line can contain up to 64 events. `shown` and `revealed` are separate clocks;
metadata is not carried over to the next line. `@speech` and `@voice` can each
occur once per line. JSON action/event payloads must be records, reject duplicate
keys and `null`, and are validated against their typed contracts. The host owns
actual effects, asset loading, sound playback and animation.

See [event timing and payloads](events-and-actions.md#schedule-custom-events),
[expressions](characters-and-audio.md#select-character-expressions), and
[voice and speech profile fields](characters-and-audio.md#describe-voices-and-character-chatter)
for exact payload shapes, ranges and host behavior.

## Structural tags

Bracketed flow tags are not rich-text marks and do not appear in the displayed
text. They belong in the position shown above, even in a rich script.

### `[label]` — Section declaration

**Definition:** A section name on its own line. See [section syntax](#label-section)
for how it participates in flow.

**Example:**

```text
[shop.done]
```

### `[target:label]` — Destination

**Definition:** Names the destination of a choice, wait, or true branch.

**Example:**

```text
-> Continue [target:next]
```

### `[else:label]` — False branch destination

**Definition:** Names the destination when a branch condition is false.

**Example:**

```text
@branch context.ready [target:yes] [else:no]
```

### `[success:label]` and `[failure:label]` — Action outcomes

**Definition:** Name the two destinations of a registered host action, chosen by
its explicit result (`true` or `false`).

**Example:**

```text
@action purchase [id:shop.purchase] [success:done] [failure:declined] {"item":"shell"}
```

### `[id:stable.id]` — Event or action identity

**Definition:** An optional ID on legacy `@event`, required on typed `@emit`
and `@action`. IDs are unique within the document and distinct from text
identities (`// @id`).

**Example:**

```text
@event shown 0 pulse [id:harbor.pulse] "soft"
```

## Whitespace and escaping

Indentation before a statement is ignored. One optional ASCII space following
`:` is syntax; additional leading and trailing utterance whitespace is preserved.
The choice prefix is `-> ` and its target suffix is ` [target:label]`. One space
before a terminal `(end)` is syntax; whitespace after that marker is ignored.
There are no inline comments: `//` inside utterance text is literal text.

### `\n`, `\t`, `\r` — Text spacing

**Definition:** Represent a newline, tab, or carriage return inside one prose
field, without starting a new script statement.

**Example:**

```text
: Welcome.\nYour room is ready.
```

### Escaped literal characters

**Definition:** Use a backslash to preserve a reserved character as text:
`\\`, `\"`, `\{`, `\}`, `\[`, `\]`, `\$`, `\(` and `\)`. Escaped quotes are
especially useful inside quoted speaker names.

**Example:**

```text
: Read the \[notice\] on the wall. The sign says \"Welcome\".
```

### `\(end)` — Literal terminal marker

**Definition:** Displays `(end)` literally, instead of terminating the utterance.

**Example:**

```text
: The card reads \(end)
```

### `%{name}` — Named placeholder

**Definition:** Receives a scalar argument or typed binding. The substituted
value remains literal prose, even if it contains markup characters.

**Example:**

```text
: Welcome, %{name}!
```

### `%%` — Literal percent

**Definition:** Represents a literal percent during template formatting. In a
rich-text template, `%%{name}` displays `%{name}` without substituting it.

**Example:**

```text
: Progress: 100%%. Literal token: %%{name}.
```

Unknown or unfinished escapes are errors. Use simple placeholder names (ASCII
letters, digits and underscores, not starting with a digit). See
[localization](localization.md) and [typed bindings](conditions.md#typed-context-and-conversation-locals).
Plain text is decoded without rich markup; placeholders are substituted by the
configured resolver or typed bindings. Rich text resolves its template even
without a catalog. Substituted values are literal text, never new markup or code.

## Rich-text tags

Enable rich text explicitly, immediately after the version header:

```text
@dialogue 1
@text rich
```

Blank lines and ordinary comments may separate these directives. Plain scripts
must opt in before using tags or the editor's formatting toolbar. In a rich
script, lines, choices and disabled reasons support semantic marks; character
display names stay plain. Timing marks are supported only in utterances.

### `[em]…[/em]` — Emphasis

**Definition:** Semantic emphasis on a line, choice, or reason. The editor
preview draws it in bold; the game's renderer decides how to present it. Beacon
Harbor uses heavier lettering and an accent color.

**Example:**

```text
: Please read the [em]important notice[/em].
```

### `[style=key]…[/style]` — Named style

**Definition:** A host-defined presentation key on a line, choice, or reason.
The key is a simple ASCII identifier of 1–64 bytes (letters, underscores, digits
after the first character). The same syntax supports all [named styles](#named-style-vocabulary).

**Example:**

```text
: [style=warning]The tide is rising.[/style]
```

### `[speed=rate]…[/speed]` — Scoped reveal speed

**Definition:** Sets reveal speed for the marked utterance text in grapheme
clusters per second, greater than 0 and at most 1,000. Closing the mark restores
the enclosing/default rate. Lines only.

**Example:**

```text
: Listen. [speed=8]Very carefully.[/speed] Are you ready?
```

The marked words reveal at eight clusters per second; the remaining text uses
the ordinary rate.

### `[pause=seconds]` — Reveal pause

**Definition:** Pauses reveal at the current boundary for 0–60 seconds. Lines
only; this is a standalone tag with no closing mark.

**Example:**

```text
: Wait… [pause=0.5]There it is.
```

### `[pronounce="hint"]…[/pronounce]` — Pronunciation hint

**Definition:** Attaches an opaque speech hint of 1–256 UTF-8 bytes to text in a
line, choice, or reason. Does not synthesize speech or play audio. Pronunciation
marks cannot nest within one another.

**Example:**

```text
: [pronounce="ahoy"]Ahoy[/pronounce], traveler!
```

### Combining and nesting tags

Marks nest strictly; close them in reverse order of opening. Unknown tags,
mismatched closing tags and empty paired regions fail. Use ordinary unsigned
decimals (`15` or `0.25`) for speed and pause values; exponents and signs are
not accepted. Marks must not split a final grapheme cluster, including across
placeholder substitutions.

```text
@dialogue 1
@text rich
[start]
// @id harbor.demo
: [pause=0.25][em][style=highlight]Look here[/style][/em]. [speed=8]Read slowly[/speed]. [pronounce="ahoy"]Ahoy[/pronounce]! (end)
```

This displays “Look here. Read slowly. Ahoy!” without brackets or parameters.
The reveal controller waits a quarter second, uses the host's default rate for
“Look here.”, reveals “Read slowly” at eight clusters per second, then returns to
the default rate. A plain renderer may draw `node.text` and ignore all visual
styles. See [text presentation and reveal](presentation.md#rich-presentations-and-timing)
for accessing runs, pauses and origin ranges, and for manual finish behavior.

Per-field limits: 65,536 UTF-8 bytes for templates and resolved text, 4,096 tokens,
16 nested marks, 256 pauses and 120 seconds of total pauses.

## Named style vocabulary

`[style=key]…[/style]` is a single built-in mark with a host-defined name. There
are no separate `[b]`, `[i]`, color, font, image or animation tags. Arbitrary valid
style keys can be authored, nested and combined with emphasis. The preview and
runnable showcase agree on common names, but these are **not guaranteed game
rendering effects**: your game decides how each name maps to fonts, colors,
geometry, motion or audio.

### `style=thought` — Thought / italic

**Definition:** An inner thought or reflection. The editor preview uses italic text and Beacon Harbor uses slanted lettering.

**Example:**

```text
: [style=thought]I wonder…[/style]
```

### `style=underline` — Underline

**Definition:** Identifies words that should be underlined. Both the editor preview and Beacon Harbor draw an underline.

**Example:**

```text
: Remember [style=underline]this part[/style].
```

### `style=strike` — Strikethrough

**Definition:** Marks crossed-out, corrected or obsolete prose. Both the editor preview and Beacon Harbor draw strikethrough.

**Example:**

```text
: [style=strike]Old route[/style] Take the new path.
```

### `style=highlight` — Highlight

**Definition:** Draws attention to a phrase. Both the editor preview and Beacon Harbor display a highlighted background.

**Example:**

```text
: The [style=highlight]key clue[/style] is here.
```

### `style=whisper` — Whisper

**Definition:** Signals a quiet aside. The editor preview and Beacon Harbor use muted text color; it does not automatically change audio volume.

**Example:**

```text
: [style=whisper]Be quiet.[/style]
```

### `style=warning` — Warning

**Definition:** Signals caution. The editor preview uses a wavy underline; Beacon Harbor uses a warning color and shake.

**Example:**

```text
: [style=warning]Danger ahead![/style]
```

### `style=success` — Success

**Definition:** Signals a successful outcome. The editor preview uses a green underline; Beacon Harbor uses a success color.

**Example:**

```text
: [style=success]The gate is unlocked.[/style]
```

### `style=radio` — Radio

**Definition:** Suggests a transmitted message. The editor preview uses a monospace font; Beacon Harbor uses its radio accent color.

**Example:**

```text
: [style=radio]Harbor radio, calling in.[/style]
```

### `style=wave` — Wave

**Definition:** Suggests moving text. The editor preview annotates the custom style without motion; Beacon Harbor draws a vertical wave.

**Example:**

```text
: [style=wave]The water is moving.[/style]
```

### `style=shake` — Shake

**Definition:** Suggests trembling text. The editor preview annotates the custom style without motion; Beacon Harbor draws a shake.

**Example:**

```text
: [style=shake]Oh no![/style]
```

### `style=clue` — Custom style

**Definition:** Any valid key can describe a game-specific treatment. The editor preview annotates the custom key; a renderer without a mapping should fall back to readable plain text.

**Example:**

```text
: [style=clue]An important clue.[/style]
```

!!! tip "Use the formatting toolbar"
    The six editor buttons insert or remove emphasis (`[em]`) and the named
    styles `thought`, `underline`, `strike`, `highlight` and `whisper`.
    Select **only the dialogue prose** in a rich script to enable them. See
    [formatting toolbar](editor.md#format-rich-text-with-the-toolbar).

Unknown named styles remain valid metadata and should fall back to readable
text. The showcase disables motion effects in reduced motion mode; the core
runtime does not load fonts, shaders, images or sounds or automatically animate
text.

Try the [LÖVE showcase](showcase.md) and press **F3** for the rich-text tour.
Its [`rich_styles.tua`](https://github.com/kyonru/tua-lang/blob/main/examples/dialogue/showcase/src/rich_styles.tua)
illustrates how rendering can consume these semantic keys.

## Authoring rules and common mistakes

- Labels, text IDs and event names begin with an ASCII letter or underscore, then
  use letters, digits, underscores, dots or hyphens. Import aliases are simple
  identifiers. `else` and `endif` are reserved labels. Display text accepts Unicode.
- Give every utterance, choice and reason a project-unique text ID. Blank lines
  and ordinary comments may separate an ID from its text; another ID, label or
  unrelated directive may not. Event/action IDs are unique within the document
  and use a separate namespace from text IDs.
- Declare characters and imports before labels. Inline characters and shared
  imports can coexist, but their aliases must not collide.
- The host may start at a named label. Use `@entry` to describe external entries
  to the checker; it does not create a separate conversation.

| Symptom | Check |
| --- | --- |
| Formatting buttons are disabled even with selected text | Put `@text rich` immediately after `@dialogue 1`. Select text inside one line, choice or reason; exclude speaker/target/end syntax. |
| A style does not appear in game | The renderer must consume the semantic runs and map the style; `node.text` is deliberately plain. |
| An event never fires | The host must call `update`; check the reveal flag, handler name and whether the line ended early. |
| `advance()` does nothing | You may be at a choice; use `choose()` with a stable option ID. |
| The final line stays visible | `(end)` still waits for acknowledgement. |
| A portrait does not appear | The host must load and render the snapshot's portrait identifier. |
| A character reference reports `TL5012` | Check the imported module, exported member, required string name and optional string portrait. |
| Edited text still appears unchanged | Rebuild and refresh the running document/player and Lua module cache. |
