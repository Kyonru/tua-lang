---
icon: lucide/video
---

# Camera

`Camera` is a built-in module, like `ecs`. It needs no `require` or
configuration: when a file uses it, Tua emits
`local Camera = require("tuacamera")` and `tua build` copies the bundled
runtime into the output. A file that declares its own `Camera` uses that
instead.

!!! note "A starting point, not a framework"
    The camera covers the common cases so a game can start moving quickly:
    following through a deadzone, smoothing, look-ahead, world bounds, zoom,
    rotation, shake, split screen, and coordinate conversion. Games with more
    specific needs, such as multiple targets, cinematic paths, or
    parallax-aware culling, should write their own camera. Copy
    `tuacamera.lua` from a build as a starting point, keep it in your source
    root under that name to replace the bundled one, or give it another name
    and `require` it.

## Follow A Player

```tua
local player = { x = 0, y = 0 }
local cam = Camera.new({ style = "platformer", smoothing = 8 })

function love.update(dt: number)
  -- move the player first, then the camera
  cam:follow(player.x, player.y)
  cam:update(dt)
end

function love.draw()
  cam:draw(function()
    love.graphics.rectangle("fill", player.x - 8, player.y - 16, 16, 32)
  end)
  -- anything drawn here, outside cam:draw, stays in screen space (HUD)
end
```

`cam:draw(fn)` is `cam:attach()`, `fn()`, and `cam:detach()`. It detaches even
when `fn` raises an error. Use `attach`/`detach` directly when that reads
better.

## Options

Every option is optional.

| Option | Default | Meaning |
| --- | --- | --- |
| `x`, `y` | `0` | World point at the center of the view. |
| `zoom` | `1` | Scale factor; `2` draws the world twice as large. |
| `rotation` | `0` | View rotation in radians. |
| `style` | `"lockon"` | Deadzone preset (see below). |
| `smoothing` | `0` | Catch-up rate per second; `0` snaps. Around `5` to `10` feels smooth and is frame-rate independent. |
| `lead` | `0` | Seconds of the target's movement to look ahead. |
| `bounds` | none | `{ x, y, w, h }` world area the view must stay inside. A world smaller than the view is centered. |
| `viewport` | whole window | `{ x, y, w, h }` screen area to draw into, for split screen or a minimap. Drawing is clipped to it. |
| `pixelPerfect` | `false` | Round the view to whole screen pixels, which keeps pixel art from shimmering. |
| `shakeAmount` | `12` | Largest shake offset in screen pixels. |
| `shakeDecay` | `1.5` | Shake trauma removed per second. |

The same settings are available as methods: `setStyle`, `setDeadzone`,
`setBounds`/`clearBounds`, and `setViewport`/`clearViewport`. `x`, `y`, `zoom`,
`rotation`, `smoothing`, `lead`, and `pixelPerfect` are plain fields.

## Follow Styles

The deadzone is the area around the view's center where the target can move
without moving the camera.

| Style | Behavior |
| --- | --- |
| `lockon` | No deadzone: the target stays exactly centered. |
| `platformer` | Wide horizontally and short vertically, slightly above center, so small hops don't move the view. |
| `topdown` | A square a quarter of the shorter screen side. |
| `topdowntight` | A square an eighth of the shorter screen side. |
| `screen` | Moves one whole screen at a time, like room-based games. Rooms start at `bounds`' origin, or `0, 0`. |

`cam:setDeadzone(x, y, w, h)` replaces the preset with your own rectangle, in
screen pixels relative to the view's top left. Combine `screen` with
`smoothing` to slide between rooms instead of cutting.

## Shake

`cam:shake(trauma)` adds trauma from 0 to 1. The offset grows with trauma
squared, so small hits stay subtle and big ones stack up, and trauma fades by
`shakeDecay` per second. Shake moves only the drawn view; `cam.x` and `cam.y`
stay on the followed point.

```tua
function onPlayerHit()
  cam:shake(0.4)
end
```

## Zoom And Rotation With Tweens

`zoom` and `rotation` are plain numbers, so the [tween](tweening.md) add-on can
animate them:

```tua
tween.to(cam, 0.4, { zoom = 2 }):ease("quadout")
```

## Screen And World Coordinates

```tua
local worldX, worldY = cam:mousePosition()
local screenX, screenY = cam:toScreen(enemy.x, enemy.y)
local left, top, width, height = cam:visibleBounds()
```

`toWorld` and `toScreen` convert in either direction, taking zoom, rotation,
shake, and the viewport into account. `visibleBounds` returns the world
rectangle in view, enlarged to cover a rotated view, which is useful for
skipping offscreen drawing.

## Split Screen

Give each player a camera with its own viewport:

```tua
local w, h = love.graphics.getDimensions()
local left = Camera.new({ viewport = { x = 0, y = 0, w = w / 2, h = h } })
local right = Camera.new({ viewport = { x = w / 2, y = 0, w = w / 2, h = h } })

function love.draw()
  left:draw(drawWorld)
  right:draw(drawWorld)
end
```

## What Tua Checks

Tua types `Camera` from the runtime's LuaDoc, so option types, style names, and method arguments
are checked:

```tua
Camera.new({ style = "orbit" })            -- error: not a camera style
Camera.new({ bounds = { x = 0, y = 0 } })  -- error: bounds needs w and h
cam:shake("hard")                          -- error: trauma expects number
```

Multiple results, such as `toWorld`'s second number, are currently typed as
`any`.
