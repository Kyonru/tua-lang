---
icon: lucide/move-up-right
---

# Vectors And Game Math

Tua has a built-in `Vec2` type for positions, velocities, and directions, and
a small set of opt-in math helpers for interpolation, clamping, and colors.
Neither needs a `require`: builds include the runtime code only when a file uses
it.

=== "Tua"

    ```tua
    local position = vec2(100, 200)
    local velocity = vec2(60, -30)

    function love.update(dt: number)
      position += velocity * dt
    end

    function love.draw()
      love.graphics.circle("fill", position.x, position.y, 8)
    end
    ```

=== "Generated Lua"

    ```lua
    local __tua_vec2 = require("brinevector")
    local position = __tua_vec2(100, 200)
    local velocity = __tua_vec2(60, -30)

    function love.update(dt)
      position = __tua_vec2(position.x + velocity.x * dt, position.y + velocity.y * dt)
    end

    function love.draw()
      love.graphics.circle("fill", position.x, position.y, 8)
    end
    ```

`Vec2` values are [BrineVector](https://github.com/novemberisms/brinevector)
vectors at runtime. Tua checks their fields, methods, and operators, and
compiles whole vector expressions so they allocate once; see
[Fused Arithmetic](#fused-arithmetic).

## Create Vectors

`vec2(x, y)` creates a vector, with signature
`function vec2(x: number, y: number): Vec2`. Create vectors with `vec2`, not
plain tables: `Vec2` is nominal, so `local p: Vec2 = { x = 1, y = 2 }` is a
type error. A plain table wouldn't have the vector operators at runtime.

A project that declares its own `type Vec2 = {...}` keeps that alias as an
ordinary structural type, and the built-in rules don't apply to it.

## Vec2 API

### Fields And Computed Properties

| Member | Type | Value |
| --- | --- | --- |
| `x`, `y` | `number` | The components. Assignable. |
| `length` | `number` | The vector's length. Assigning it rescales the vector to that length. |
| `length2` | `number` | The squared length, which is cheaper to compare than `length`. |
| `angle` | `number` | The direction in radians, from `math.atan2(y, x)`. Assigning it rotates the vector to that angle, keeping its length. |
| `normalized` | `Vec2` | A new vector of length 1 in the same direction, or `(0, 0)` for a zero vector. |
| `copy` | `Vec2` | A new vector with the same components. |
| `inverse` | `Vec2` | A new vector `(1 / x, 1 / y)`. |
| `floor`, `ceil` | `Vec2` | A new vector with both components rounded down or up. |

Assigning any other field, such as `v.z = 1`, raises an error.

The computed properties run code on every read, and the ones that return a
`Vec2` create a new vector. In a hot loop, read `v.length` once into a local
rather than several times.

### Methods

| Method | Returns |
| --- | --- |
| `v:dot(other)` | The dot product, as a `number`. |
| `v:rotated(angle)` | A new vector rotated by `angle` radians. |
| `v:angled(angle)` | A new vector with `v`'s length pointing at `angle`. |
| `v:trim(maxLength)` | `v` itself if it is shorter than `maxLength`, otherwise a new vector of that length. |
| `v:clamp(min, max)` | A new vector with each component clamped between `min` and `max`, which are both `Vec2`. |
| `v:hadamard(other)` | A new vector multiplied component by component. |
| `v:split()` | The two components, `x, y`, as separate values. |

`getLength`, `getLengthSquared`, `getNormalized`, `getAngle`, `getCopy`,
`getInverse`, `getFloor`, and `getCeil` are method forms of the computed
properties above.

### Operators

| Expression | Result |
| --- | --- |
| `a + b`, `a - b` | Component-wise sum or difference. |
| `-a` | The opposite vector. |
| `a * 2`, `2 * a` | Scaled vector. |
| `a * b` | Component-wise product. |
| `a / 2` | Divided vector. Division by `0` raises an error. |
| `a / b` | Component-wise quotient. A zero component in `b` raises an error. |
| `a == b` | `true` when both components are equal. |
| `tostring(a)` | Text such as `Vector{3.0000, 4.0000}`. |

The compound forms `+=`, `-=`, `*=`, and `/=` work too.

!!! warning "Use `v.length`, not `#v`"
    `#v` returns the length only when BrineVector uses LuaJIT's FFI. With the
    JIT off, as in LÖVE on Apple Silicon by default, vectors are plain tables,
    and LuaJIT ignores `__len` on tables, so `#v` returns `0`. `v.length` gives
    the right answer in both modes. The standard lint rules report `#v` as
    [`TL4060`](linter.md#rules), and the editor offers `v.length` as a quick
    fix.

### FFI And Table Vectors

When BrineVector is first required, it checks whether LuaJIT's JIT compiler is
on. If it is, vectors are FFI structs (`cdata`); otherwise they are plain
tables with a metatable. Both behave the same through the API above, but:

- `type(v)` is `"cdata"` in one mode and `"table"` in the other, so don't use
  `type` to test for a vector.
- Turning the JIT on after the first `require` doesn't switch the mode for that
  run.

## Fused Arithmetic

BrineVector allocates a new vector for every operator, so `p + v * dt` would
normally create two vectors. When Tua knows the operands are `Vec2`, it
compiles the whole expression into one constructor call instead:

```tua
position += velocity * dt
```

```lua
position = __tua_vec2(position.x + velocity.x * dt, position.y + velocity.y * dt)
```

The result is identical, including floating-point rounding, but the expression
allocates once. This matters most with the JIT off: in a benchmark updating
10,000 vectors, `p + v * dt` ran about 2.7 times faster and created half the
garbage. With the JIT on, LuaJIT already removes the temporary vectors, and
timings don't change.

### What Gets Fused

Fusion applies to `+`, `-`, unary `-`, `*` by a number or another `Vec2`, `/` by
a nonzero number literal, and the matching compound assignments. Operands may be
bare names, number expressions made from bare names and literals, or
`vec2(...)` calls with such arguments.

Field and index reads such as `self.pos` or `positions[i]` are also fused when
the `Vec2` expression is the whole right-hand side of an assignment, compound
assignment, `local` declaration, or `return`. Lua may run an `__index`
metamethod on every read, so each read is bound to a local once, in the order
Lua would evaluate it:

```tua
self.pos += self.vel * dt
```

```lua
do local __tua_self_pos = self.pos; local __tua_self_vel = self.vel; self.pos = __tua_vec2(__tua_self_pos.x + __tua_self_vel.x * dt, __tua_self_pos.y + __tua_self_vel.y * dt) end
```

These keep BrineVector's ordinary operators instead:

- an assignment target deeper than one field or key, such as
  `world.player.pos`
- a `local` whose right-hand side mentions its own name
- a `Vec2` expression nested inside a larger one, such as a call argument
- expressions containing a function call, division by a variable, or a comment
  inside the expression (comments before or after it are fine)

### Trade-Offs

The generated locals have two visible effects:

- Runtime errors name the generated local. If `self.pos` is unexpectedly `nil`,
  Lua reports an attempt to index local `__tua_self_pos`; the name identifies
  the field that supplied the bad value.
- Every field and index operand is read before the arithmetic starts. If an
  earlier BrineVector operation would have failed, getters for later operands
  have already run. This matters only when those reads have side effects
  through `__index`.

During project builds, fusion uses project-wide type information. A file
doesn't need to mention `vec2` or `Vec2` itself: vectors that come from a
required Tua module, or from an annotated Lua module, are fused when their type
resolves to the built-in `Vec2`.

## Game Math Helpers

`math` and `love.math` both offer the same small set of game-math helpers:

```tua
local angle: number = math.tau
local health: number = math.clamp(rawHealth, 0, 100)
local x: number = love.math.lerp(startX, targetX, amount)
local orange = love.math.rgb(255, 128, 0)
local faded = love.math.rgba(40, 80, 255, 128)
```

| Helper | Returns |
| --- | --- |
| `tau` | `2 * math.pi`, a full turn in radians. |
| `clamp(value, minimum, maximum)` | `value` limited to the inclusive range. |
| `lerp(from, to, amount)` | The value `amount` of the way from `from` to `to`. `amount` isn't clamped, so `1.5` overshoots. |
| `inverseLerp(from, to, value)` | How far `value` is between `from` and `to`, as an unclamped amount. |
| `remap(value, inMin, inMax, outMin, outMax)` | `value` mapped from one range to another, unclamped. |
| `sign(value)` | `-1`, `0`, or `1`. |
| `round(value)` | The nearest integer, with halves rounded away from zero. |
| `smoothstep(edge0, edge1, value)` | A smooth 0-to-1 curve between the edges, clamped outside them. |
| `rgb(red, green, blue)` | A `{ r, g, b }` color with 0–255 components converted to LÖVE's 0–1 range. |
| `rgba(red, green, blue, alpha)` | The same, with alpha. |

```tua
-- Fade a title in over its first second on screen.
local alpha = math.smoothstep(0, 1, elapsed)

-- Map a health value onto a 120-pixel bar.
local barWidth = math.remap(health, 0, maxHealth, 0, 120)

-- Pass a color straight to LÖVE.
love.graphics.setColor(love.math.rgb(255, 128, 0))
```

`rgb` and `rgba` return a new table on each call. For a color you use every
frame, create it once outside `love.draw` and reuse it.

The helpers cost nothing unless used. A file that references one gets a small
guarded Lua definition for just that helper, so ordinary `math` code produces
no extra output, and a `math` field your project already defines with the same
name is kept.

## Runtime And Overrides

`vec2(...)` lowers to a cached `require("brinevector")` constructor, and builds
write the bundled `brinevector.lua` only when a file uses vectors. A
`brinevector.lua` already in your project is never overwritten.
