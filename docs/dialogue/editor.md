---
icon: lucide/panel-top
---

# Editor tools and previews

Author, navigate and refactor `.tdialogue` scripts in VS Code or VSCodium. Use the source preview to explore authored routes and the runtime inspector to observe a running player.

[Dialogue overview](index.md)

## Format rich text with the toolbar

!!! important "Enable rich mode first"
    The toolbar works only in rich-text dialogue files. Put `@text rich`
    immediately after `@dialogue 1`, before imports or labels:

    ```text
    @dialogue 1
    @text rich
    ```

    Select the spoken text, choice label or disabled reason itself, excluding
    the speaker name, `[target:…]` and `(end)` control syntax.

The `.tdialogue` editor has a persistent row of formatting icons at the top right
of its title bar: **B** (emphasis), *I* (thought), underline, strikethrough,
highlight and whisper. They stay visible and disabled until you select valid rich
text within one line, choice, or `@reason`. These are native editor buttons, not
hover actions. Narrow editor groups may place overflowing actions in **…**.

Click a button to apply its style; the formatted fragment stays selected so you
can combine styles. An outlined active icon means its mark covers exactly the selected
content. Click it again to remove that mark, keeping other styles and text.
For example, select `hello`, click **B**, then *I*: both styles become active.
Click **B** again to keep only the thought style. Partial or mixed selections add
a mark without removing existing marks outside the selected content. Each edit
supports a single undo step.

You can also right-click **Format Dialogue Text** or run **Tua: Format Dialogue
Text** from the command palette. The toolbar edits source markup, not Markdown
or a WYSIWYG document. Named styles such as `[style=thought]` need support from
your game's renderer; the editor preview provides its own presentation.
Selections must keep placeholders, escapes and existing markup intact.
Multi-cursor selections and malformed rich text are not formatted. Hide the
title-bar buttons with `tua.dialogue.formattingToolbar`; the formatting command
stays available. See the [named style vocabulary](dialect.md#named-style-vocabulary)
for the tags each button inserts, and how the preview interprets them.

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
Use the [Flow preview](#preview-the-authored-conversation) to explore the authored routes visually.

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
**Source** and line-number links reuse the source file’s existing tab, including
a hidden tab. If source and preview share a group, the preview moves aside so
both remain visible. If the source is closed, it opens in a separate group.

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

### Portraits in the reader

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
[editor reference](../lsp.md) for refresh behavior and display limits.

## Inspect a running dialogue

Enable the [Runtime Inspector](../debugging.md#live-runtime-inspector) and restart the
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
the resolver again. See [sampling limits](../debugging.md#live-runtime-inspector).
