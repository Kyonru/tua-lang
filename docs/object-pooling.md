---
icon: lucide/package
---

# Object Pooling

Tua includes a small object pool through the built-in `objectPool` helper. It
needs no `require`, package installation, or configuration. When a build uses
the helper, Tua emits `require("tuaobjectpool")` and writes the bundled runtime
beside the generated Lua.

An object pool creates a group of reusable objects ahead of time or on demand.
Instead of allocating and abandoning a new table for every bullet, particle,
damage number, or temporary effect, the game acquires an object, uses it, and
releases it for later reuse.

## Create A Typed Pool

=== "Tua"

    ```tua
    type Bullet = {
      x: number,
      y: number,
      active: boolean,
    }

    local bullets = objectPool({
      create = function(): Bullet
        return { x = 0, y = 0, active = false }
      end,
      reset = function(bullet: Bullet)
        bullet.x = 0
        bullet.y = 0
        bullet.active = false
      end,
      initial = 128,
    })

    local bullet = bullets:acquire()
    bullet.x = 120
    bullet.y = 80
    bullet.active = true

    bullets:release(bullet)
    ```

=== "Generated Lua"

    ```lua
    local __tua_object_pool = require("tuaobjectpool")
    local bullets = __tua_object_pool({
      create = function()
        return { x = 0, y = 0, active = false }
      end,
      reset = function(bullet)
        bullet.x = 0
        bullet.y = 0
        bullet.active = false
      end,
      initial = 128,
    })

    local bullet = bullets:acquire()
    bullet.x = 120
    bullet.y = 80
    bullet.active = true

    bullets:release(bullet)
    ```

!!! tip "Annotate the factory return"
    A return annotation such as `Bullet` lets completion, hover, checking,
    signatures, and inlay hints carry the item type through
    `bullets:acquire()`. An unannotated factory remains valid and permissive,
    but its acquired values have type `any`.

The `reset` hook runs before a released object becomes available again. Keep it
small and deterministic. If resetting fails, the object remains active instead
of entering the available stack in a partially reset state.

## Lifecycle API

Pools expose a deliberately small API:

| Method | Behavior |
| --- | --- |
| `acquire()` | Reuse the most recently released object or call `create`. |
| `release(object)` | Reset and return an active object; return `false` for a foreign or duplicate release. |
| `prewarm(count)` | Ensure at least `count` objects are immediately available. |
| `owns(object)` | Report whether the pool created an object. |
| `activeCount()` | Count objects currently checked out. |
| `availableCount()` | Count objects ready for reuse. |
| `capacity()` | Count all objects owned by the pool. |
| `clear()` | Call the optional `dispose` hook for every object and empty the pool. |

`initial` is equivalent to prewarming immediately after construction. Use a
`dispose` hook when pooled values own resources that need explicit cleanup:

```tua
local effects = objectPool({
  create = function(): Effect
    return makeEffect()
  end,
  reset = function(effect: Effect)
    effect:stop()
  end,
  dispose = function(effect: Effect)
    effect:releaseResources()
  end,
})
```

Calling `clear()` also disposes objects that are currently active. Do it only
when the owning scene or game state is being torn down.

## When Pooling Helps

Consider a pool when all of these are true:

- the same kind of short-lived object is created and discarded frequently;
- allocation or garbage-collection time is visible in a representative
  profile;
- every object has a clear acquire/release owner;
- resetting an object is cheaper and simpler than creating it;
- retaining the pool's peak capacity is acceptable.

Bullets, particles, collision contacts, floating text, and temporary effects
are common candidates. Prewarming can also move predictable allocation work
out of a gameplay frame.

Pooling is not automatically faster. It retains memory, adds lifecycle state,
and can hide ownership mistakes. Ordinary tables are usually clearer for
infrequent allocations, long-lived objects, configuration, scenes, and UI.
Measure the real game before and after introducing a pool.

## Pooling And ECS

ECS and pooling solve different problems:

- ECS organizes data and iterates populations by component composition.
- Object pooling reuses expensive or high-churn values.

!!! warning "Do not duplicate active ownership"
    Do not use a pool as a second list of active entities. Let ECS queries own
    population iteration and use a pool only for values whose allocation cost
    is measurable, such as complex component payloads or external LOVE objects.
    The built-in ECS already reuses destroyed numeric entity IDs.

The pool tracks object identity, active state, and ownership. Duplicate and
foreign releases return `false`, making lifecycle mistakes observable without
silently corrupting the available stack.

## Editor Assistance

The language server keeps object-pool facts in the shared semantic snapshot:

- completion, hover, signature help, inlay hints, definition, and semantic
  highlighting understand `objectPool` and its lifecycle methods;
- a quick fix can wrap repeated zero-argument constructor calls such as
  `Bullet.new()` in a new `objectPool({...})` declaration and rewrite the
  selected call to `bulletPool:acquire()`;
- `TL5007` warns when a directly acquired same-file local, such as
  `local bullet = bullets:acquire()`, has no later same-file
  `bullets:release(bullet)` or `bullets:clear()` call;
- `Tua: Show Object Pool Explorer` opens a compiler-backed view of pool
  declarations, `acquire`, `release`, `clear`, and `prewarm` calls, and
  lifecycle warnings.

The lifecycle warning is conservative. It does not try to prove ownership
through aliases, closures, module boundaries, or dynamic control flow. When a
pool is intentionally handed to another owner, release explicitly in the same
file or treat the warning as a prompt to make the ownership boundary clearer.
