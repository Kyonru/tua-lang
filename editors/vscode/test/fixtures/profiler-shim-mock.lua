local reportPath, shimPath, callbacks = ...
local elapsed = 0
local onSample
jit = { version = "LuaJIT fixture", status = function() return false end }
love = {
  getVersion = function() return 11, 5, 0 end,
  timer = { getTime = function() elapsed = elapsed + 0.01; return elapsed end },
  update = function() end,
  run = function()
    return function()
      for _ = 1, tonumber(callbacks) or 1 do onSample({}, 1, "I") end
      love.update()
      return true
    end
  end,
}
package.preload["jit.profile"] = function()
  return {
    start = function(_, callback) onSample = callback end,
    stop = function() end,
    dumpstack = function() return "@main.lua:9\n[C]:0" end,
  }
end
local shim = assert(loadfile(shimPath))(reportPath, reportPath .. ".stop", 0, { context = { target = "love11-luajit", gcPolicy = "running", benchmark = { workload = "fixture", revision = "test", warmupUpdates = 10, repetitions = 3 } } })
shim.install()
love.run()()
