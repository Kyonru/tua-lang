Smiti18n runtime, pinned at 7246ba75e36a29ff045ee2fe1073036eb24f009d
https://github.com/Oval-Tutu/smiti18n

The six Lua modules originate from the pinned revision. `init.lua` has two
local adaptations:

- When a locale's plural rule selects an absent form, use its required `other` form.
- `resolve(key, locale)` returns a borrowed, read-only raw entry and its resolved
  locale using the existing store and fallback chain. Tua's scalar dialogue adapter
  uses it to insert named arguments literally, without invoking printf formatting
  on user-provided values. Existing `translate` behavior is unchanged.

LICENSE must accompany distributions containing these files.
Upstream sample locales and tests are not shipped in game builds.
