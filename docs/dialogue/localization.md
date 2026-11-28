---
icon: lucide/languages
---

# Dialogue localization

Keep authored text identities stable, select translations at runtime and refresh presentation without replaying gameplay effects.

[Dialogue overview](index.md)

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
[checkpoints](checkpoints-and-history.md#checkpoint-a-plain-conversation).

Use `%{name}` for named scalar arguments. Names are simple ASCII identifiers, not
field paths or executable expressions. With localization enabled, `%%` produces a
literal percent: `%%{name}` displays `%{name}`. A replacement value such as
`"100% %{other}"` is inserted literally and never parsed again. General printf
formatting and `%<name>.d` are not part of this dialogue adapter.

### 2. Configure existing catalogs

Enable `[i18n]` and configure locale files as described in
[Localization](../localization.md). In `src/locales/en.lua`:

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
can carry arguments for several lines. [Typed context bindings](conditions.md#typed-context-and-conversation-locals) support refreshing bound values independently of these initial scalar arguments. Do not put services or mutable gameplay objects in this argument map.

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
