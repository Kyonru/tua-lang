---
icon: lucide/gamepad-2
---

# Input

`input({...})` creates a typed input player backed by
[Baton](https://github.com/tesselode/baton). It keeps Baton's polling model and
runtime behavior while Tua adds completion, checking, name-aware return types,
and zero-setup build integration.

=== "Tua"

    ```tua
    local controls = input({
      controls = {
        left = { "key:left", "key:a", "axis:leftx-" },
        right = { "key:right", "key:d", "axis:leftx+" },
        up = { "key:up", "key:w", "axis:lefty-" },
        down = { "key:down", "key:s", "axis:lefty+" },
        confirm = { "key:return", "button:a" },
      },
      pairs = {
        move = { "left", "right", "up", "down" },
      },
      deadzone = 0.25,
    })

    function love.update(dt: number)
      controls:update()

      local x, y = controls:get("move")
      if controls:pressed("confirm") then
        print("confirmed", x, y)
      end
    end
    ```

=== "Generated Lua"

    ```lua
    local __tua_input = require("baton").new
    local controls = __tua_input({
      controls = {
        left = { "key:left", "key:a", "axis:leftx-" },
        right = { "key:right", "key:d", "axis:leftx+" },
        up = { "key:up", "key:w", "axis:lefty-" },
        down = { "key:down", "key:s", "axis:lefty+" },
        confirm = { "key:return", "button:a" },
      },
      pairs = {
        move = { "left", "right", "up", "down" },
      },
      deadzone = 0.25,
    })

    function love.update(dt)
      controls:update()

      local x, y = controls:get("move")
      if controls:pressed("confirm") then
        print("confirmed", x, y)
      end
    end
    ```

## Controls And Pairs

Every key under `controls` becomes an exact input name. A control combines one
or more sources and returns a scalar value between 0 and 1:

```tua
local amount: number = controls:get("confirm")
```

Every key under `pairs` becomes an exact directional-pair name. The four
control names must be ordered left, right, up, down. A pair returns two values:

```tua
local x, y = controls:get("move") -- both inferred as number
```

Tua completes these names in pair declarations and in `get`, `getRaw`, `down`,
`pressed`, and `released` calls. Unknown static names and pair references are
reported before the game runs.

!!! warning "Pair order is significant"
    Baton calculates x and y from `{ left, right, up, down }`. Tua validates
    that exactly four known controls are present but does not reinterpret their
    order.

## Source Strings

Sources use Baton's `kind:value` format. Tua validates literal sources and
completes values from the configured LOVE 11.x API catalog.

| Source | Meaning | Example |
| --- | --- | --- |
| `key:<KeyConstant>` | Keyboard key | `"key:return"` |
| `sc:<Scancode>` | Layout-independent keyboard scancode | `"sc:w"` |
| `mouse:<button>` | Numeric mouse button | `"mouse:1"` |
| `axis:<axis><+|->` | Gamepad axis or numeric joystick axis direction | `"axis:leftx-"` |
| `button:<button>` | Gamepad button or numeric joystick button | `"button:a"` |
| `hat:<hat><direction>` | Numeric joystick hat and LOVE hat direction | `"hat:1u"` |

Keyboard and mouse sources work without a joystick. Axis, button, and hat
sources use the optional LOVE `Joystick` assigned in the configuration:

```tua
local joysticks = love.joystick.getJoysticks()

local controls = input({
  controls = {
    jump = { "key:space", "button:a" },
  },
  joystick = joysticks[1],
})
```

## Runtime API

| Method | Result |
| --- | --- |
| `update()` | Poll all sources and update frame state. |
| `get(name)` | Scalar control value or pair x and y after the deadzone. |
| `getRaw(name)` | Scalar control value or pair x and y before the deadzone. |
| `down(name)` | Whether the control or pair is held. |
| `pressed(name)` | Whether it became held during this update. |
| `released(name)` | Whether it stopped being held during this update. |
| `getActiveDevice()` | `"kbm"`, `"joy"`, or `"none"`. |

Call `update()` once near the start of each `love.update`. Separate `input`
instances are useful for local multiplayer or for keeping gameplay and menu
controls independent.

`deadzone` defaults to `0.5`. `squareDeadzone = true` applies the threshold to
each pair axis independently; the default radial deadzone uses the pair's
length.

## Builds And Overrides

`input({...})` lowers to an ordinary Baton constructor:

```lua
local __tua_input = require("baton").new
local controls = __tua_input({
  -- unchanged configuration
})
```

`tua build` writes the bundled MIT-licensed `baton.lua` only when generated Lua
requires it. A project-provided `src/baton.lua` is copied instead and is never
overwritten.

An explicit `input` binding also disables the helper for the remainder of that
file:

```tua
local input = require("project_input")
local controls = input({ controls = {} })
```

!!! info "Static configuration provides exact names"
    A table literal passed directly to `input` derives exact control names,
    pair names, completion, and scalar-versus-pair returns. Computed
    configurations remain valid through the generic `Input` type, but Tua does
    not execute Lua code to discover dynamic names.
