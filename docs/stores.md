---
icon: lucide/database
---

# State Stores

`store({...})` creates an explicit shared runtime-state owner. Tua derives the
state shape from the initial table, so reads, replacements, updates,
subscriptions, and keyed watchers receive completion and type checking without
a generic annotation or package install.

=== "Tua"

    ```tua
    type GameState = {
      score: number,
      paused: boolean,
      playerName: string,
    }

    local game = store({
      score = 0,
      paused = false,
      playerName = "Ada",
    })

    game:update(function(state: GameState)
      state.score += 10
    end)

    local state: GameState = game:get()
    print(state.score)
    ```

=== "Generated Lua"

    ```lua
    local __tua_store = require("tuastore")
    local game = __tua_store({
      score = 0,
      paused = false,
      playerName = "Ada",
    })

    game:update(function(state)
      state.score = state.score + 10
    end)

    local state = game:get()
    print(state.score)
    ```

!!! info "Explicit instances, not a global singleton"
    `store` is a constructor. Put an instance in a normal module and share it
    with `require`, or construct isolated stores for tests, previews, save
    slots, replay simulations, and separate game worlds.

## Store API

| Method | Behavior |
| --- | --- |
| `get()` | Return the current state table. |
| `set(next)` | Replace the state, notify listeners, and return `next`. |
| `update(mutator)` | Run `mutator(state)`, notify listeners, and return the state. |
| `subscribe(listener)` | Run `listener(state, previous)` after every change and return an unsubscribe function. |
| `watch(key, listener)` | Run `listener(value, previous, state)` when one top-level field changes and return an unsubscribe function. |

The initial table supplies the state type:

```tua
local game = store({ score = 0, paused = false })

game:set({ score = 20, paused = true }) -- accepted
game:set({ score = "high", paused = true }) -- TL1003

game:watch("score", function(value: number, previous: number)
  print(previous, value)
end)
```

Completion after `game:get().` offers `score` and `paused`. Signature help for
`watch` derives one overload per field, so the key and callback value type stay
connected. Hovering `"score"` shows its state-field type, and definition on the
key returns to `score` in the initial table. The returned unsubscribe function
is idempotent.

!!! warning "Updates are shallow"
    `update` takes a shallow snapshot before running the mutator. Whole-store
    subscribers always run. A keyed watcher runs when its top-level value
    changes according to Lua's `~=` comparison. Mutating inside the same nested
    table does not change that nested table's identity; replace the top-level
    field or use a whole-store subscription when that distinction matters.

## Share A Store Through A Module

Lua caches successful `require` results, so a normal module is enough to share
one explicit store instance:

```tua title="src/game_state.tua"
local game = store({
  score = 0,
  paused = false,
})

return game
```

```tua title="src/player.tua"
local game = require("game_state")

local function addScore(points: number)
  local current = game:get()
  game:set({
    score = current.score + points,
    paused = current.paused,
  })
end

return { addScore = addScore }
```

The exported specialized store shape participates in normal module inference,
so consumers retain state fields and method signatures.

## Persist With LOVE

Runtime state and save data are separate concerns. The store does not choose a
serialization format, filename, save slot, schema version, or migration policy.
Use a subscription as the adapter to `love.filesystem`:

```tua
local game = require("game_state")

local stopSaving = game:subscribe(function(state)
  local contents = encodeSave(state) -- Your JSON/Lua/binary serializer.
  local ok, message = love.filesystem.write("save.json", contents)
  if not ok then
    print("save failed", message)
  end
end)
```

Load and migrate persisted data before constructing or replacing the store:

```tua
local defaults = { score = 0, paused = false }
local saved = loadAndMigrateSave("save.json")
local game = store(saved or defaults)
```

!!! tip "Control save frequency in the adapter"
    Gameplay may update state every frame. Debounce saves, save at checkpoints,
    or mark state dirty and flush it from a suitable LOVE callback rather than
    writing the filesystem after every update.

## Runtime And Overrides

`store({...})` lowers to `require("tuastore")`. `tua build` writes the bundled
`tuastore.lua` beside generated Lua only when the helper is used. A
project-provided `tuastore.lua` is never overwritten, so compatible custom
implementations can replace the bundled runtime.
