---
icon: lucide/move-right
---

# Tweening

`Tween` is a built-in module, like `ECS`. It needs no `require`, package, or
configuration: when a file uses it, Tua emits
`local Tween = require("tuatween")` and `tua build` copies the bundled runtime
into the output. The runtime is [flux](https://github.com/rxi/flux) by rxi,
unchanged apart from LuaDoc types, so flux's documentation and examples apply
as written.

The earlier `tween` global continues to work. `Tween` is the preferred module
name; `tween` remains available for existing projects and flux-style examples.

A file that declares its own `Tween` (a local, parameter, function, or global
assignment) uses that instead, and nothing is injected. A global `Tween`
assignment in another project file also prevents injection. Load the defining
file before using that global; prefer explicit local imports when you need
precise cross-file member types.

## A frame-by-frame workflow

Create a tween when an animation starts, advance the appropriate group once per
frame, and stop or replace it when its target is removed. The `Tween` functions
share one default group. A separate `Tween.group()` is useful when, for example,
UI animation should continue while gameplay animation is paused.

## Animate A Value

`Tween.to(target, seconds, values)` moves each numeric field of `target`
toward the value in `values`. Call `Tween.update(dt)` once per frame.

```tua
local banner = { x = -200, alpha = 0 }

function love.load()
  Tween.to(banner, 0.6, { x = 40, alpha = 1 }):ease("backout")
end

function love.update(dt: number)
  Tween.update(dt)
end

function love.draw()
  love.graphics.setColor(1, 1, 1, banner.alpha)
  love.graphics.print("Ready?", banner.x, 40)
end
```

Every field named in `values` must already hold a number on `target`; flux
raises an error at the tween's first update otherwise. Starting a tween on a
field another tween is animating takes that field over from the older tween.
The target is updated in place. Keep its fields stable while the tween runs,
and cancel animations when a pooled or destroyed target must no longer change.

## Chain Settings

Setters return the tween, so they chain:

```tua
local door = { y = -64 }

Tween.to(door, 0.4, { y = 0 })
  :ease("quadinout")
  :delay(0.25)
  :onstart(function() print("opening") end)
  :oncomplete(function() print("open") end)
```

| Method | Effect |
| --- | --- |
| `:ease(name)` | Easing curve. The default is `"quadout"`. |
| `:delay(seconds)` | Wait before starting. |
| `:onstart(fn)` | Call `fn` on the first update after the delay. |
| `:onupdate(fn)` | Call `fn` after every update. |
| `:oncomplete(fn)` | Call `fn` when the tween finishes. Calling it again adds another callback. |
| `:after(target, seconds, values)` | Start another tween when this one finishes, and return it. `:after(seconds, values)` reuses the same target. Each form requires a numeric duration and a table of numeric values. |
| `:stop()` | Remove the tween without completing it. |

Easing names are `linear` plus `quad`, `cubic`, `quart`, `quint`, `expo`,
`sine`, `circ`, `back`, and `elastic`, each with an `in`, `out`, or `inout`
suffix, such as `"elasticout"`. `Tween.easing.quadout(0.5)` evaluates one curve
directly.

## Groups

The functions on `Tween` share one default group. `Tween.group()` creates an
independent one, which is useful when parts of the game update at different
times, for example UI that keeps animating while gameplay is paused:

```tua
local ui = Tween.group()
local menu = { y = -300 }
local paused = false

function love.update(dt: number)
  ui:update(dt)
  if not paused then
    Tween.update(dt)
  end
end

function openMenu()
  ui:to(menu, 0.3, { y = 0 }):ease("backout")
end
```

Update each group once per frame; calling its update twice advances its
animations twice.

`Tween.remove(t)` and `t:stop()` both stop a tween early.

For a runnable pause menu with separate input routing and a native scenario,
see [pause-input](https://github.com/Kyonru/tua-lang/tree/main/examples/pause-input).
Poll input while paused to keep press/release edges current; gate gameplay
updates separately from UI updates.

## Stop an animation when its target is removed

Keep the returned tween when its lifetime depends on a scene or entity. This
fragment assumes the game already calls `Tween.update(dt)` each frame:

```tua
local notification = { alpha = 0 }
local fade = Tween.to(notification, 0.3, { alpha = 1 })

local function closeNotification()
  fade:stop()
  notification.alpha = 0
end
```

Call `closeNotification()` when the notification closes. Stopping leaves the
current field values in place, so the example explicitly hides the target.
It does not run the completion callback. Release any resources in the owner's
cleanup path as well; completion alone cannot handle cancellation.

## What Tua Checks

Tua types `Tween` from the runtime's own LuaDoc, so it checks how you call it:

```tua
Tween.to(box, 1, { x = "far" })          -- error: values.x expects number
Tween.to(box, 1, { x = 10 }):ease("bounce") -- error: not an easing name
Tween.update("fast")                    -- error: dt expects number
```

Every setter in a fluent chain is checked, including later links such as
`Tween.to(box, 1, { x = 10 }):ease("quadout"):delay(0.5)`. The inferred
Tween instance type keeps completion and hover available for setter methods
available after each call.

Fields not named in a tween are left alone. When the tween starts, flux checks
that each target field contains a number; a missing field fails at that point.
Check the keys in `values` against the target record when diagnosing this error.

## Replace The Runtime

To change flux's behavior, put your own `tuatween.lua` in the source root.
The build then emits your file and skips the bundled copy. Checking still uses
the bundled annotations, so keep the same API, or require your module under a
different name.

## Common mistakes

- A missing or nonnumeric target field raises a runtime error when the tween
  starts. Initialize every animated field before creating the tween.
- An unsupported easing name is a type error. Use names such as `"quadout"`
  or `"backinout"` rather than an untyped label.
- Tweens do not advance themselves. Call `Tween.update(dt)` or the owning
  group's `update(dt)` from the game loop.
- `:after(...)` starts a follow-up tween after completion. Use `:delay(seconds)`
  when the current tween should wait before starting.
