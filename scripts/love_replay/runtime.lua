-- Development runner only; never injected into ordinary game builds.
io.stdout:setvbuf('no')
function love.errorhandler(message)
  print('TUA_REPLAY_ERROR\n' .. debug.traceback(tostring(message), 2))
  return function() return 1 end
end
local identity = assert(os.getenv('TUA_REPLAY_IDENTITY'))
assert(identity:match('^tua%-replay%-[%w%-]+$'), 'invalid replay identity')
local setIdentity = love.filesystem.setIdentity
setIdentity(identity)
love.filesystem.setIdentity = function() return setIdentity(identity) end
print('TUA_REPLAY_SAVE\t' .. love.filesystem.getSaveDirectory())
local output = assert(os.getenv('TUA_REPLAY_OUTPUT'))
local frames = assert(tonumber(os.getenv('TUA_REPLAY_FRAMES')))
local fps = assert(tonumber(os.getenv('TUA_REPLAY_FPS')))
local capture = os.getenv('TUA_REPLAY_CAPTURE') == '1'
local seed = assert(tonumber(os.getenv('TUA_REPLAY_SEED')))
local context = { frame = 0, time = 0, dt = 1 / fps, seed = seed }
local keys, buttons = {}, {}
local pointerX, pointerY = 0, 0
local quit = love.event.quit
local done = false

local function held(values, ...)
  for index = 1, select('#', ...) do
    if values[select(index, ...)] then return true end
  end
  return false
end
love.keyboard.isDown = function(...) return held(keys, ...) end
love.keyboard.isScancodeDown = function(...) return held(keys, ...) end
love.mouse.isDown = function(...) return held(buttons, ...) end
love.mouse.getPosition = function() return pointerX, pointerY end
love.mouse.getX = function() return pointerX end
love.mouse.getY = function() return pointerY end
love.timer.getTime = function() return context.time end
love.timer.getDelta = function() return context.dt end
math.randomseed(seed)
love.math.setRandomSeed(seed)
-- Physical input must not contaminate scripted input. Scenario calls below
-- dispatch directly to game callbacks, while window close remains available.
for _, event in ipairs({
  'keypressed',
  'keyreleased',
  'textinput',
  'mousepressed',
  'mousereleased',
  'mousemoved',
  'wheelmoved',
  'touchpressed',
  'touchreleased',
  'touchmoved',
  'joystickpressed',
  'joystickreleased',
  'joystickaxis',
  'joystickhat',
  'gamepadpressed',
  'gamepadreleased',
  'gamepadaxis',
  'focus',
  'mousefocus',
  'visible',
}) do
  love.handlers[event] = function() end
end

function context.key(key, down)
  local previous = keys[key] or false
  keys[key] = down
  if down and not previous and love.keypressed then love.keypressed(key, key, false) end
  if not down and previous and love.keyreleased then love.keyreleased(key, key) end
end
function context.pointer(x, y, down)
  local dx, dy = x - pointerX, y - pointerY
  pointerX, pointerY = x, y
  if love.mousemoved then love.mousemoved(x, y, dx, dy, false) end
  local previous = buttons[1] or false
  buttons[1] = down
  if down and not previous and love.mousepressed then love.mousepressed(x, y, 1, false, 1) end
  if not down and previous and love.mousereleased then love.mousereleased(x, y, 1, false, 1) end
end
love.event.quit = function() error('game requested exit before replay verification', 2) end
require('__tua_replay_game')
local scenario = require(assert(os.getenv('TUA_REPLAY_SCENARIO')))
assert(type(scenario) == 'table', 'scenario must return a table of hooks')
for _, name in ipairs({ 'setup', 'beforeFrame', 'afterFrame', 'verify' }) do
  assert(
    scenario[name] == nil or type(scenario[name]) == 'function',
    'invalid scenario hook: ' .. name
  )
end
assert(type(scenario.verify) == 'function', 'scenario must provide verify(context)')
local loadGame, updateGame, drawGame = love.load, love.update, love.draw
function love.load(args)
  local major, minor, revision = love.getVersion()
  print('TUA_REPLAY_RUNTIME\t' .. major .. '.' .. minor .. '.' .. revision
    .. '\t' .. (jit and jit.version or _VERSION)
    .. '\t' .. tostring(jit and jit.status() or false))
  if loadGame then loadGame(args) end
  if scenario.setup then scenario.setup(context) end
end
function love.update()
  if done or context.frame >= frames then return end
  context.frame = context.frame + 1
  context.time = context.frame * context.dt
  if scenario.beforeFrame then scenario.beforeFrame(context) end
  if updateGame then updateGame(context.dt) end
  if scenario.afterFrame then scenario.afterFrame(context) end
end
local function finish()
  scenario.verify(context)
  done = true
  print('TUA_REPLAY_OK\t' .. context.frame)
  quit(0)
end
function love.draw()
  if done then return end
  if drawGame then drawGame() end
  local frame = context.frame
  if frame < 1 then return end
  if capture then
    love.graphics.captureScreenshot(function(data)
      local file = assert(io.open(output .. '/frames/' .. string.format('%06d.png', frame), 'wb'))
      file:write(data:encode('png'):getString())
      file:close()
      if frame == frames then finish() end
    end)
  elseif frame == frames then finish() end
end
-- Failures must exit unattended runs instead of leaving LOVE's error window open.
function love.errorhandler(message)
  print('TUA_REPLAY_ERROR\n' .. debug.traceback(tostring(message), 2))
  return function() return 1 end
end
love.quit = function() return false end
