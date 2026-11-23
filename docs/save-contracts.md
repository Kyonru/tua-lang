---
icon: lucide/save
---

# Typed Save Contracts

`Save` is an optional LÖVE persistence helper. A contract declares exactly the
data written to a slot; a store binding decides when to write it. In `.tua`,
using `Save` adds `require("tuasave")` to that file's generated Lua. The build
includes `tuasave.lua` and its JSON codec only when referenced. Plain `.lua`
files use `local Save = require("tuasave")`.
Passing or storing the built-in `Save` value also imports it; a table field
named `Save` does not.

```tua title="src/profile.tua"
local Profile = Save.contract({
  slot = "profile",
  version = 2,
  schema = Save.record({
    highScore = Save.number(0),
    musicOn = Save.boolean(true),
    unlocked = Save.array(Save.string(), {}),
    settings = Save.record({ volume = Save.number(0.8) }),
  }),
  migrations = {
    [1] = function(old: any)
      return { highScore = old.score or 0, musicOn = true,
        unlocked = {}, settings = { volume = 0.8 } }
    end,
  },
})

return Profile
```

`Save.record`, `Save.array`, `Save.number`, `Save.string`, `Save.boolean`,
`Save.optional`, and `Save.enum` make a small JSON-shaped schema. A required
field needs a default; optional fields may be absent. Arrays have dense numeric
indices, and an empty array stays distinct from an empty record. The root
schema must be a record. `Profile:defaults()` returns a fresh deep copy. A
literal schema gives completion and type checking for the snapshot, including
after importing `Profile` from another file. A computed schema remains valid at
runtime, but editor types can be less specific.

```tua title="src/main.tua"
local Profile = require("profile")
local saved, issue = Profile:load()
if issue then print(issue.code, issue.message) end
local game = store({ profile = saved or Profile:defaults(), currentEnemy = nil })
local persistence = Profile:bind(game, {
  select = function(state) return state.profile end,
  debounce = 1.0,
  max_interval = 10.0,
})

function love.update(dt: number)
  persistence:update(dt)
end

function love.quit()
  local ok, writeIssue = persistence:flush()
  if not ok and writeIssue then print(writeIssue.code, writeIssue.message) end
end
```

`Profile:load()` returns `snapshot | nil, issue | nil`. `nil, nil` means no save
yet. A migration converts directly from an old version to the current schema;
it runs in a protected call and its result is validated before use. Migration
entries may reference a named or imported function, as well as an inline callback.
Known function signatures are checked against `(old: any) -> Snapshot`; invalid
return shapes and extra required parameters are rejected. Loading
does not rewrite files.

Optional `migration_schemas` validate historical input before calling a migration:

```tua
local Profile = Save.contract({
  slot = "profile",
  version = 2,
  schema = Save.record({ score = Save.number(0) }),
  migration_schemas = {
    [1] = Save.record({ points = Save.number(0) }),
  },
  migrations = {
    [1] = function(old: { points: number }): { score: number }
      return { score = old.points }
    end,
  },
})
```

Each historical schema must be a record and have a matching direct migration for
an integer version below the current version. Historical descriptors are copied
when the contract is created. Validation rejects missing/unknown fields, invalid
values and mismatched JSON arrays/objects; defaults do not repair old data.
A successful check gives the callback an owned plain-data copy without decoder
markers. Failure returns `invalid_migration_input` in phase `validate`, does not
call the migration or rewrite files, and blocks writes under the normal error
policy. The callback result is still validated against the current schema.

Literal historical entries also check named/imported migration parameter types
against their input schema; explicitly typed callbacks retain ordinary field
completion and hover. Computed schema maps retain runtime validation but may have
less precise static checks. Entries without a historical schema keep the existing
`old: any` callback behavior, including their original decoded payload. Migrations
still convert directly to the current version, never through an automatic chain.

Use `Profile:save(snapshot)` for an explicit checkpoint.
`persistence:update(dt)` polls the store revision and writes after the quiet
`debounce` period or `max_interval` during continuous changes. `flush()` writes
synchronously when dirty. `dispose()` does not flush. Call `flush()` before
`dispose()` if the game needs a final checkpoint.

The binding only observes `store:set()` and `store:update()`. If you directly
mutate the table returned by `store:get()`, call `persistence:markDirty()`.
Neither `set` nor `update` performs serialization or filesystem I/O.

## Recovery and errors

Each slot has two files under `__tua_saves/` in LÖVE's save directory. Writes
target the inactive file, read it back, and verify its length and SHA-256
checksum before advancing the generation. If one slot is damaged and the other
contains a valid snapshot, `load()` returns that snapshot with
`issue.code == "damaged_slot_unverified"`. The damaged slot's generation cannot
be trusted, even if part of its header is readable: the selected snapshot
could be newer or older. The contract blocks writes until the game tells the
player and calls `Profile:acceptRecovery()`, explicitly accepting the risk of
overwriting the damaged slot. Any other load error, including a future save
version, stays blocked. `Profile:reset()` is an explicit destructive choice:
it removes both slot files and returns fresh defaults. Do not call it without
an appropriate player-facing recovery choice.

Every `SaveIssue` has `code`, `message`, `slot`, `phase` (`read`, `decode`,
`migrate`, `validate`, `write`, or `verify`), and `recoverable`. A failed write
keeps a binding dirty and scheduled retries back off. The slot name is a
1–64 character ASCII identifier; it cannot contain path separators. The
default payload cap is 128 KiB, configurable up to 4 MiB with `max_bytes`.
The lower default avoids accidentally scheduling a large synchronous write
from a game update. On one macOS LÖVE 11.5 run, a 1 MiB payload took roughly
80 ms to save and 187 ms to reload; measure your target hardware before
raising the cap for an autosave binding.
Decoding also limits nesting and entry counts. The JSON subset rejects cycles,
sparse arrays, non-finite numbers, invalid UTF-8, functions, userdata, unknown
record fields, and oversized input.

Set a stable `t.identity` in `conf.tua` before loading saves. The checksum
detects accidental corruption; it is not an anti-cheat signature. The two-slot
protocol does not guarantee power-loss durability (`fsync`), concurrent game
process coordination, or cloud-sync conflict resolution. The LSP never reads
player save files. Packaged games exclude `__tua_saves/`, and a custom
`tuasave.lua` in the project can replace the bundled runtime.
