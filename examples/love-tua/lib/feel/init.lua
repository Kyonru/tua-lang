-- A deliberately small, vendored Feel-style feedback runner for this example.
-- The runtime remains plain Lua while lib/feel/types/feel.lua supplies the
-- declaration surface that Tua indexes without copying into the build.
local feel = {}
local definitions = {}
local active = {}

function feel.define(name, cue)
  assert(type(name) == "string" and name ~= "", "feedback name must be a string")
  assert(type(cue) == "table", "feedback cue must be a table")
  definitions[name] = cue
end

function feel.play(name)
  local cue = definitions[name]

  if cue == nil then
    return false
  end

  active[name] = {
    elapsed = 0,
    duration = cue.duration or 0.12,
    strength = cue.strength or 1,
  }
  return true
end

function feel.update(dt)
  for name, cue in pairs(active) do
    cue.elapsed = cue.elapsed + dt

    if cue.elapsed >= cue.duration then
      active[name] = nil
    end
  end
end

function feel.amount(name)
  local cue = active[name]

  if cue == nil then
    return 0
  end

  return cue.strength * (1 - cue.elapsed / cue.duration)
end

return feel

