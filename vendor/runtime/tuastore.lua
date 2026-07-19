-- Tua store runtime.
-- Copyright (c) 2026 Kyonru. MIT license.

local Store = {}
Store.__index = Store

local function shallowCopy(source)
  local copy = {}
  for key, value in pairs(source) do
    copy[key] = value
  end
  return copy
end

local function removeListener(listeners, listener)
  for index = #listeners, 1, -1 do
    if listeners[index] == listener then
      table.remove(listeners, index)
      return
    end
  end
end

local function notifyListeners(listeners, ...)
  local snapshot = shallowCopy(listeners)
  for index = 1, #snapshot do
    snapshot[index](...)
  end
end

function Store:get()
  return self._state
end

function Store:set(nextState)
  assert(type(nextState) == "table", "store:set expects a state table")

  local previous = self._state
  self._state = nextState
  self:_notify(previous)
  return self._state
end

function Store:update(mutator)
  assert(type(mutator) == "function", "store:update expects a function")

  local previous = shallowCopy(self._state)
  mutator(self._state)
  self:_notify(previous)
  return self._state
end

function Store:subscribe(listener)
  assert(type(listener) == "function", "store:subscribe expects a function")

  table.insert(self._listeners, listener)
  local active = true

  return function()
    if active then
      active = false
      removeListener(self._listeners, listener)
    end
  end
end

function Store:watch(key, listener)
  assert(type(key) == "string", "store:watch expects a string key")
  assert(type(listener) == "function", "store:watch expects a function")

  local listeners = self._watchers[key]
  if listeners == nil then
    listeners = {}
    self._watchers[key] = listeners
  end

  table.insert(listeners, listener)
  local active = true

  return function()
    if active then
      active = false
      removeListener(listeners, listener)
      if #listeners == 0 then
        self._watchers[key] = nil
      end
    end
  end
end

function Store:_notify(previous)
  local state = self._state
  local watchers = shallowCopy(self._watchers)
  notifyListeners(self._listeners, state, previous)

  for key, listeners in pairs(watchers) do
    local value = state[key]
    local previousValue = previous[key]
    if value ~= previousValue then
      notifyListeners(listeners, value, previousValue, state)
    end
  end
end

return function(initialState)
  assert(type(initialState) == "table", "store expects an initial state table")

  return setmetatable({
    _state = initialState,
    _listeners = {},
    _watchers = {},
  }, Store)
end
