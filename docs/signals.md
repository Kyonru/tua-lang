---
icon: lucide/radio
---

# Signals

`signal()` creates a lightweight event bus. One part of a game emits a named
event and any number of handlers react to it, so a damage system, a score
display, and a sound effect can respond to `"player.hit"` without knowing about
each other.

Tua checks event names and payloads from the handlers and declarations it can
see, completes known event names, and lets you rename an event across a project.

=== "Tua"

    ```tua
    type Player = { name: string, hp: number }

    local events = signal()

    events:on("player.hit", function(player: Player, damage: number)
      player.hp -= damage
    end)

    events:on("player.hit", function(player: Player, damage: number)
      print(player.name .. " took " .. damage)
    end)

    local hero: Player = { name = "Ada", hp = 10 }
    events:emit("player.hit", hero, 4)
    ```

=== "Generated Lua"

    ```lua
    local __tua_signal = require("tuasignal")
    local events = __tua_signal()

    events:on("player.hit", function(player, damage)
      player.hp = player.hp - damage
    end)

    events:on("player.hit", function(player, damage)
      print(player.name .. " took " .. damage)
    end)

    local hero = { name = "Ada", hp = 10 }
    events:emit("player.hit", hero, 4)
    ```

!!! info "Explicit buses, not a global event system"
    `signal` is a constructor. Share one bus by returning it from a module, or
    create separate buses for a scene, a UI screen, or a test.

## Signal API

| Method | Behavior |
| --- | --- |
| `on(name, handler)` | Add `handler` to `name` and return `handler`. |
| `once(name, handler)` | Add a handler that removes itself before its first call. Returns the **wrapper** that was registered. |
| `emit(name, ...)` | Call every handler for `name`, in registration order, with the payload values. |
| `off(name, handler)` | Remove every registration of `handler` from `name`. |
| `off(name)` | Remove all handlers for `name`. |
| `clear(name)` | Remove all handlers for `name`. |
| `clear()` | Remove every handler on the bus. |
| `has(name)` | Return `true` when `name` has at least one handler. |
| `listeners(name)` | Return a copy of the handlers for `name`; editing the copy does not change the bus. |
| `declare(name, schema)` | Describe an event's payload for the type checker without registering a handler. Returns `schema`, or the bus when no schema is given. |

`register` and `remove` are aliases for `on` and `off`, matching hump.signal.

Event names must be strings and handlers must be functions; anything else
raises an error at the call. Registering the same function twice calls it twice
per emit.

### Removing Handlers

Keep the value returned by `on` when you need to remove a handler later.
Anonymous functions can't be removed any other way:

```tua
local onHit = events:on("player.hit", function(player: Player, damage: number)
  print(damage)
end)

events:off("player.hit", onHit)
```

`once` registers a wrapper around your function, so `off` with the original
function does not remove it. Use the value `once` returns:

```tua
local onFirstHit = events:once("player.hit", function(player: Player, damage: number)
  print("first blood")
end)

-- Cancel it before it ever runs:
events:off("player.hit", onFirstHit)
```

### Emitting Inside A Handler

A handler may emit other events, register handlers, or remove them, including
itself. The emit that is already running finishes with the handler list it
started with, and the change applies to nested and later emits:

```tua
events:on("wave.cleared", function()
  events:on("wave.cleared", function() print("added during emit") end)
end)

events:emit("wave.cleared") -- runs only the first handler
events:emit("wave.cleared") -- runs both
```

The runtime uses copy-on-write handler lists to do this, so an ordinary emit
copies nothing and allocates nothing.

If a handler raises an error, the error propagates out of `emit` and the
remaining handlers for that emit don't run. The bus stays usable afterward.

## Typed Payloads

Tua learns an event's payload from typed handlers and from `declare`:

```tua
events:declare("game.paused", function(reason: string) end)

events:emit("game.paused", "menu") -- accepted
events:emit("game.paused", 3) -- TL1003: expected string
```

`declare` is useful when the handlers live in another module, or when an event
is emitted before anything listens to it. The schema function is never called;
it only describes the payload. A declaration with no schema,
`events:declare("game.paused")`, records the name for completion only.

Signature help on `emit` shows the expected payload. Two typed handlers or
declarations that disagree about one event's payload report `TL5005`. Untyped
handlers and buses Tua can't resolve statically keep `emit` permissive, so
dynamic code still compiles.

## Pattern Methods

Pattern methods act on every event whose name matches a
[Lua pattern](https://www.lua.org/manual/5.1/manual.html#5.4.1):

| Method | Behavior |
| --- | --- |
| `onPattern(pattern, handler)` | Add `handler` to every currently registered event whose name matches. Returns `handler`. |
| `emitPattern(pattern, ...)` | Emit every currently registered event whose name matches. |
| `offPattern(pattern, handler)` | Remove `handler` from every matching event; with no handler, remove all of their handlers. |
| `clearPattern(pattern)` | Remove all handlers from every matching event. |

`registerPattern` and `removePattern` are aliases for `onPattern` and
`offPattern`.

```tua
-- Silence every enemy event when a cutscene starts.
events:clearPattern("^enemy%.")

-- Stop one handler on every UI event it was attached to.
events:offPattern("^ui%.", refreshHud)
```

!!! warning "Patterns match names that exist now"
    Pattern methods iterate the events that currently have handlers. They don't
    subscribe to future names: `onPattern("^enemy%.", h)` attaches `h` only to
    enemy events that already have a handler, and an event first registered
    afterward won't call `h`. Likewise `emitPattern` reaches only events that
    have handlers. Use explicit names when an event may not have been
    registered yet.

In Lua patterns, `.` matches any character. Escape it as `%.` when matching
dotted names such as `enemy.spawned`, and anchor with `^` so `"enemy"` doesn't
also match `"boss.enemy"`.

## Share A Bus Through A Module

Lua caches `require` results, so returning a bus from a module shares one
instance across the game:

```tua
-- src/events.tua
local events = signal()
return events
```

```tua
-- src/player.tua
local events = require("events")

events:emit("player.jumped")
```

Tua indexes event names across files, so completion and checking work in both
modules.

For state that only matters while a scene is active, create a bus for the
scene and call `clear()` when leaving it. That drops every handler at once, so
the scene can't leak handlers onto a shared bus.

## Editor Support

- **Completion:** inside the first string argument of `on`, `once`, `emit`,
  `off`, `declare`, `has`, `listeners`, and their aliases, the language server
  suggests known event names. It ranks them by the same bus, then the same
  file, typed declarations, nearby calls, and the rest of the project.
- **Navigation:** hover, go to definition, find references, and rename work on
  event-name strings and understand which bus each call uses.
- **Signal Graph:** run **Tua: Show Signal Graph** to see every indexed event
  across the project. It filters by bus, file, listener, or emitter, shows
  conflicting payload schemas, and can create a typed listener or an empty
  `declare` from a known payload.

## Runtime And Overrides

`signal()` lowers to a cached `require("tuasignal")` constructor, and builds
write the bundled `tuasignal.lua` only when a file uses it. A `tuasignal.lua`
already in your project is never overwritten, so you can replace the runtime
with your own implementation of the same methods.

The audio system uses `tuasignal` internally for its events; see
[Audio](audio.md).
