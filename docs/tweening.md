---
icon: lucide/move-right
---

# Tweening

`tween` is a built-in module, like `ecs`. It needs no `require`, package, or
configuration: when a file uses it, Tua emits
`local tween = require("tuatween")` and `tua build` copies the bundled runtime
into the output. The runtime is [flux](https://github.com/rxi/flux) by rxi,
unchanged apart from LuaDoc types, so flux's documentation and examples apply
as written.

A file that declares its own `tween` (a local, parameter, function, or global
assignment) uses that instead, and nothing is injected.

## Animate A Value

`tween.to(target, seconds, values)` moves each numeric field of `target`
toward the value in `values`. Call `tween.update(dt)` once per frame.

```tua
local banner = { x = -200, alpha = 0 }

function love.load()
  tween.to(banner, 0.6, { x = 40, alpha = 1 }):ease("backout")
end

function love.update(dt: number)
  tween.update(dt)
end

function love.draw()
  love.graphics.setColor(1, 1, 1, banner.alpha)
  love.graphics.print("Ready?", banner.x, 40)
end
```

Every field named in `values` must already hold a number on `target`; flux
raises an error at the tween's first update otherwise. Starting a tween on a
field another tween is animating takes that field over from the older tween.

## Chain Settings

Setters return the tween, so they chain:

```tua
local door = { y = -64 }

tween.to(door, 0.4, { y = 0 })
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
| `:after(target, seconds, values)` | Start another tween when this one finishes, and return it. `:after(seconds, values)` reuses the same target. |
| `:stop()` | Remove the tween without completing it. |

Easing names are `linear` plus `quad`, `cubic`, `quart`, `quint`, `expo`,
`sine`, `circ`, `back`, and `elastic`, each with an `in`, `out`, or `inout`
suffix, such as `"elasticout"`. `tween.easing.quadout(0.5)` evaluates one curve
directly.

## Groups

The functions on `tween` share one default group. `tween.group()` creates an
independent one, which is useful when parts of the game update at different
times, for example UI that keeps animating while gameplay is paused:

```tua
local ui = tween.group()
local menu = { y = -300 }
local paused = false

function love.update(dt: number)
  ui:update(dt)
  if not paused then
    tween.update(dt)
  end
end

function openMenu()
  ui:to(menu, 0.3, { y = 0 }):ease("backout")
end
```

`tween.remove(t)` and `t:stop()` both stop a tween early.

## What Tua Checks

Tua types `tween` from the runtime's own LuaDoc, so it checks how you call it:

```tua
tween.to(box, 1, { x = "far" })          -- error: values.x expects number
tween.to(box, 1, { x = 10 }):ease("bounce") -- error: not an easing name
tween.update("fast")                    -- error: dt expects number
```

The first setter after `to(...)` is checked. A tween's type refers to itself,
so the setters return a dynamic value, and later links in the same chain are
accepted without checking. Calling setters on a local, as in `t:delay(0.5)`,
checks each one.

Fields not named in a tween are left alone, and flux never checks that a field
exists before the tween starts. Use typed tables for the objects you animate,
so a misspelled field name is caught where you declare it.

## Replace The Runtime

To change flux's behavior, put your own `tuatween.lua` in the source root.
The build then emits your file and skips the bundled copy. Checking still uses
the bundled annotations, so keep the same API, or require your module under a
different name.
