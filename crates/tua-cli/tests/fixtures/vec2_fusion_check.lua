-- Compares compiler-fused Vec2 arithmetic with BrineVector's own operators.
-- arg[1] is a build directory containing fused.lua (emitted from the Tua
-- module in the calling test) and brinevector.lua.
local build = assert(arg[1])
package.path = build .. "/?.lua;" .. package.path

local Vector = require("brinevector")
local fused = require("fused")

local state = 12345
local function random()
  state = (state * 1103515245 + 12345) % 2147483648
  -- Mix magnitudes so floating-point evaluation order would show up.
  local value = state / 2147483648
  local scale = ({ 1e-3, 0.1, 1, 37.5, 1e4 })[state % 5 + 1]
  return (value - 0.5) * scale
end

-- The same expressions as the Tua module, evaluated through metamethods.
local function expected(p, v, g, dt, k)
  local moved = p + v * dt
  local scaled = Vector(p.x, p.y) * (k + 1)
  local halved = v / 2
  return {
    p + v * dt,
    -(p - v) * 2,
    p + (v + g),
    (p * v) / 2,
    0.5 * dt * v,
    Vector(1, 2) * k,
    p - -v,
    p + v * dt + g * (0.5 * dt * dt),
    v * k / 4,
    moved,
    scaled,
    halved,
  }
end

local failures = 0
for case = 1, 500 do
  local p = Vector(random(), random())
  local v = Vector(random(), random())
  local g = Vector(random(), random())
  local dt = math.abs(random()) + 1e-4
  local k = random()

  local want = expected(p, v, g, dt, k)
  local got = fused.cases(p, v, g, dt, k)

  for index = 1, #want do
    local a, b = got[index], want[index]
    if not Vector.isVector(a) or a.x ~= b.x or a.y ~= b.y then
      failures = failures + 1
      io.stderr:write(string.format(
        "case %d, expression %d: got %s, want (%.17g, %.17g)\n",
        case, index, Vector.isVector(a) and string.format("(%.17g, %.17g)", a.x, a.y) or tostring(a),
        b.x, b.y))
    end
  end
end

if failures > 0 then
  io.stderr:write(failures .. " fused Vec2 result(s) differ from BrineVector\n")
  os.exit(1)
end
print("fused Vec2 arithmetic matches BrineVector")
