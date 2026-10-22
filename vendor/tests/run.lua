-- Plain-Lua tests for bundled runtimes. Run with `make runtime-test`
-- (any Lua 5.1+ interpreter; tests stub the LOVE APIs they need).
local runtime, tests = arg[1] or "vendor/runtime", arg[2] or "vendor/tests"
package.path = runtime .. "/?.lua;" .. tests .. "/?.lua;" .. package.path

local failures = 0
for _, name in ipairs({ "tween_test", "camera_test", "hotreload_test" }) do
  local ok, err = pcall(require, name)
  if ok then
    print("ok   " .. name)
  else
    failures = failures + 1
    print("FAIL " .. name .. ": " .. tostring(err))
  end
end
if failures > 0 then
  os.exit(1)
end
