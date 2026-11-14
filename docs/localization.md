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
`src/locales/en.lua` and `src/locales/fr.lua`. Each file is a Lua module with a
table under its configured locale name:

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

`I18n.t(key, args?, locale?)` returns a translated string, a string array for
dialogue entries, or `nil` if the key has no usable translation. The optional
third argument chooses a locale for one lookup without changing the current
locale. `I18n.getLocale()` returns the current locale;
`I18n.getLocales()` returns configured catalog tags. `I18n.formatNumber`,
`I18n.formatPrice`, and `I18n.formatDate` expose the bundled formatter.

The runtime tries the selected locale, each parent tag, and the configured
fallback locale. A per-call locale follows that chain without consulting or
changing the player's current locale. It initializes on first `require`, before a lookup, so it does
not depend on `love.load`. If the adapter itself is reloaded, it retains a
locale previously chosen with `setLocale`. Restart the game or explicitly
reload catalog modules to pick up changed Lua catalog data. Restore a player's
saved locale yourself during startup; Tua does not guess the operating system
locale.

## Editor assistance

The language server indexes literal keys in configured catalogs without running
their Lua code. It suggests keys in `I18n.t("...")`, locales in
`I18n.setLocale("...")` and the third `t` argument, and named fields in an
unfinished argument table. Hover shows a translation and the catalog it came
from; definition opens that entry. References and rename operate on literal
uses and catalog entries. Diagnostics cover unknown keys, absent named fields,
unconfigured literal locales, missing plural `other` forms, mismatched
placeholders, malformed catalog roots, and missing configured files. A catalog
warning summarizes keys missing in each nonfallback locale. For an unknown key
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

**Tua: Show Localization Explorer** opens a key × locale coverage table. It
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
still receive catalog diagnostics. The catalog index reads only literal tables. Computed
entries remain runtime Lua; the editor marks them as dynamic and cannot prove
their result. The checker never executes a catalog. Catalog Lua runs as game
code when the emitted game loads it, so use trusted catalog files.

Translation text still needs a game font containing the selected language's
glyphs. Tua does not perform text shaping, UI mirroring, or layout overflow
measurement.

## Build and interop

When a `.tua` file references the built-in `I18n`, generated Lua imports
`tuai18n`. The build copies the adapter, its pinned Smiti18n modules and MIT
notice, a generated configuration module, and the configured Lua catalogs.
Enabled projects that never use `I18n` do not copy the runtime. Disabled
projects do not gain the built-in or an injected import. A local/imported
binding named `I18n` takes precedence, and ordinary third-party `require`
calls retain their Lua meaning.

The bundled Smiti18n revision is recorded in
`vendor/runtime/smiti18n/UPSTREAM.md`; its license is copied with every build
that uses the adapter. See the runnable
[`examples/features` localization source](https://github.com/kyonru/tua-lang/blob/main/examples/features/src/localization.tua)
for a small game-facing example.
