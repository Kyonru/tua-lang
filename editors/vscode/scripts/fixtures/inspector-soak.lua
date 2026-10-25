local store = require('tuastore')
local signals = require('tuasignal')
local ecs = require('tuaecs')
local duration = tonumber(os.getenv('TUA_SOAK_SECONDS')) or 600
local stores, world, component = {}, ecs.world(), ecs.component('Position', {})
for i = 1, 48 do
  local value = {}; for j = 1, 32 do value['field' .. j] = j end
  stores[i] = store(value)
end
for i = 1, 2000 do world:add(world:create(), component, { x = i, y = i % 100 }) end
local bus = signals.new(); bus:on('tick', function() end)
local function update(value) value.field1 = value.field1 + 1 end
function love.run()
  local started, reported, frame = love.timer.getTime(), 0, 0
  return function()
    love.event.pump()
    frame = frame + 1
    for _, value in ipairs(stores) do value:update(update) end
    for i = 1, 128 do bus:emit('tick', frame) end
    if frame % 60 == 0 then for i = 1, 20 do store({ temporary = i }) end end
    local elapsed = love.timer.getTime() - started
    if elapsed - reported >= 10 or elapsed >= duration then
      collectgarbage('collect')
      print(string.format('SOAK {"seconds":%.3f,"heapKiB":%.3f,"frames":%d}', elapsed, collectgarbage('count'), frame))
      reported = elapsed
    end
    if elapsed >= duration then print('DONE'); return 0 end
    assert(world and bus and #stores == 48)
    love.timer.sleep(1 / 60)
  end
end
