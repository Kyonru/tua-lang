-- Tua store runtime.
-- Copyright (c) 2026 Kyonru. MIT license.

local Store = {}
Store.__index = Store
local EMPTY_LIST = {}

local function shallowCopy(source)
  local copy = {}
  for key, value in pairs(source) do
    copy[key] = value
  end
  return copy
end

-- Keep state snapshots separate from the small listener and watcher copies so
-- the large-table hot path remains straightforward for Lua and LuaJIT.
local function snapshotState(state)
  local snapshot = {}
  for key, value in pairs(state) do
    snapshot[key] = value
  end
  return snapshot
end

local function addListener(listeners, listener)
  local copy = shallowCopy(listeners)
  copy[#copy + 1] = listener
  return copy
end

local function removeListener(listeners, listener)
  for index = #listeners, 1, -1 do
    if listeners[index] == listener then
      local copy = shallowCopy(listeners)
      table.remove(copy, index)
      return copy
    end
  end
  return listeners
end

local function notifyListeners(listeners, ...)
  for index = 1, #listeners do
    listeners[index](...)
  end
end

local function snapshotWatchedValues(watchers, state, previous)
  previous = previous or {}
  for key in pairs(watchers) do
    previous[key] = state[key]
  end
  return previous
end

local function notifyStore(store, previous, listeners, watchers)
  local state = store._state
  notifyListeners(listeners, state, previous)

  for key, listeners in pairs(watchers) do
    local value = state[key]
    local previousValue = previous[key]
    if value ~= previousValue then
      notifyListeners(listeners, value, previousValue, state)
    end
  end
end

local function updateSubscribers(store, mutator, listeners, watchers)
  local previous = snapshotState(store._state)
  mutator(store._state)
  notifyStore(store, previous, listeners, watchers)
  return store._state
end

-- Watchers receive individual previous values, never the table itself, so a
-- watcher-only update reuses one per-store scratch table. A nested update (from
-- the mutator or a watcher) finds it taken and allocates its own, and an error
-- simply drops it.
local function updateWatchers(store, mutator, watchers)
  local previous = snapshotWatchedValues(watchers, store._state, store._watchedScratch)
  store._watchedScratch = nil
  mutator(store._state)
  notifyStore(store, previous, EMPTY_LIST, watchers)
  for key in pairs(previous) do
    previous[key] = nil
  end
  store._watchedScratch = previous
  return store._state
end

function Store:get()
  return self._state
end

function Store:set(nextState)
  assert(type(nextState) == "table", "store:set expects a state table")

  local previous = self._state
  local listeners = self._listeners
  local watchers = self._watchers
  if #listeners == 0 and next(watchers) == nil then
    self._state = nextState
    return self._state
  end

  self._state = nextState
  notifyStore(self, previous, listeners, watchers)
  return self._state
end

function Store:update(mutator)
  assert(type(mutator) == "function", "store:update expects a function")

  local listeners = self._listeners
  local watchers = self._watchers

  if #listeners == 0 and next(watchers) == nil then
    mutator(self._state)
    return self._state
  end

  if #listeners > 0 then
    return updateSubscribers(self, mutator, listeners, watchers)
  end
  return updateWatchers(self, mutator, watchers)
end

function Store:subscribe(listener)
  assert(type(listener) == "function", "store:subscribe expects a function")

  self._listeners = addListener(self._listeners, listener)
  local active = true

  return function()
    if active then
      active = false
      self._listeners = removeListener(self._listeners, listener)
    end
  end
end

function Store:watch(key, listener)
  assert(type(key) == "string", "store:watch expects a string key")
  assert(type(listener) == "function", "store:watch expects a function")

  local watchers = self._watchers
  local listeners = watchers[key]
  if listeners == nil then
    listeners = {}
  end

  local updated = shallowCopy(watchers)
  updated[key] = addListener(listeners, listener)
  self._watchers = updated
  local active = true

  return function()
    if active then
      active = false
      local current = self._watchers[key]
      if current == nil then
        return
      end
      local remaining = removeListener(current, listener)
      local nextWatchers = shallowCopy(self._watchers)
      if #remaining == 0 then
        nextWatchers[key] = nil
      else
        nextWatchers[key] = remaining
      end
      self._watchers = nextWatchers
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
