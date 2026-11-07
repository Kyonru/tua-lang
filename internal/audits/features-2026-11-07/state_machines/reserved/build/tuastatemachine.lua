-- lua-state-machine
-- https://github.com/kyleconroy/lua-state-machine
--
-- Copyright (c) 2012 Kyle Conroy
--
-- Permission is hereby granted, free of charge, to any person obtaining a copy
-- of this software and associated documentation files (the "Software"), to deal
-- in the Software without restriction, including without limitation the rights
-- to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
-- copies of the Software, and to permit persons to whom the Software is
-- furnished to do so, subject to the following conditions:
--
-- The above copyright notice and this permission notice shall be included in
-- all copies or substantial portions of the Software.
--
-- THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
-- IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
-- FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
-- AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
-- LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
-- OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
-- SOFTWARE.

local machine = {}
machine.__index = machine

local NONE = "none"
local ASYNC = "async"

-- Compatibility for Lua 5.1 vs 5.2+.
local unpack = unpack or table.unpack

-- Callback names depend only on the state, so build each one once.
local function name_cache(prefix)
  return setmetatable({}, {
    __index = function(cache, state)
      local name = prefix .. state
      cache[state] = name
      return name
    end,
  })
end

local LEAVE = name_cache("onleave")
local ENTER = name_cache("onenter")
local ON = name_cache("on")

-- Transitions without extra arguments call handlers directly; only extra
-- arguments are kept in a table, so asynchronous resumption still sees them.
local function call_handler(handler, self, name, from, to, extra)
  if handler then
    if extra == nil then
      return handler(self, name, from, to)
    end
    return handler(self, name, from, to, unpack(extra, 1, extra.n))
  end
end

local function create_transition(name)
  local can, to, from, extra
  local beforeCallback = "onbefore" .. name
  local afterCallback = "onafter" .. name
  local eventCallback = "on" .. name
  local waitingOnLeave = name .. "WaitingOnLeave"
  local waitingOnEnter = name .. "WaitingOnEnter"

  local step

  -- Starting a transition checks `can` and records extra arguments here, so
  -- the recursive steps below are not vararg calls. An asynchronous
  -- resumption (`fsm:transition(name)`) keeps the original arguments.
  local function transition(self, ...)
    local state = self.asyncState
    if state == waitingOnLeave or state == waitingOnEnter then
      return step(self)
    end
    local interrupted = false
    if state ~= NONE then
      if not (string.find(state, "WaitingOnLeave") or string.find(state, "WaitingOnEnter")) then
        self.currentTransitioningEvent = nil
        return false
      end
      self.asyncState = NONE
      interrupted = true
    end
    can, to = self:can(name)
    if not can then
      return interrupted
    end
    if select("#", ...) == 0 then
      extra = nil
    else
      extra = { n = select("#", ...), ... }
    end
    local result = step(self)
    return interrupted or result
  end

  function step(self)
    if self.asyncState == NONE then
      from = self.current
      self.currentTransitioningEvent = name

      local beforeReturn = call_handler(self[beforeCallback], self, name, from, to, extra)
      local leaveReturn = call_handler(self[LEAVE[from]], self, name, from, to, extra)

      if beforeReturn == false or leaveReturn == false then
        self.currentTransitioningEvent = nil
        return false
      end

      self.asyncState = waitingOnLeave

      if leaveReturn ~= ASYNC then
        step(self)
      end

      return true
    elseif self.asyncState == waitingOnLeave then
      self.current = to

      local enterReturn = call_handler(self[ENTER[to]] or self[ON[to]], self, name, from, to, extra)

      self.asyncState = waitingOnEnter

      if enterReturn ~= ASYNC then
        step(self)
      end

      return true
    end

    call_handler(self[afterCallback] or self[eventCallback], self, name, from, to, extra)
    call_handler(self.onstatechange, self, name, from, to, extra)
    self.asyncState = NONE
    self.currentTransitioningEvent = nil
    return true
  end

  return transition
end

local function add_to_map(map, event)
  if type(event.from) == "string" then
    map[event.from] = event.to
  else
    for _, from in ipairs(event.from) do
      map[from] = event.to
    end
  end
end

function machine.create(options)
  assert(options.events)

  local fsm = {}
  setmetatable(fsm, machine)

  fsm.options = options
  fsm.current = options.initial or "none"
  fsm.asyncState = NONE
  fsm.events = {}

  for _, event in ipairs(options.events or {}) do
    local name = event.name
    fsm[name] = fsm[name] or create_transition(name)
    fsm.events[name] = fsm.events[name] or { map = {} }
    add_to_map(fsm.events[name].map, event)
  end

  for name, callback in pairs(options.callbacks or {}) do
    fsm[name] = callback
  end

  return fsm
end

function machine:is(state)
  return self.current == state
end

function machine:can(event_name)
  local event = self.events[event_name]
  local to = event and (event.map[self.current] or event.map["*"])
  return to ~= nil, to
end

function machine:cannot(event_name)
  return not self:can(event_name)
end

function machine:todot(filename)
  local dotfile = io.open(filename, "w")
  assert(dotfile ~= nil)
  dotfile:write("digraph {\n")

  local transition = function(event, from, to)
    dotfile:write(string.format("%s -> %s [label=%s];\n", from, to, event))
  end

  for _, event in pairs(self.options.events) do
    if type(event.from) == "table" then
      for _, from in ipairs(event.from) do
        transition(event.name, from, event.to)
      end
    else
      transition(event.name, event.from, event.to)
    end
  end
  dotfile:write("}\n")
  dotfile:close()
end

function machine:transition(event)
  if self.currentTransitioningEvent == event then
    return self[self.currentTransitioningEvent](self)
  end
end

function machine:cancelTransition(event)
  if self.currentTransitioningEvent == event then
    self.asyncState = NONE
    self.currentTransitioningEvent = nil
  end
end

machine.NONE = NONE
machine.ASYNC = ASYNC

return machine
