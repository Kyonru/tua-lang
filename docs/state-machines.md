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

Start with a literal `initial` state and `events` table. Call the generated
event methods from input or gameplay code, then use callbacks for guarded or
deferred transitions. The static preview shows the declared graph; it does not
replace the machine instance or simulate callback behavior.

| Goal | Start with… |
| --- | --- |
| Declare states and transitions | [Configure events](#configure-events) |
| Guard or defer a transition | [Use callbacks](#use-callbacks) |
| Inspect the declared routes | [Preview the graph](#preview-the-graph) |
| Keep computed configuration | [Static and dynamic configurations](#static-and-dynamic-configurations) |

For a runnable game with title, pause and restart, follow [The playable loop](tutorials/arena-game/playable-loop.md).
It combines transitions with input ownership and explains when to separate UI and gameplay clocks.

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

## Preview The Graph

Enable `tua.preview.enabled`, then run **Tua: Preview State Machine** from a
Tua file, or follow **Preview `<name>` State Machine** directly from the
machine's hover. The compiler-backed preview shows the initial state, all
statically derived states, event routes, multi-source transitions, wildcard
transitions, cycles, and self-transitions. Selecting a state or route opens a
source-linked inspector; double-clicking it jumps to the declaration.

Drag empty canvas space to pan; pinch or use Control/Command + wheel to zoom.
Zoom in, Zoom out, and Fit also have keyboard shortcuts: `+`, `-`, and `0`
when the canvas has focus. Select a route in the inspector to highlight its
edge when transitions overlap.

The visible preview follows edits; a hidden preview catches up when reopened. It
can switch between multiple machines in one
file. It reads the same static model used for checking and completion, so it
does not execute project code or simulate the machine's current runtime state.
Computed configurations remain valid Tua, but cannot produce a graph until
their states and events are visible in a direct table literal.

In the development Runtime Inspector, choose an instance's **Open static
state diagram** action to overlay its last sampled state. The banner identifies
the game session, sample age, and any state missing from the diagram.
The routes still come from source; the overlay does not simulate transitions.
Ordinary previews have no overlay, and packaged games contain no inspector code.

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

Use an array in `from` when one event is valid in several states. Use `"*"`
only when it should be valid from every state. Callback logic can still cancel
or defer a declared transition.

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

### Resume a deferred transition

Return `self.ASYNC` to keep an event pending while an external operation
finishes. Resume that same event when the operation completes; call
`cancelTransition(event)` if the operation is abandoned.

```tua
local door = stateMachine({
  initial = "closed",
  events = {
    { name = "open", from = "closed", to = "open" },
  },
  callbacks = {
    onenteropen = function(self, event)
      playOpeningAnimation(function()
        self:transition(event)
      end)
      return self.ASYNC
    end,
  },
})
```

The transition stays pending until the animation signals completion. If that
animation can be cancelled, cancel the pending transition as part of the same
owner's cleanup.

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

Use `can(event)` to decide whether to offer an action in the current state. The
event method still performs the transition and runs callbacks, so a check does
not replace normal event handling.

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
