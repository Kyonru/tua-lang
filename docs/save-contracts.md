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

For a typical save flow, define a versioned contract, load it before building
game state, bind the relevant store, and poll the binding from `love.update`.
Flush at a deliberate checkpoint such as `love.quit`. Loading does not write or
migrate the slot by itself; keep recovery choices in the game.

| If you need to… | Start with… |
| --- | --- |
| Validate data without using files | [Validate data without persistence](#validate-data-without-persistence) |
| Declare a save and connect it to game state | [Declare a save contract](#declare-a-save-contract) |
| Read an older save safely | [Validate historical save data before migration](#validate-historical-save-data-before-migration) |
| Recover from damaged or incompatible data | [Recovery and errors](#recovery-and-errors) |

For a runnable game, follow [Save progress and preferences](tutorials/arena-game/save-progress.md).
It connects between-wave checkpoints, autosave, historical validation and recovery UI.

## Validate data without persistence

`Save.shape(Save.record({...}))` provides `{defaults, copy}` without a slot, backend,
filesystem access or LÖVE. `defaults` is an owned initial value; `copy(value)` validates
and returns another owned value with the inferred record type. It does not fill missing
required fields or silently drop unknown fields. The same finite-number, plain-table,
dense-array, depth (32) and entry-count (100,000) limits apply as for save contracts.
Required fields need defaults; optional fields may be absent.

```tua
local shape = Save.shape(Save.record({ name = Save.string(""), count = Save.number(0) }))
local data = shape.copy({ name = "Ada", count = 2 })
```

Dialogue context definitions reuse this helper for safe per-player copies. Creating a
shape captures the schema and default data; later edits to its source descriptors do
not rewrite that captured schema. The returned `defaults` value itself is mutable;
copy it before using it as independent state.

## Declare a save contract

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
schema must be a record. `Profile:defaults()` returns a fresh deep copy.

A literal schema gives completion and type checking for the snapshot, including
after importing `Profile` from another file. Literal `Save.enum({ "ready", "waiting" }, "ready")` lists retain the
`"ready" | "waiting"` union, including inside arrays and optional fields and across
module imports. Computed mutable lists remain `string`-valued. A computed schema
remains valid at runtime, but editor types can be less specific.

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

Load before constructing systems that depend on saved values. Keep the loaded
snapshot separate from the persistence binding: the contract validates and
copies data, while `bind()` observes one store field and schedules writes.

### Validate historical save data before migration

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
when the contract is created.

Validation rejects missing/unknown fields, invalid
values and mismatched JSON arrays/objects; defaults do not repair old data.
A successful check gives the callback an owned plain-data copy without decoder
markers.

Failure returns `invalid_migration_input` in phase `validate`, does not
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
verify the inactive file's length and SHA-256 checksum before advancing the
generation.

If only one file is valid, `load()` returns its snapshot with
`damaged_slot_unverified`. The damaged file may contain newer progress, so
writes remain blocked until the player accepts recovery. Other load errors,
including a future save version, also block writes. Use the
[recovery table below](#choose-a-recovery-response) to choose a response.

Every `SaveIssue` has `code`, `message`, `slot`, `phase` (`read`, `decode`,
`migrate`, `validate`, `write`, or `verify`), and `recoverable`.

Slot names must be 1–64 ASCII identifier characters without path separators.
The payload cap defaults to 128 KiB and can be raised to 4 MiB with `max_bytes`.
Saving is synchronous; measure on target hardware before increasing the cap
for autosave.

Decoding limits nesting and entry counts. It rejects cycles, sparse arrays,
non-finite numbers, invalid UTF-8, functions, userdata, unknown record fields,
and oversized input.

Set a stable `t.identity` in `conf.tua` before loading saves. The checksum
detects accidental corruption; it is not an anti-cheat signature. The two-slot
protocol does not guarantee power-loss durability (`fsync`), concurrent game
process coordination, or cloud-sync conflict resolution. The LSP never reads
player save files. Packaged games exclude `__tua_saves/`, and a custom
`tuasave.lua` in the project can replace the bundled runtime.

### Choose a recovery response

Treat load issues as product decisions instead of silently replacing data:

| Load result | Recommended response |
| --- | --- |
| `nil, nil` | Start from `Profile:defaults()`; this is a new slot. |
| Snapshot and no issue | Continue with the validated snapshot. |
| Snapshot with `damaged_slot_unverified` | Offer the recovered snapshot, explain that the other generation may be newer, and call `acceptRecovery()` only after the player accepts the risk. |
| No usable snapshot or another issue | Keep writes blocked; offer a deliberate retry, backup restore, or reset path. |

`reset()` deletes both generations; it is not a repair operation. Keep it behind
an explicit confirmation when progress matters. If a write fails, retain the
binding and surface the issue. Scheduled retries use backoff, while `flush()`
lets the game decide when to try synchronously again.


### Save a consequential dialogue result

The [durable dialogue example](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/durable)
keeps local gameplay counters and an operation receipt in one Save record. It
persists intent before dispatch, reopens uncertain writes, and returns a committed
result on recovery without applying the purchase again. This single-writer recipe
also stops on damaged-slot issues; it never automatically accepts a potentially
older receipt. External side effects require an idempotent authority of their own.

For dialogue position persistence, the [checkpoint recipe](https://github.com/Kyonru/tua-lang/tree/main/examples/dialogue/checkpoint)
uses an explicit typed envelope and fresh contracts. Save validates its shape;
dialogue restore validates content compatibility and rebinds current game facts.
See [persist checkpoints](dialogue.md#persist-checkpoints-with-save) for ownership
and error handling.
