local Candidate = require("combat")
local Reference = require("reference")
local Components = require("components")
local function snapshot(combat, player)
  local values = { player.x, player.y }
  for _, key in ipairs({
    "health",
    "attacks",
    "wave",
    "defeated",
    "shots",
    "hits",
    "skipped",
    "ticks",
    "hitEntity",
    "hitX",
    "hitY",
  }) do
    values[#values + 1] = combat.state[key]
  end
  local ids = {}
  combat.enemies:each(function(id)
    ids[#ids + 1] = id
  end)
  table.sort(ids)
  for _, id in ipairs(ids) do
    local position, health, brain =
      combat.readPosition(id), combat.readHealth(id), combat.readBrain(id)
    for _, value in ipairs({
      id,
      position.x,
      position.y,
      position.previousX,
      position.previousY,
      health.current,
      brain.context.mode,
      brain.context.cooldown,
      brain.context.attack,
    }) do
      values[#values + 1] = value
    end
  end
  for _, bullet in ipairs(combat.bullets) do
    for _, value in ipairs({ bullet.x, bullet.y, bullet.vx, bullet.vy, bullet.age, bullet.hit }) do
      values[#values + 1] = value
    end
  end
  return values
end
local seed = 71
local function random(limit)
  seed = seed * 48271 % 2147483647
  return seed % limit
end
for _, count in ipairs({ 8, 32, 128 }) do
  local leftPlayer, rightPlayer = { x = 800, y = 600 }, { x = 800, y = 600 }
  local left, right = Reference.new(leftPlayer), Candidate.new(rightPlayer)
  left.options.enemyCount, right.options.enemyCount = count, count
  for index = 1, 1200 do
    if index % 60 == 1 then
      for _, game in ipairs({ left, right }) do
        game.reset()
        -- Exercise dense simultaneous attack intents, not only approaching enemies.
        if index % 120 == 1 then
          game.enemies:each(function(_, position)
            position.x, position.y = 800, 572
          end)
        end
      end
      leftPlayer.x, leftPlayer.y = 800, 600
      rightPlayer.x, rightPlayer.y = 800, 600
    end
    if index % 37 == 0 then
      local id
      left.enemies:each(function(candidate)
        if not id or candidate < id then
          id = candidate
        end
      end)
      if id then
        for _, game in ipairs({ left, right }) do
          game.world:add(id, Components.Position, {
            x = 820,
            y = 572,
            previousX = 820,
            previousY = 572,
          })
          game.readHealth(id).current = 1
        end
      end
    end
    local x, y, held = random(3) - 1, random(3) - 1, random(2) == 1
    local aimX, aimY = 800 + random(700) - 350, 572 + random(700) - 350
    local dt = ({ 0, 1 / 120, 1 / 60, 0.05, 0.25 })[random(5) + 1]
    left.update(dt, x, y, held, aimX, aimY)
    right.update(dt, x, y, held, aimX, aimY)
    local expected, actual = snapshot(left, leftPlayer), snapshot(right, rightPlayer)
    assert(#expected == #actual, "snapshot length differs")
    for key, value in ipairs(expected) do
      assert(value == actual[key], "differential mismatch at step " .. index .. " field " .. key)
    end
  end
  left.dispose()
  right.dispose()
end
print(
  "ARENA_DIFFERENTIAL_OK 3600 mixed steps, 8/32/128 enemies, attacks, replacements, deaths and resets"
)
