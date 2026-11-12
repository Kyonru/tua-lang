---
icon: lucide/boxes
---

# Procedural game helpers

Tua includes an optional, dependency-free `Rot` module for game generation and
grid algorithms. Tua infers its public types from the runtime annotations, and
the generated `tuarot.lua` is copied into a build only when the project uses
`Rot`.

```tua
local rng = Rot.Rng.new(817)
local maze = Rot.Map.generate(31, 21, "sidewinder", rng)
local route = Rot.Path.astar(maze, 2, 2, 30, 20)
local visible = Rot.FOV.compute(31, 21, 2, 2, 8, function(x, y)
  return not maze:get(x, y)
end)
```

## Random streams and dice

Create a seeded stream with `Rot.Rng.new(seed)`. Use `next`, `int`, `float`,
`choice`, and `shuffle` to keep procedural output repeatable. The helper does
not call or modify LOVE's global random generator. Pass the stream explicitly
to other random operations:

```tua
local rng = Rot.Rng.new(817)
local total, rolls = Rot.Dice.roll("3d6+2", rng)
local nextItem = rng:choice({ "potion", "scroll", "key" })
```

Dice expressions support `dM`, `NdM`, and an optional signed integer bonus.
Each die count is limited to 1,000 and the side count to one billion.

## Colors

`Rot.Color.parse` accepts `#RGB`, `#RRGGBB`, `#RRGGBBAA`, and a small set of
basic names (`black`, `white`, `red`, `green`, `blue`, `yellow`, `cyan`,
`magenta`, `orange`, `purple`, `gray`/`grey`, and `transparent`). Colors are
tables with normalized `r`, `g`, `b`, and `a` values. `mix`, `add`, and
`multiply` return new values. For LOVE drawing, pass those four channels to
`love.graphics.setColor`.

## Maps and paths

`Rot.Map.arena(width, height)` returns a grid of `false` cells. `Map.generate`
supports `"arena"`, `"binaryTree"`, and `"sidewinder"`; maze dimensions must be
at least 3 by 3, with at most 250,000 cells. Maze algorithms preserve a closed
outer border and mark walkable cells `true`. Grids expose `width`, `height`,
`grid:get(x, y)`, and `grid:set(x, y, walkable)`; coordinates start at 1 and
out-of-bounds reads return `false`. Use the same seeded RNG to reproduce a map.

`Rot.Path.astar` and `Rot.Path.dijkstra` search four-way through walkable grid
cells. They return an ordered array of `{ x, y }` points from start to goal, or
`nil` when no path exists. An optional `passable(cell, x, y)` callback can adapt
the search to domain-specific grid data. Current paths use unit movement costs;
Dijkstra is useful when you want an uninformed search or a baseline to compare
against A-star.

### Preview a seeded map in the editor

Run **Tua: Preview Seeded Map and Route** from the VS Code Command Palette (or
the Tua editor context menu). The preview uses a small, isolated Tua project and
your configured LÖVE executable to generate a sample with the bundled `Rot`
runtime. Change the algorithm, odd map dimensions, seed, or sight radius and
select **Generate**. The grid marks walls, the route from `(2, 2)` to the
opposite interior corner, and cells visible from the start. Select a cell or
use arrow keys to read its coordinates and state. The sample does not execute
your game or modify its build. Workspace trust and experimental previews must
be enabled; the configured LÖVE executable must be available.

This view is a way to inspect the helper algorithms. It is not a preview of
your game's own map, path callback, or current runtime state.

## Visibility, noise, grammars, and lights

- `Rot.FOV.compute(width, height, x, y, radius, opaque)` returns a visibility
  grid with the same `get`/`set` accessors. It uses bounded Bresenham
  line-of-sight rays. Opaque destination cells remain visible; cells behind
  them are hidden. Radius is limited to 64 and the grid to 250,000 cells.
- `Rot.Noise.new(seed)` returns a seeded 2D simplex noise generator. Call
  `noise:sample(x, y)` for deterministic values in `[-1, 1]`.
- `Rot.StringGenerator.generate(grammar, start, rng, maxDepth?)` expands
  `{rule}` references from strings or arrays of alternatives. The default
  recursion limit is 32; the supported limit is 256.
- `Rot.Lighting.compute(width, height, lights, opaque?)` combines radial point
  lights into a color grid. Read normalized channels through
  `light:get(x, y)`. A supplied opacity callback blocks light behind walls.
  The grid is limited to 65,536 cells, each light radius to 64, and
  line-of-sight mode to 16 lights with radius at most 32.

## Deliberate scope

This is a Tua-owned, LOVE-friendly utility module informed by the feature areas
in [rotLove](https://github.com/paulofmandown/rotLove). It implements seeded
random streams, dice, colors, two perfect-maze algorithms, grid pathfinding,
line-of-sight visibility, simplex noise, grammar expansion, and radial
lighting. It does not copy rotLove source code.

The current API does not include rotLove's CP437 display, text display,
schedulers/event queue, its other dungeon generators, multiple RNG algorithms,
147-color palette, or precise-shadowcasting FOV. Prefer LOVE's own font and
graphics APIs for rendering; Tua's state-machine and tween modules cover common
time-based behavior. These omitted features should be added as separate,
tested APIs when game projects need them.
