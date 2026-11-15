local reportPath, shimPath = ...
local elapsed = 0
local onSample
love = {
  timer = { getTime = function() elapsed = elapsed + 0.01; return elapsed end },
  update = function() end,
  run = function()
    return function()
      onSample({}, 1, "I")
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
local shim = assert(loadfile(shimPath))(reportPath, reportPath .. ".stop", 0)
shim.install()
love.run()()
