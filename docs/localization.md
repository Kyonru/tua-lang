---
icon: lucide/languages
---

# Localization

Tua can check translation keys and bundle a small localization runtime for a
LOVE game. The integration is **off by default**. Enable it in the project's
`tua.toml` when you want Tua to own the catalogs:

```toml
[i18n]
enabled = true
default_locale = "en"
fallback_locale = "en"

[[i18n.catalogs]]
locale = "en"
path = "locales/en.lua"

[[i18n.catalogs]]
locale = "fr"
path = "locales/fr.lua"
```

Catalog paths are relative to `source`, so the example above reads
`src/locales/en.lua` and `src/locales/fr.lua`.
Path components cannot contain dots (for example, `en.us.lua` is rejected),
because the bundled runtime loads catalogs through Lua `require`.
Each file is a Lua module with a table under its configured locale name:

```lua
-- src/locales/en.lua
return {
  en = {
    menu = { greeting = "Hello, %{name}!" },
    hud = {
      coins = { one = "%{count} coin", other = "%{count} coins" },
    },
  },
}
```

An enabled project needs at least one catalog. Locale tags use canonical
hyphenated spelling such as `en` or `pt-BR`; the default and fallback tags
must have configured catalogs.

Start with the locale files and `[i18n]` settings, then replace literal
player-facing text with stable catalog keys. Keep gameplay decisions and UI
layout in your game; the runtime returns translated values and does not draw or
measure text.

| Task | Start with… |
| --- | --- |
| Translate a string or plural value | [Use catalogs at runtime](#use-catalogs-at-runtime) |
| Find missing keys or compare locales | [Editor assistance](#editor-assistance) |
| Localize authored dialogue | [Authored dialogue graphs](#authored-dialogue-graphs) |
| Map voice recordings to translated text | [Spoken audio and subtitles](#spoken-audio-and-subtitles) |

In a `.tua` file, use `I18n` without a `require`:

```tua
function love.load()
  I18n.setLocale("fr")
end

function love.draw()
  love.graphics.print(I18n.t("menu.greeting", { name = "Ada" }), 20, 20)
  love.graphics.print(I18n.t("hud.coins", { count = 2 }), 20, 50)
end
```

## Use catalogs at runtime

`I18n.t(key, args?, locale?)` returns a translated string, a string array for
dialogue entries, or `nil` if the key has no usable translation. The optional
third argument chooses a locale for one lookup without changing the current
locale. `I18n.getLocale()` returns the current locale;
`I18n.getLocales()` returns configured catalog tags. `I18n.formatNumber`,
`I18n.formatPrice`, and `I18n.formatDate` expose the bundled formatter.

Lookups try the selected locale, its parent tags, then the configured fallback.
A per-call locale uses the same chain without changing the player's selection.
Restore a saved locale during startup; Tua does not choose one from the operating
system.

The adapter initializes on first `require` and retains a previously selected
locale when reloaded. To pick up edited catalog data, restart the game or reload
the catalog modules explicitly.

Use the optional locale argument for a one-off lookup without changing the
player's selected locale:

```tua
local greeting = I18n.t("menu.greeting", { name = "Ada" }, "fr")
if greeting then
  love.graphics.print(greeting, 20, 20)
end
```

The lookup tries `fr`, then the fallback locale. `I18n.getLocale()` still
returns the locale selected for the game. A regional tag such as `fr-CA` can
also use the configured parent catalog when that tag is not listed directly.

## Editor assistance

The language server indexes literal keys in configured catalogs without running
their Lua code. It suggests keys in `I18n.t("...")`, locales in
`I18n.setLocale("...")` and the third `t` argument, and named fields in an
unfinished argument table. Hover shows a translation and the catalog it came
from; definition opens that entry. References and rename operate on literal
uses and catalog entries. They also include [authored dialogue text IDs](dialogue/editor.md#rename-a-text-identity),
with duplicate/conflict checks and project-local edits. Rename accepts a new final
key segment, retaining the existing namespace.

Diagnostics cover unknown keys, absent named fields,
unconfigured literal locales, missing plural `other` forms, mismatched
placeholders, malformed catalog roots, and missing configured files. A catalog
warning summarizes keys missing in each nonfallback locale.

For an unknown key
whose parent is a literal table in the fallback catalog, the quick fix inserts
a clearly unfinished `TODO` translation there.

A literal key with a static text or plural fallback and consistent string-shaped
locale entries has type `string`; a consistent dialogue key has type `string[]`.
When a locale's plural rule selects a form absent from its catalog, the runtime
uses that entry's required `other` text.
Calculated or mixed-shape keys retain the broader `string | string[] | nil`
result. This precision applies to project checking and ordinary local
inference. A dynamic key can still fail at runtime, so handle `nil` where a
translation is not statically guaranteed.

**Tua: Show Localization Explorer** opens a key × locale coverage table for
the project containing the active Tua or Lua file. It
labels exact translations, fallback results, missing entries, and entries
calculated by Lua. Select a key to compare its catalog text and plural forms,
then open the source entry. Search and paging keep large catalogs manageable.
This is a static source view; it does not claim to show the game's current
locale or rendered pixel width.

`I18nKey` and `I18nLocale` are project types available only while localization
is enabled. Use `I18nKey` for a wrapper parameter when you want callers to get
catalog-key suggestions, key diagnostics, hover, and navigation across imports.
Both aliases remain open to strings: games can compute
keys and use regional tags whose parent catalog is configured. Literal calls
still receive catalog diagnostics.

The catalog index reads only literal tables. Computed
entries remain runtime Lua; the editor marks them as dynamic and cannot prove
their result. The checker never executes a catalog. Catalog Lua runs as game
code when the emitted game loads it, so use trusted catalog files.

When a key is calculated at runtime, handle the nullable result explicitly:

```tua
local function drawMenuLabel(key: string)
  local label = I18n.t(key)
  if label then
    love.graphics.print(label, 20, 20)
  else
    love.graphics.print("?", 20, 20)
  end
end
```

The fallback is useful for keys Tua cannot prove statically. Literal indexed
keys with consistent string-shaped catalogs can be known to return `string`;
computed keys can still be missing at runtime.

Translation text still needs a game font containing the selected language's
glyphs. Tua does not perform text shaping, UI mirroring, or layout overflow
measurement.

## Authored dialogue graphs

[Dialogue localization](dialogue/localization.md#localize-lines-choices-and-character-names) maps
`.tdialogue` line/reply IDs to this same catalog index. Pass `I18n.dialogue` as the
player resolver with an explicit locale. It returns scalar text, selects existing
plural/fallback rules and safely inserts `%{name}` arguments as literal text;
`I18n.t` keeps its existing string/array/nil contract.

The Explorer includes authored keys without translations and a separate source
prose/navigation panel. `tua dialogue-extract` prints a source manifest for reviewed
catalog updates without executing Lua or overwriting translator edits. Open the
Explorer from `.tua`, `.lua` or `.tdialogue` files.

## Spoken audio and subtitles

`I18n` catalogs contain text, not sound files. Keep subtitles under stable
translation keys and define recordings as named [audio cues](audio.md). For a
small English/French game with short `src/assets/voice/<locale>/intro.wav`
recordings, add `dialogue.intro` as text in both catalogs and select a cue when
the line plays:

```tua
local audio = audioSystem({
  cues = {
    intro_en = { source = sound("assets/voice/en/intro.wav") },
    intro_fr = { source = sound("assets/voice/fr/intro.wav") },
  },
})

local function playIntro()
  local subtitle = I18n.t("dialogue.intro")
  if I18n.getLocale() == "fr" then
    audio:play("intro_fr")
  else
    audio:play("intro_en")
  end
  return subtitle
end
```

This example intentionally falls back to the English recording. Tua checks
the literal cue names and referenced asset paths, but it does not infer that
`intro_en` and `intro_fr` are variants of one line or require a recording for
every configured locale. Its text fallback chain does not select audio: map
regional tags such as `fr-CA` to a voice language explicitly if your game uses
them.

For a game that offers independent text and voice languages, use the
player's voice-language setting here instead of `I18n.getLocale()`. Keep the
localized subtitle available when a recording is absent.

Create reusable sources during setup, not on each frame. `sound(...)` is for
short static clips; use `music(...)` for longer recordings that should stream,
following [LÖVE's source guidance](https://love2d.org/wiki/SourceType). Plan
for the size of all language recordings included in a build. Tua currently has
no locale-aware voice coverage diagnostic, voice-variant completion, or
automatic subtitle-to-recording timing.

## Build and interop

When a `.tua` file references the built-in `I18n`, generated Lua imports
`tuai18n`. The build copies the adapter, its pinned Smiti18n modules and MIT
notice, a generated configuration module, and the configured Lua catalogs.
This also applies when passing or storing `I18n` as a value; an unrelated table
field named `I18n` does not import it.

Enabled projects that never use `I18n` do not copy the runtime. Disabled
projects do not gain the built-in or an injected import. A local/imported
binding named `I18n` takes precedence, and ordinary third-party `require`
calls retain their Lua meaning.

The bundled Smiti18n revision is recorded in
`vendor/runtime/smiti18n/UPSTREAM.md`; its license is copied with every build
that uses the adapter. See the runnable
[`examples/features` localization source](https://github.com/kyonru/tua-lang/blob/main/examples/features/src/localization.tua)
for a small game-facing example.
