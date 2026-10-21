-- Checks statement-level Vec2 fusion: every field and index read must happen
-- exactly once, and results must match BrineVector's own operators.
-- arg[1] is a build directory containing hoisted.lua and brinevector.lua.
local build = assert(arg[1])
package.path = build .. "/?.lua;" .. package.path

local Vector = require("brinevector")
local hoisted = require("hoisted")

-- A table whose reads are counted by key and whose writes go to `values`.
local function counted(values)
  local reads = {}
  local proxy = setmetatable({}, {
    __index = function(_, key)
      reads[key] = (reads[key] or 0) + 1
      return values[key]
    end,
    __newindex = function(_, key, value)
      values[key] = value
    end,
  })
  return proxy, reads
end

local state = 2024
local function random()
  state = (state * 1103515245 + 12345) % 2147483648
  local scale = ({ 1e-3, 0.1, 1, 37.5, 1e4 })[state % 5 + 1]
  return (state / 2147483648 - 0.5) * scale
end
local function randomVector()
  return Vector(random(), random())
end

local failures = 0
local function check(condition, message)
  if not condition then
    failures = failures + 1
    io.stderr:write("FAIL: " .. message .. "\n")
  end
end
local function same(a, b)
  return Vector.isVector(a) and a.x == b.x and a.y == b.y
end

for case = 1, 200 do
  local pos, vel, extra = randomVector(), randomVector(), randomVector()
  local speed, dt = random(), math.abs(random()) + 1e-4

  -- body.pos += body.vel * dt
  local values = { pos = pos, vel = vel, speed = speed }
  local body, reads = counted(values)
  hoisted.integrate(body, dt)
  check(reads.pos == 1 and reads.vel == 1, "integrate reads pos and vel once (case " .. case .. ")")
  check(same(values.pos, pos + vel * dt), "integrate matches BrineVector (case " .. case .. ")")

  -- positions[i] = positions[i] + velocities[i] * dt
  local positionValues, velocityValues = { [3] = pos }, { [3] = vel }
  local positions, positionReads = counted(positionValues)
  local velocities, velocityReads = counted(velocityValues)
  hoisted.advance(positions, velocities, 3, dt)
  check(positionReads[3] == 1 and velocityReads[3] == 1, "advance reads each element once (case " .. case .. ")")
  check(same(positionValues[3], pos + vel * dt), "advance matches BrineVector (case " .. case .. ")")

  -- local predicted = body.pos + body.vel * (body.speed * dt); return predicted
  values = { pos = pos, vel = vel, speed = speed }
  body, reads = counted(values)
  local predicted = hoisted.predict(body, dt)
  check(reads.pos == 1 and reads.vel == 1 and reads.speed == 1, "predict reads each field once (case " .. case .. ")")
  check(same(predicted, pos + vel * (speed * dt)), "predict matches BrineVector (case " .. case .. ")")

  -- return -body.pos + extra * 2
  values = { pos = pos, vel = vel, speed = speed }
  body, reads = counted(values)
  local offset = hoisted.offset(body, extra)
  check(reads.pos == 1 and reads.vel == nil, "offset reads pos once (case " .. case .. ")")
  check(same(offset, -pos + extra * 2), "offset matches BrineVector (case " .. case .. ")")
end

-- Hoisting deliberately reads every operand before component arithmetic. This
-- makes failures name the generated local and means a later getter runs before
-- an earlier nil vector fails at `.x`.
do
  local body, reads = counted({ pos = nil, vel = Vector(1, 2) })
  local ok, message = pcall(hoisted.failAfterReads, body)
  check(not ok, "nil vector should fail")
  check(
    type(message) == "string" and message:find("__tua_body_pos", 1, true) ~= nil,
    "error names the hoisted field local"
  )
  check(
    reads.pos == 1 and reads.vel == 1,
    "all hoisted reads happen before component arithmetic"
  )
end

if failures > 0 then
  io.stderr:write(failures .. " hoisted Vec2 check(s) failed\n")
  os.exit(1)
end
print("hoisted Vec2 reads happen once and match BrineVector")
