# Entity Component Systems

Tua includes a small entity component system through the built-in `ecs`
module. It needs no `require`, package installation, or project configuration.
When a build uses `ecs`, Tua emits a bundled `tuaecs.lua` runtime beside the
generated Lua.

An ECS is useful when a game has many objects that share combinations of data
and are updated by the same loops. It replaces a deep object hierarchy with
three simple ideas:

- **Entity**: a numeric ID for one game object.
- **Component**: one kind of data, such as position, velocity, health, or a
  sprite.
- **System**: behavior applied to every entity with a required set of
  components.

## Start With One Moving Entity

```tua
local Position = ecs.component("Position", {
  x = "number",
  y = "number",
})

local Velocity = ecs.component("Velocity", {
  x = "number",
  y = "number",
})

local world = ecs.world()
local player = world:create()

world:add(player, Position, { x = 120, y = 80 })
world:add(player, Velocity, { x = 30, y = 0 })

local movement = ecs.system(world, { Position, Velocity }, function(_, position, velocity, dt)
  position.x += velocity.x * dt
  position.y += velocity.y * dt
end)

function love.update(dt: number)
  movement:update(dt)
end
```

The component order in `ecs.system(world, { Position, Velocity }, callback)`
is also the callback data order. The callback receives the entity ID first and
`dt` last.

Tua completes the built-in module and its methods, shows named signatures and
short API documentation, checks static component values, and reports unknown
components in queries. Typing `ecs component` also offers a complete starter
snippet.

## ECS Explorer

Run **Tua: Show ECS Explorer** from VS Code to inspect the current project's
static ECS structure. The compiler-backed view lists components, systems, and
standalone or system-owned queries without reparsing source in the extension.
Selecting a component shows its schema, every resolved system access, and every
relevant query; each relationship opens the exact component reference.

System access is inferred conservatively from callback parameters:

- `filter` means the component participates in matching but its callback value
  is not used;
- `read` and `write` are proven references and assignments;
- `read + write` includes compound assignments such as `position.x += dx`;
- `unknown mutation` means a method was called on the component value and Tua
  cannot prove whether that method mutates it.

Module-qualified references such as `Components.Position` resolve through the
same project definition index as go-to-definition. Dynamic component lists stay
unresolved rather than being guessed.

The Insights tab reports conservative performance observations, not compiler
errors. It currently highlights queries wider than three components because the
bundled runtime allocates a temporary values table per match, table or closure
allocations inside per-entity system callbacks, and LOVE resource construction
inside those callbacks. These observations are prompts to profile; they do not
change diagnostics or builds.

## When ECS Helps

Use ECS when most of these are true:

- the game updates many bullets, particles, enemies, pickups, or effects;
- objects gain and lose capabilities while the game runs;
- several systems need different views of the same population;
- hot loops should iterate compact component collections instead of unrelated
  object fields;
- composition is clearer than adding more subclasses.

The bundled runtime stores values per component and starts each query from its
smallest component store. This can reduce irrelevant work and object overhead,
but ECS is not automatically faster. Measure representative update loops and
avoid changing architecture solely for a theoretical gain.

## When Ordinary Tua Is Better

Prefer tables, modules, or classes when:

- there are only a few long-lived objects;
- one object owns complex behavior and does not share it with a population;
- the code is UI, menus, configuration, scene control, or a singleton service;
- following an entity across many systems would make the feature harder to
  understand;
- a direct function call describes the behavior more clearly.

Mixing both styles is normal. A scene can be an ordinary Tua module while its
projectiles and enemies live in an ECS world.

## Worlds And Queries

`ecs.world()` owns all entities and component values:

```tua
local enemy = world:create()
world:add(enemy, Position, { x = 20, y = 40 })

local position = world:get(enemy, Position)
local isPositioned = world:has(enemy, Position)

world:remove(enemy, Position)
world:destroy(enemy)
```

A query can be reused without defining a system:

```tua
local visible = world:query({ Position, Sprite })

visible:each(function(entity, position, sprite)
  love.graphics.draw(sprite.image, position.x, position.y)
end)

print(visible:count())
```

Use `world:clear()` when leaving a scene and discarding every entity.

## Runtime Overrides

The built-in runtime is the default profile:

```toml
[ecs]
enabled = true
backend = "tua"
```

The section can be omitted because these are the defaults. Set `enabled =
false` to remove the typed semantic surface and automatic runtime injection.
Existing projects can bind a local `ecs` module explicitly; Tua preserves that
binding and does not inject the bundled runtime for the file:

```tua
local ecs = require("project_ecs")
```

The current component schema diagnostics target clear static calls. Dynamic
component construction remains permissive.
