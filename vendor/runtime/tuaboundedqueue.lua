---A bounded FIFO with copied initial entries and reentrant-safe, snapshot-bounded drains.
---@class BoundedQueue<T>
---@field push fun(value: T): boolean
---@field pop fun(): T | nil
---@field count fun(): number
---@field capacity fun(): number
---@field rejected fun(): number
---@field clear fun()
---@field drain fun(limit: number, visit: fun(value: T)): number
local Queue = {}

---@generic T
---@param capacity number
---@param initial T[] A dense array; copied, never retained or modified.
---@return BoundedQueue<T>
function Queue.new(capacity, initial)
  assert(type(capacity) == 'number' and capacity >= 1 and capacity < math.huge
    and capacity % 1 == 0, 'queue capacity must be a positive finite integer')
  assert(type(initial) == 'table' and #initial <= capacity, 'initial queue exceeds capacity')
  local values = {}
  local head, size, rejected = 1, 0, 0
  local removed = 0
  local draining = false
  local queue = {}

  function queue.push(value)
    assert(value ~= nil, 'queue does not accept nil')
    if size == capacity then
      rejected = rejected + 1
      return false
    end
    values[(head + size - 1) % capacity + 1] = value
    size = size + 1
    return true
  end

  function queue.pop()
    if size == 0 then return nil end
    local value = values[head]
    values[head] = nil
    head = head % capacity + 1
    size = size - 1
    if draining then removed = removed + 1 end
    return value
  end

  function queue.count() return size end
  function queue.capacity() return capacity end
  function queue.rejected() return rejected end

  function queue.clear()
    values = {}
    head, size, rejected = 1, 0, 0
    removed = 0
  end

  function queue.drain(limit, visit)
    assert(type(limit) == 'number' and limit >= 0 and limit < math.huge
      and limit % 1 == 0, 'drain limit must be a nonnegative finite integer')
    assert(type(visit) == 'function', 'drain visitor must be a function')
    if draining then return 0 end
    local count = math.min(limit, size)
    local generation, processed, boundary = values, 0, size
    removed = 0
    draining = true
    local ok, message = xpcall(function()
      for _ = 1, count do
        if values ~= generation or size == 0 or removed >= boundary then break end
        local value = queue.pop()
        processed = processed + 1
        visit(value)
      end
    end, debug.traceback)
    draining = false
    if not ok then error(message, 0) end
    return processed
  end

  for index = 1, #initial do queue.push(initial[index]) end
  return queue
end

return Queue
