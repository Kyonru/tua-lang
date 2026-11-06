-- Run with real LOVE/LuaJIT; render and keyboard calls are deterministic stubs.
local root = assert(os.getenv('TUA_ARENA_BUILD'))
package.path = root .. '/?.lua;' .. package.path
local keys = {}
local draws = 0
love.keyboard = {isDown = function(...) for _,k in ipairs({...}) do if keys[k] then return true end end return false end}
love.window = {setMode=function() end,setTitle=function() end}
love.graphics = setmetatable({}, {__index=function() return function() draws=draws+1 end end})
local game = assert(loadfile(root .. '/main.lua'))()
love.load()
assert(game.snapshot().status == 'playing')
keys.d = true
love.update(0.05)
keys.d = false
assert(game.snapshot().player.x == 412)
local state = game.snapshot()
table.insert(state.enemies, {x=state.player.x+30,y=state.player.y,radius=11,speed=0})
love.keypressed('space')
assert(game.snapshot().score == 1)
state = game.snapshot()
table.insert(state.enemies, {x=state.player.x,y=state.player.y,radius=11,speed=0})
love.update(0.01)
assert(game.snapshot().status == 'lost')
love.keypressed('r')
assert(game.snapshot().status == 'playing' and #game.snapshot().enemies == 0)
-- Remove newly spawned enemies after each step to isolate win timer.
for i=1,901 do
  love.update(0.05)
  local enemies = game.snapshot().enemies
  for j=#enemies,1,-1 do table.remove(enemies,j) end
end
assert(game.snapshot().status == 'won')
love.draw()
assert(draws > 30)
print('PASS: movement, pulse score, collision/loss, restart, survival/win, draw dispatch; '.._VERSION..' '..jit.version)
os.exit(0)
