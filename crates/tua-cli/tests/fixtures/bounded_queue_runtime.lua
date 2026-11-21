local Queue = require('tuaboundedqueue')
local initial = { false, 2 }
local queue = Queue.new(3, initial)
initial[2] = 99
assert(queue.capacity() == 3 and queue.count() == 2)
assert(queue.pop() == false and queue.pop() == 2 and queue.pop() == nil)
for _ = 1, 200 do
  assert(queue.push(1) and queue.push(2) and queue.push(3))
  assert(not queue.push(4))
  assert(queue.pop() == 1 and queue.pop() == 2 and queue.pop() == 3)
end
assert(queue.rejected() == 200)
queue.clear()
assert(queue.rejected() == 0)
queue.push(1)
queue.push(2)
local seen = {}
assert(queue.drain(10, function(value)
  seen[#seen + 1] = value
  queue.push(value + 2)
  assert(queue.drain(10, function() error('nested drain') end) == 0)
end) == 2)
assert(seen[1] == 1 and seen[2] == 2 and queue.pop() == 3 and queue.pop() == 4)
queue.push(1)
queue.push(2)
assert(queue.drain(10, function()
  queue.clear()
  queue.push(9)
end) == 1)
assert(queue.pop() == 9)
queue.push(1)
queue.push(2)
assert(queue.drain(10, function()
  assert(queue.pop() == 2)
  queue.push(3)
end) == 1)
assert(queue.pop() == 3)
queue.push(1)
queue.push(2)
assert(not pcall(function() queue.drain(10, function() error('visitor failed') end) end))
assert(queue.drain(10, function(value) assert(value == 2) end) == 1)
assert(not pcall(function() queue.push(nil) end))
for _, capacity in ipairs({ 0, -1, 0.5, math.huge, 0 / 0 }) do
  assert(not pcall(function() Queue.new(capacity, {}) end))
end
assert(not pcall(function() Queue.new(1, { 1, 2 }) end))
-- Pop and clear must release payload references.
local weak = setmetatable({}, { __mode = 'v' })
for i = 1, 2 do
  local item = {}
  weak[i] = item
  queue.push(item)
end
queue.pop()
queue.clear()
collectgarbage('collect')
assert(weak[1] == nil and weak[2] == nil)
print('bounded queue FIFO, overflow, references, nested drains, clear/rebuild and errors passed')
