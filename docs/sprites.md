---
icon: lucide/film
---

# Sprites And Animation

`spriteSheet({...})` describes a sprite image, its frame grid, and its named
animations. Tua checks the configuration while you type and compiles it to the
bundled [anim8](https://github.com/kikito/anim8) runtime. Each sheet creates
lightweight players that track their own animation, frame, and timing.

```tua
local Hero = spriteSheet({
  image = "assets/hero.png",
  frame = { width = 32, height = 32 },
  origin = { x = 16, y = 32 },
  animations = {
    idle = { from = 0, to = 3, row = 0, fps = 6 },
    run = { from = 0, to = 7, row = 1, fps = 12 },
  },
})

local hero = Hero:player("idle")

function love.update(dt: number)
  if love.keyboard.isDown("right") then
    hero:play("run")
  else
    hero:play("idle")
  end
  hero:update(dt)
end

function love.draw()
  hero:draw(100, 200)
end
```

Animation names are checked: `hero:play("jump")` is a type error until the
sheet declares `jump`, and completion only offers `idle` and `run`. Calling
`play` with the animation that is already playing keeps its place, so calling
it every frame is fine.

## Declare A Sheet

A sheet names an image, a default frame size, and its animations. Grid rows,
columns, and marker frames count from **0** in the configuration; Tua converts
them to anim8's one-based selectors when it compiles:

```tua
local background = spriteSheet({
  image = "assets/images/spritesheet_stall.png",
  frame = { width = 256, height = 256, left = 2, top = 4, border = 1 },
  origin = { x = 128, y = 256 },
  animations = {
    orange = { column = 1, row = 0, durations = 1 },
    blue = { from = 0, to = 3, row = 1, fps = 8, onLoop = "pauseAtEnd" },
    green = {
      frames = {
        { from = 0, to = 3, row = 2 },
        { from = 0, to = 1, row = 3 },
      },
      frame = { width = 128, height = 64, left = 8, top = 16, border = 2 },
      origin = { x = 64, y = 64 },
      durations = { 0.1, 0.5, 0.1, 0.1, 0.1, 0.1 },
      markers = {
        { frame = 1, name = "step" },
      },
      onLoop = function(animation, loops)
        if loops > 1 then
          animation:pause()
        end
      end,
    },
  },
})
```

### Frame Geometry

| Field | Meaning |
| --- | --- |
| `frame.width`, `frame.height` | The default grid cell size, in pixels. |
| `frame.left`, `frame.top` | Offset of the grid from the image's top-left corner. |
| `frame.border` | Spacing between adjacent cells. |
| `origin.x`, `origin.y` | The registration point: the pixel inside a frame that lands on the position you draw at. |

An animation may override `frame` when part of the image uses a different cell
size, and override `origin` when it needs a different registration point.

### Choosing Frames

| Selector | Picks |
| --- | --- |
| `column = 1` | One cell. |
| `from = 0, to = 3` | An inclusive column range. |
| `row = 2` | The row for `column` or `from`/`to`; defaults to `0`. |
| `frames = { {...}, {...} }` | Ordered ranges, for animations that span rows. |

`frames` can't be combined with the top-level `column`, `from`, `to`, or `row`
selectors.

### Timing And Looping

- `fps = 8` is shorthand for a uniform duration of `1 / 8` seconds.
- `durations` accepts what `anim8.newAnimation` accepts: one number for every
  frame, a list such as `{ 0.1, 0.5, 0.1 }`, or ranges such as
  `{ ['3-5'] = 0.2 }`.
- `onLoop` runs when the animation wraps. It accepts the name of an animation
  method, such as `"pauseAtEnd"`, or a function receiving
  `(animation, loops)`. The older `loop = false` is shorthand for
  `onLoop = "pauseAtEnd"` and can't be combined with `onLoop`.

Tua rejects conflicting selector, timing, marker, and loop settings before it
emits anything.

## Players

`sheet:player(name)` creates a player and starts it on `name`; with no name it
starts on the sheet's first animation. Create one player per on-screen sprite.
Each player keeps its own animation, frame, and speed.

| Method | Behavior |
| --- | --- |
| `play(name, restart?)` | Play `name`. If it is already the current animation, it keeps its place unless `restart` is `true`. |
| `stop()` | Pause on the current frame. |
| `resume()` | Continue after `stop`. |
| `gotoAndPlay(name, frame?)` | Switch to `name`, jump to `frame` (default `1`), and play. |
| `gotoAndStop(name, frame?)` | Switch to `name`, jump to `frame` (default `1`), and pause there. |
| `seekFrame(frame, emitMarkers?)` | Jump to `frame` in the current animation. |
| `seek(progress, emitMarkers?)` | Jump to a fraction of the current animation, from `0` (first frame) to `1` (last). |
| `update(dt)` | Advance playback by `dt` seconds, scaled by `speed`, and update nested clips. |
| `draw(x, y, rotation?, scaleX?, scaleY?, originX?, originY?, shearX?, shearY?)` | Draw the current frame. `scaleY` defaults to `scaleX`; the origin defaults to the configured registration point. |
| `addToBatch(batch, x, y, ...)` | Add the current frame to a `SpriteBatch` with the same arguments as `draw`; see [Batching](#batch-many-sprites). |
| `on(marker, callback)` | Run `callback(player, marker, animation, frame)` when playback reaches `marker`. |
| `off(marker)` | Remove every callback for `marker`. |
| `child(name)` | Return a nested clip's player; see [Nested Clips](#nested-clips). |

| Field | Meaning |
| --- | --- |
| `name` | The current animation's name, or `nil`. |
| `playing` | `true` while the player advances on `update`. |
| `speed` | Playback rate: `1` is normal, `2` is double speed, `0` freezes, and negative values play backward. |

!!! note "Player frame numbers start at 1"
    The configuration counts rows, columns, and marker frames from 0. The
    player's frame arguments (`gotoAndPlay`, `gotoAndStop`, `seekFrame`) and
    the `frame` passed to marker callbacks count from **1**, like anim8.
    Out-of-range frames are clamped to the animation.

### Speed And Manual Seeking

Use `speed` for slow motion, hit-stop, or a per-enemy tempo:

```tua
hero.speed = 0.5 -- half speed
hero.speed = 0 -- hold the current frame without stopping
```

`seek` lets game state drive the pose instead of elapsed time. For example,
tie a run cycle to the distance travelled so feet don't slide:

```tua
local strideLength = 48
local distance = 0

function love.update(dt: number)
  local speedX = 120
  distance += speedX * dt
  hero:seek((distance % strideLength) / strideLength)
end
```

## Markers

`markers` name a frame of an animation, using the configuration's 0-based
frame index. Marker names are checked and completed in `on` and `off`:

```tua
hero:on("step", function(player, marker, animation, frame)
  footstep:play()
end)
```

Normal playback fires every marker it crosses, including across loops and
during one long update, so a slow frame doesn't skip footsteps. `seek` and
`seekFrame` are silent unless you pass `true` as `emitMarkers`.

## Trimmed Frames And Metadata

Texture packers often trim empty pixels from each frame. Optional `metadata`
keeps one record per frame so trimmed frames still line up:

```tua
metadata = {
  { origin = { x = 64, y = 64 }, trimX = 0, trimY = 0, sourceWidth = 128, sourceHeight = 64 },
  { origin = { x = 66, y = 62 }, trimX = 2, trimY = 3, sourceWidth = 132, sourceHeight = 68 },
}
```

- `origin` is the frame's registration point in the untrimmed frame.
- `trimX` and `trimY` place the trimmed rectangle inside the untrimmed frame.
- `sourceWidth` and `sourceHeight` record the untrimmed size.

Players subtract the trim from the draw origin automatically, and the VS Code
preview rebuilds the untrimmed canvas. When `metadata` is present, it must have
exactly one entry per frame of the animation.

## Nested Clips

A sheet can include other sheets as named child clips, forming a small display
tree, such as a body with a hat:

```tua
-- Hat is another spriteSheet declared above.
local Body = spriteSheet({
  image = "assets/player.png",
  frame = { width = 32, height = 32 },
  animations = { idle = { column = 0, fps = 1 } },
  children = {
    hat = { sheet = Hat, initial = "idle", x = 4, y = -8 },
  },
})

local body = Body:player("idle")
body:child("hat"):gotoAndPlay("idle", 1)
```

Each child sets `sheet`, an optional `initial` animation, and an optional
local `x`, `y`, `rotation`, `scaleX`, and `scaleY`. Child names are checked and
completed. Children update with their parent and draw inside the parent's
translation, rotation, and scale. Use nested clips for attachments that move
together; keep game logic in ordinary code.

## Batch Many Sprites

Drawing many sprites that share one image is faster through a LÖVE
`SpriteBatch`, which sends them to the GPU together. Add each player's current
frame yourself, so draw order stays under your control:

```tua
type EnemyDraw = { player: SpritePlayer, x: number, y: number }
local enemies: EnemyDraw[] = {}
local enemyBatch = spriteBatch(Enemy.image, 1000)

function love.draw()
  enemyBatch:clear()
  for _, enemy in ipairs(enemies) do
    enemy.player:addToBatch(enemyBatch, enemy.x, enemy.y)
  end
  love.graphics.draw(enemyBatch)
end
```

`addToBatch` uses the current frame, registration point, trim metadata, flip
state, and transform, and returns the batch entry index, or `nil` when no
animation is selected.

- The batch must be created from the sheet's image (`Enemy.image` above).
- A player with nested clips raises an error, because a child may use another
  image. Draw it normally, or batch each leaf player separately.

## Sharing And Performance

Creating players is cheap because every player shares its sheet's frame,
duration, and interval arrays. Only the playback state (timer, current frame,
status, and flips) belongs to each player, so spawning hundreds of animated
enemies doesn't copy animation data.

Treat those shared arrays as read-only. Editing a player's animation arrays in
place would change every player of that sheet:

- Use `player.speed` to change one player's timing.
- To change an animation's durations, build a new one with
  `anim8.newAnimation`. anim8 only computes frame intervals when an animation
  is created, so editing `durations` afterward would leave them inconsistent.
- If one player really needs its own copy, clone the animation and restart it.
  A player keeps its current animation until it selects one again, so the
  restart is what makes it use the copy:

  ```lua
  player.animations.run = player.animations.run:clone()
  player:play("run", true)
  ```

A player draws one image per call. For large crowds, [batch](#batch-many-sprites)
leaf players that share an image.

## Editor Support

- Completion inside the configuration suggests sheet, frame, animation, range,
  origin, metadata, and marker fields, inserts their value shapes, and offers
  common `onLoop` method names.
- Hover explains each field, including the `loops` parameter of an `onLoop`
  callback. Fixed fields navigate to their schema definitions.
- `background.animations.green` is typed as `SpriteAnimation`, completes known
  animation names, and navigates to the `green` entry in the configuration.
- The VS Code preview renders the sheet's frames and animations.

## Runtime And Overrides

Builds write the bundled `anim8.lua` and `tuasprite.lua` only when a file uses
`spriteSheet`, and never overwrite copies that already exist in your project.
If you replace `anim8.lua`, players use that runtime's public `clone()` for
each animation, which copies the arrays instead of sharing them.
