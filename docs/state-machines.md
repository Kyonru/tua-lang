---
icon: lucide/route
---

# State Machines

Tua includes a typed finite state machine through the built-in
`stateMachine({...})` helper. It follows the interface of
[lua-state-machine](https://github.com/kyleconroy/lua-state-machine), requires
no package installation or explicit `require`, and preserves ordinary Lua
runtime behavior.

When the configuration is a static table, Tua derives exact state and event
names for checking and editor tooling. A build lowers the helper to
`require("tuastatemachine").create` and writes the bundled runtime only when it
is required.

## Create A Machine

=== "Tua"

    ```tua
    local light = stateMachine({
      initial = "green",
      events = {
        { name = "warn", from = "green", to = "yellow" },
        { name = "panic", from = "yellow", to = "red" },
        { name = "calm", from = "red", to = "yellow" },
        { name = "clear", from = "yellow", to = "green" },
      },
      callbacks = {
        onenterred = function(self, event, from, to)
          print(event, from, to, self.current)
        end,
      },
    })

    light:warn()
    light:panic()
    ```

=== "Generated Lua"

    ```lua
    local __tua_state_machine = require("tuastatemachine").create
    local light = __tua_state_machine({
      initial = "green",
      events = {
        { name = "warn", from = "green", to = "yellow" },
        { name = "panic", from = "yellow", to = "red" },
        { name = "calm", from = "red", to = "yellow" },
        { name = "clear", from = "yellow", to = "green" },
      },
      callbacks = {
        onenterred = function(self, event, from, to)
          print(event, from, to, self.current)
        end,
      },
    })

    light:warn()
    light:panic()
    ```

The static declaration gives `light.current` the type
`"green" | "red" | "yellow"`. It also creates typed `warn`, `panic`, `calm`,
and `clear` methods. Completion after `light:` offers those event methods,
while completion inside `light:is("...")` and `light:can("...")` offers only
configured state or event names.

!!! tip "Let completion write callback signatures"
    Inside `callbacks`, start typing `on` and choose a generated callback such
    as `onenterred` or `onbeforewarn`. The inserted function annotates `self`,
    `event`, `from`, and `to` with the machine's exact unions.

Hover inside nested options, events, and callbacks shows the specific field or
parameter type. Definition on fixed fields such as `initial` and `name` opens a
generated built-in schema. Definition on an event method such as `light:warn()`
or a callback key such as `onenterred` returns to the source event or state
string from which Tua derived it.

## Configure Events

Each event entry has three fields:

| Field | Meaning |
| --- | --- |
| `name` | Event method added to the machine. |
| `from` | One source state, an array of source states, or `"*"`. |
| `to` | Destination state entered after the event. |

Use an array when several states share one transition:

```tua
local menu = stateMachine({
  initial = "closed",
  events = {
    { name = "open", from = "closed", to = "main" },
    { name = "back", from = { "settings", "credits" }, to = "main" },
    { name = "close", from = "*", to = "closed" },
  },
})
```

`initial` defaults to `"none"` when omitted. Event methods accept optional Lua
payload arguments; every callback receives them after the standard callback
parameters.

## Use Callbacks

Callbacks receive `(self, event, from, to, ...)`. Tua derives these names from
the configuration:

| Callback | Timing |
| --- | --- |
| `onbefore<event>` | Before the event starts. Returning `false` cancels it. |
| `onleave<state>` | Before leaving a state. Returning `false` cancels; returning `self.ASYNC` defers. |
| `onenter<state>` | After changing `current`. Returning `self.ASYNC` defers completion. |
| `onafter<event>` | After the event completes. |
| `on<event>` | Shorthand for `onafter<event>`. |
| `on<state>` | Shorthand for `onenter<state>`. |
| `onstatechange` | After every completed state change. |

```tua
local door = stateMachine({
  initial = "closed",
  events = {
    { name = "open", from = "closed", to = "open" },
  },
  callbacks = {
    onbeforeopen = function(self, event, from, to, actor)
      return actor ~= nil
    end,
    onenteropen = function(self)
      print("door opened", self.current)
    end,
  },
})

local player = { name = "Mina" }
door:open(player)
```

!!! note "Event and state callbacks can share a shorthand name"
    If an event and a state have the same name, prefer the explicit
    `onafter<event>` or `onenter<state>` form so the callback's purpose remains
    clear.

## Machine API

| Member | Behavior |
| --- | --- |
| `current` | Current state, typed as the configured state union. |
| `currentTransitioningEvent` | Event currently waiting on an async callback, or `nil`. |
| `<event>(...)` | Fire a configured event and forward payload values to callbacks. |
| `is(state)` | Test the current state. |
| `can(event)` | Return whether an event can fire and its destination state. |
| `cannot(event)` | Inverse of `can`. |
| `transition(event)` | Resume a transition deferred with `self.ASYNC`. |
| `cancelTransition(event)` | Cancel a deferred transition. |
| `todot(filename)` | Write the transition graph as Graphviz DOT. |

To defer work, return `self.ASYNC` from `onleave<state>` or `onenter<state>`,
then call `transition(event)` when the asynchronous work is complete:

```tua
callbacks = {
  onleavepaused = function(self)
    startFade(function()
      self:transition("resume")
    end)
    return self.ASYNC
  end,
}
```

## Static And Dynamic Configurations

Exact names come from a table literal passed directly to `stateMachine`. If
the configuration is computed or typed as `any`, the call remains valid but
returns the generic `StateMachine` surface with string-based state and event
methods:

```tua
local options: any = loadStateMachineOptions()
local machine = stateMachine(options)
```

!!! info "A gradual boundary, not a runtime restriction"
    Dynamic event construction still works at runtime. Tua does not guess
    names produced by loops, mutation, file loading, or arbitrary function
    calls, so exact suggestions stop at that boundary.

## Runtime And Attribution

`tua build` writes `tuastatemachine.lua` beside generated Lua when emitted code
requires it. A project-provided file with that name is never overwritten, so a
compatible customized runtime can replace the bundled one.

The bundled implementation is adapted from Kyle Conroy's
[lua-state-machine](https://github.com/kyleconroy/lua-state-machine) and keeps
its MIT license header.
