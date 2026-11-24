---
icon: lucide/git-branch
---

# Behavior trees

`BehaviorTree` is an optional Lua runtime for choosing an actor's behavior each
frame. In Tua, calling `BehaviorTree.new()` inserts the runtime import and
copies `tuabehavior.lua` into the build. Projects that never reference it do
not include it. Plain Lua can use `local BehaviorTree = require("tuabehavior")`.

Think of a tree definition as reusable behavior and a runner as one actor's
progress through that behavior. Share the definition, create one runner per
actor, and update each runner from the game loop. The runner owns timers and
composite cursors; the context is the actor data your callbacks read or change.

```tua
local enemy = { seesPlayer = false, shots = 0 }
local tree = BehaviorTree.new(BehaviorTree.reactiveSelector({
  BehaviorTree.sequence({
    BehaviorTree.condition(function(actor) return actor.seesPlayer end),
    BehaviorTree.action(function(actor, dt)
      actor.shots = actor.shots + 1
      return "success"
    end),
  }),
  BehaviorTree.action(function(actor) return "running" end),
}))
local runner = tree:instance(enemy)

function love.update(dt)
  runner:update(dt)
end
```

Action results tell the parent node what to do next:

| Result | Meaning |
| --- | --- |
| `"success"` | This action completed successfully. |
| `"failure"` | This action could not complete. |
| `"running"` | Keep this action active and visit it on a later update. |

Conditions return a boolean instead. A `wait(seconds)` node stays running until
its per-runner timer expires, then returns success.

`new()` may also be called without a root; call `tree:setRoot(node)` before
creating a runner. A definition can create many runners. Each runner owns its
own execution state. Keep
the definition's nodes unchanged after instancing.
Use `reactiveSelector` when a running idle branch must yield as soon as an
earlier condition becomes true. A plain `selector` resumes its running child
without revisiting earlier branches.

## Use a tree with ECS and object pools

Let ECS own which enemies are active. Give each enemy one runner and one context
when it spawns, then update the runner through an ECS system. The tree itself is
shared by every enemy:

```tua
type Actor = { wantsEffect: boolean }
type Runner = {
  context: Actor,
  update: (self: Self, dt: number) -> ("success" | "failure" | "running"),
  cancel: (self: Self) -> nil,
}
local Brain = ECS.component("Brain", { context = "Actor", runner = "Runner" })
local Effect = ECS.component("Effect")
local world = ECS.world()
local effects = objectPool({
  create = function() return { age = 0 } end,
  reset = function(effect) effect.age = 0 end,
  initial = 32,
})

local tree = BehaviorTree.new(BehaviorTree.action(function(actor: Actor)
  actor.wantsEffect = true
  return "success"
end))

local function spawnEnemy(actor: Actor)
  local entity = world:create()
  world:add(entity, Brain, { context = actor, runner = tree:instance(actor) })
  return entity
end

local brains = ECS.system(world, { Brain }, function(_, brain, dt)
  brain.runner:update(dt)
  if brain.context.wantsEffect then
    brain.context.wantsEffect = false
    world:add(world:create(), Effect, effects:acquire())
  end
end)

local function removeEnemy(entity)
  local brain = world:get(entity, Brain)
  if brain then brain.runner:cancel() end
  world:destroy(entity)
end
```

Use an object pool for short-lived effect payloads produced by the tree. Acquire
an effect when an action requests it, attach it to an ECS entity, then destroy
that entity and release the payload when its lifetime ends. Do not pool numeric
entity IDs or create a tree or runner in `love.update`. The complete,
[runnable ECS and pool example](https://github.com/Kyonru/tua-lang/blob/main/examples/features/src/behavior_tree_ecs.tua)
shows effect release, a prewarmed pool, and headless updates. ECS schemas can
name local type aliases: the example checks both the actor and runner without
`any`.

The runner alias includes only the methods the system needs; inferred
runners also expose `reset()`, `maxDepth`, and `maxSteps`.

The brain update path makes one ECS query and one runner update per active
enemy.

Tree nodes and callbacks are allocated once; runner state is per enemy.
The runtime creates node-state tables lazily and reuses them after completion,
so steady decisions avoid new node-state tables after warm-up. Spawning effect
entities and growing a pool can still allocate. Use a cooldown or lower-frequency
update for decisions that do not need every frame, and profile representative
enemy counts before adding pooling or changing tick rates.

Call `runner:cancel()`
before removing an enemy; when clearing a whole world, cancel its runners and
release active pooled values first.

## Static editor preview

Enable `tua.preview.enabled`, then run **Tua: Preview Behavior Tree** or use a
tree name's hover action. The preview recognizes top-level
`local tree = BehaviorTree.new(...)` definitions and `tree:setRoot(...)` calls.
Direct nested constructors appear as nodes with source links.

Click a node to open its constructor. Collapse branches to focus the view;
collapse choices survive refreshes. Drag empty space to pan, and pinch or use
Control/Command + wheel to zoom. Zoom and Fit buttons are also available.
The visible preview follows edits; hidden previews refresh when reopened.

Try the [feature example](https://github.com/Kyonru/tua-lang/blob/main/examples/features/src/behavior_tree.tua)
for a complete tree.

The preview reads source only; it never runs game code. Nodes supplied by a
variable, helper function, or another module appear as **Computed node** because
their structure cannot be known from the local expression. It does not show
live runner statuses.

For a running game, enable the **Behavior runners** category in the optional
development Runtime Inspector. Each runner shows its latest root result,
active node paths after the last tick, step count, and a bounded history of 20
root results. Completed trees have no active nodes. This runtime sample is
separate from the static preview and is absent from production builds.

`action(callback, enter?, exit?)` callbacks receive `(context, dt)` and return
`"success"`, `"failure"`, or `"running"`. `enter(context)` runs on first visit;
`exit(context, reason)` runs when the action finishes or its runner is reset
or cancelled.

Conditions return a boolean. `wait(seconds)` keeps elapsed time
per runner. `sequence` and `selector` remember a running child. `invert` swaps
success and failure; `repeatN(count, child)` repeats a successful child across
updates and fails if the child fails.

`reactiveSequence` and `reactiveSelector` revisit earlier children every
update and interrupt a previously running child when a higher-priority branch
changes the outcome. `parallelAll` succeeds when every child succeeds and
fails when any child fails. `parallelAny` succeeds when any child succeeds and
fails when all children fail. Both keep completed child results until the
parallel node completes, then cancel any still-running siblings.

Use `runner:reset()` to clear running state or `runner:cancel()` when an actor
is removed. Invalid action results, recursive updates, negative `dt`, shared
node objects, and excessive traversal depth or work raise errors. The default
limits are 64 levels and 1,000 node visits per update; `runner.maxDepth` and
`runner.maxSteps` can be lowered for a game. The runtime has no LOVE dependency
and can be exercised in headless tests.

## Actor context checking

Annotate the context parameter on a leaf callback (or an action's `enter` or
`exit` callback). Direct calls through `BehaviorTree`, an alias of the module,
or its built-in runtime import carry that context through combinators,
decorators, `new(root)`, and `tree:instance(actor)`. The checker rejects an
incompatible actor or callback; runner context fields retain their types after
module exports and re-exports. Completion and call signatures use the same
contract.

Context-free waits and callbacks can be mixed with typed leaves.
Use one common actor contract for typed leaves in a tree; incompatible leaf
contracts are rejected rather than widened into a union.

```tua
type Actor = { health: number }
local tree = BehaviorTree.new(BehaviorTree.action(function(actor: Actor)
  actor.health -= 1
  return "success"
end))
local runner = tree:instance({ health = 3 })
-- tree:instance({ health = "wrong" }) -- type error
-- runner.context.health = "wrong" -- type error
```

This is inference for the existing API, not a change to Lua's runtime or general
generic callback inference. Fully dynamic callbacks remain permissive. For a
checked tree, supply the root to `new(root)`: constructing an empty tree and
later calling `setRoot` does not refine the original variable's context type.
Extracting a constructor into a standalone function also retains its dynamic
LuaDoc contract. No context witness or type metadata is emitted into Lua.

This typed sequence checks a condition, waits, then performs an action. The
wait timer belongs to the runner, so actors can be at different points in the
same shared definition:

```tua
type Enemy = {
  seesPlayer: boolean,
  fired: boolean,
}

local attack = BehaviorTree.new(BehaviorTree.sequence({
  BehaviorTree.condition(function(enemy: Enemy)
    return enemy.seesPlayer
  end),
  BehaviorTree.wait(0.25),
  BehaviorTree.action(function(enemy: Enemy)
    enemy.fired = true
    return "success"
  end),
}))

local enemy: Enemy = { seesPlayer = true, fired = false }
local runner = attack:instance(enemy)

function love.update(dt: number)
  runner:update(dt)
end
```

For several actors, call `attack:instance(actor)` once per actor and update each
runner. Do not share one runner between actors: its timer and active-node state
belong to that instance.
