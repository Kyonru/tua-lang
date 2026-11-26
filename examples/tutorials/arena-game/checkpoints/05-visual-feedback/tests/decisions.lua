local Combat = require("combat")
local Session = require("session")
local Collision = require("collision")
local Brain = require("brain")
local function step(combat, count, held)
  for _ = 1, count do
    combat.update(1 / 120, 0, 0, held or false, 1100, 572)
  end
end
local function first(combat)
  local selected
  combat.enemies:each(function(id)
    if not selected or id < selected then
      selected = id
    end
  end)
  return selected
end
local player = { x = 100, y = 100 }
local combat = Combat.new(player)
local id = first(combat)
local brain = combat.readBrain(id)
local position = combat.readPosition(id)
step(combat, 1)
assert(brain.context.mode == "patrol")
player.x, player.y = position.x - 100, position.y + 28
step(combat, 1)
assert(brain.context.mode == "chase" and brain.context.vx < 0)
player.x, player.y = position.x, position.y + 28
step(combat, 1)
assert(brain.context.mode == "attack" and combat.state.health == 9)
local attacks = combat.state.attacks
step(combat, 119)
assert(combat.state.attacks == attacks, "cooldown forbids early attacks")
step(combat, 1)
assert(combat.state.attacks == attacks + 1)
player.x, player.y = 32, 48
step(combat, 1)
assert(brain.context.mode == "patrol", "reactive branch must abandon attack when player leaves")
combat.destroyEnemy(id)
local cooldown = brain.context.cooldown
step(combat, 10)
assert(brain.context.cooldown == cooldown and not brain.context.attack)
assert(combat.readBrain(id) == nil)
-- Replacing a payload is observed: brains update the current Position, not a cached reference.
id = first(combat)
local oldPosition = combat.readPosition(id)
local Components = require("components")
combat.world:add(id, Components.Position, { x = 500, y = 500, previousX = 500, previousY = 500 })
local oldX = oldPosition.x
step(combat, 1)
assert(oldPosition.x == oldX and combat.readPosition(id).x ~= 500)
local retained = combat.readBrain(id)
combat.reset()
assert(combat.readBrain(id) == nil and not retained.context.attack)
local retainedCooldown = retained.context.cooldown
step(combat, 10)
assert(retained.context.cooldown == retainedCooldown)
combat.dispose()
-- Relative sweep catches an enemy crossing a stationary projectile.
assert(Collision.contact(0 - -50, 0, 0 - 50, 0, 0, 0, 25) ~= nil)
-- Lethal projectile wins over an attack intent from that same enemy.
combat = Combat.new({ x = 800, y = 600 })
id = first(combat)
local remove = {}
combat.enemies:each(function(other)
  if other ~= id then
    remove[#remove + 1] = other
  end
end)
for _, other in ipairs(remove) do
  combat.destroyEnemy(other)
end
position = combat.readPosition(id)
position.x, position.y = 820, 572
combat.readHealth(id).current = 1
step(combat, 1, true)
assert(combat.state.defeated == 1 and combat.state.health == 10 and combat.state.attacks == 0)
assert(combat.readBrain(id) == nil)
combat.dispose()
-- Shared definition, independent runner contexts; cancellation ends the active action.
local one, two = Brain.create(0), Brain.create(0)
one.context.distance = 0
two.context.distance = 500
one.runner:update(1 / 120)
two.runner:update(1 / 120)
assert(one.context.mode == "attack" and two.context.mode == "patrol")
one.runner:cancel()
two.runner:cancel()
-- Death ends remaining catch-up ticks, then the session owns game over.
local session = Session.new()
session.update(0, 0, 0, "confirm")
session.combat.enemies:each(function(_, pos)
  pos.x, pos.y = 800, 572
end)
for _ = 1, 10 do
  session.update(0.25, 0, 0, "none")
end
assert(session.flow:is("gameover") and session.combat.state.health == 0)
local ticks = session.combat.state.ticks
session.trigger.held = true
session.update(1, 1, 0, "none")
assert(session.combat.state.ticks == ticks)
assert(not session.combat.nextWave())
session.update(1, 1, 0, "pause")
assert(session.flow:is("gameover"))
session.update(1, 1, 0, "confirm")
assert(session.flow:is("playing") and session.combat.state.health == 10)
assert(session.player.x == 800 and session.combat.state.ticks == 0)
session.combat.dispose()
print(
  "ARENA_DECISIONS_OK patrol/chase/attack, cooldown, cancellation, replacement, kill priority, game over"
)
