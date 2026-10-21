local SignalBus = {}
SignalBus.__index = SignalBus

local function assert_name(name)
  assert(type(name) == "string", "signal name must be a string")
end

local function assert_pattern(pattern)
  assert(type(pattern) == "string", "signal pattern must be a string")
end

local function assert_handler(handler)
  assert(type(handler) == "function", "signal handler must be a function")
end

local function copy_list(list)
  local copy = {}
  for index = 1, #list do copy[index] = list[index] end
  return copy
end

local function signal_list(self, name)
  local list = self._signals[name]

  if list == nil then
    list = {}
    self._signals[name] = list
  elseif list._emitting then
    list = copy_list(list)
    self._signals[name] = list
  end

  return list
end

local function each_matching(self, pattern, callback)
  for name in pairs(self._signals) do
    if name:match(pattern) then
      callback(name)
    end
  end
end

function SignalBus:on(name, handler)
  assert_name(name)
  assert_handler(handler)

  local list = signal_list(self, name)
  list[#list + 1] = handler

  return handler
end

function SignalBus:once(name, handler)
  assert_name(name)
  assert_handler(handler)

  local wrapped
  wrapped = function(...)
    self:off(name, wrapped)
    return handler(...)
  end

  return self:on(name, wrapped)
end

function SignalBus:declare(name, handler)
  assert_name(name)

  if handler == nil then
    return self
  end

  assert_handler(handler)
  return handler
end

function SignalBus:emit(name, ...)
  assert_name(name)

  local list = self._signals[name]

  if list == nil then
    return
  end

  -- Mutations detach from this exact list, so this loop needs no snapshot.
  -- If a handler raises, leaving the private marker set is safe: the next
  -- mutation copies the abandoned version before changing it.
  list._emitting = (list._emitting or 0) + 1
  for index = 1, #list do
    list[index](...)
  end
  list._emitting = list._emitting - 1
  if list._emitting == 0 then list._emitting = nil end
end

function SignalBus:off(name, handler)
  assert_name(name)

  if handler == nil then
    self._signals[name] = nil
    return
  end

  local list = self._signals[name]

  if list == nil then
    return
  end

  local found = false
  for index = #list, 1, -1 do
    if list[index] == handler then
      found = true
      break
    end
  end
  if not found then return end

  if list._emitting then
    list = copy_list(list)
    self._signals[name] = list
  end

  for index = #list, 1, -1 do
    if list[index] == handler then
      table.remove(list, index)
    end
  end

  if #list == 0 then
    self._signals[name] = nil
  end
end

function SignalBus:clear(name)
  if name == nil then
    self._signals = {}
    return
  end

  assert_name(name)
  self._signals[name] = nil
end

function SignalBus:has(name)
  assert_name(name)

  local list = self._signals[name]

  return list ~= nil and #list > 0
end

function SignalBus:listeners(name)
  assert_name(name)

  local list = self._signals[name]

  if list == nil then
    return {}
  end

  return copy_list(list)
end

function SignalBus:onPattern(pattern, handler)
  assert_pattern(pattern)
  assert_handler(handler)

  each_matching(self, pattern, function(name)
    self:on(name, handler)
  end)

  return handler
end

function SignalBus:emitPattern(pattern, ...)
  assert_pattern(pattern)

  local payload_count = select("#", ...)
  local payload = { ... }

  each_matching(self, pattern, function(name)
    self:emit(name, unpack(payload, 1, payload_count))
  end)
end

function SignalBus:offPattern(pattern, handler)
  assert_pattern(pattern)

  each_matching(self, pattern, function(name)
    self:off(name, handler)
  end)
end

function SignalBus:clearPattern(pattern)
  assert_pattern(pattern)

  each_matching(self, pattern, function(name)
    self:clear(name)
  end)
end

SignalBus.register = SignalBus.on
SignalBus.remove = SignalBus.off
SignalBus.registerPattern = SignalBus.onPattern
SignalBus.removePattern = SignalBus.offPattern

local module = {}

function module.new()
  return setmetatable({ _signals = {} }, SignalBus)
end

return setmetatable(module, {
  __call = function()
    return module.new()
  end,
})
