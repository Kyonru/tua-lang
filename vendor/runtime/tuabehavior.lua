-- Typed, opt-in behavior trees for Tua and LOVE. Independent implementation.
-- Copyright (c) 2026 Kyonru. MIT License.

---@alias BehaviorStatus "success" | "failure" | "running"
---@class BehaviorNode
---@field kind string
---@class BehaviorRunner
---@field update fun(self: BehaviorRunner, dt: number): BehaviorStatus
---@field reset fun(self: BehaviorRunner)
---@field cancel fun(self: BehaviorRunner)
---@class BehaviorDefinition
---@field instance fun(self: BehaviorDefinition, context: any): BehaviorRunner
---@field setRoot fun(self: BehaviorDefinition, root: BehaviorNode): BehaviorDefinition
---@class BehaviorTree
---@field new fun(...: BehaviorNode): BehaviorDefinition
---@field sequence fun(children: BehaviorNode[]): BehaviorNode
---@field selector fun(children: BehaviorNode[]): BehaviorNode
---@field reactiveSequence fun(children: BehaviorNode[]): BehaviorNode
---@field reactiveSelector fun(children: BehaviorNode[]): BehaviorNode
---@field parallelAll fun(children: BehaviorNode[]): BehaviorNode
---@field parallelAny fun(children: BehaviorNode[]): BehaviorNode
---@field condition fun(test: fun(context: any, dt: number): boolean): BehaviorNode
---@field action fun(tick: fun(context: any, dt: number): BehaviorStatus, ...: any): BehaviorNode
---@field wait fun(seconds: number): BehaviorNode
---@field invert fun(child: BehaviorNode): BehaviorNode
---@field repeatN fun(count: number, child: BehaviorNode): BehaviorNode

local BehaviorTree = {}
local Definition = {}
Definition.__index = Definition
local Runner = {}
Runner.__index = Runner

local function finite(value)
  return type(value) == "number" and value == value and value ~= math.huge and value ~= -math.huge
end

local function node(kind, fields)
  fields.kind = kind
  return fields
end

local function children(kind, values)
  assert(type(values) == "table" and #values > 0, kind .. " needs at least one child")
  local copy = {}
  for i, child in ipairs(values) do
    assert(type(child) == "table" and type(child.kind) == "string", "invalid behavior child")
    copy[i] = child
  end
  return node(kind, { children = copy })
end

function BehaviorTree.sequence(values) return children("sequence", values) end
function BehaviorTree.selector(values) return children("selector", values) end
function BehaviorTree.reactiveSequence(values) return children("reactiveSequence", values) end
function BehaviorTree.reactiveSelector(values) return children("reactiveSelector", values) end
function BehaviorTree.parallelAll(values) return children("parallelAll", values) end
function BehaviorTree.parallelAny(values) return children("parallelAny", values) end
function BehaviorTree.condition(test)
  assert(type(test) == "function", "condition needs a function")
  return node("condition", { test = test })
end
---@param tick fun(context: any, dt: number): BehaviorStatus
---@param enter? fun(context: any)
---@param exit? fun(context: any, reason: string)
---@return BehaviorNode
function BehaviorTree.action(tick, enter, exit)
  assert(type(tick) == "function" and (enter == nil or type(enter) == "function") and (exit == nil or type(exit) == "function"), "invalid action callbacks")
  return node("action", { tick = tick, enter = enter, exit = exit })
end
function BehaviorTree.wait(seconds)
  assert(finite(seconds) and seconds >= 0, "wait seconds must be finite and non-negative")
  return node("wait", { seconds = seconds })
end
function BehaviorTree.invert(child)
  assert(type(child) == "table" and child.kind, "invert needs a child")
  return node("invert", { child = child })
end
function BehaviorTree.repeatN(count, child)
  assert(finite(count) and count == math.floor(count) and count >= 1 and count <= 100000 and type(child) == "table" and child.kind, "repeatN needs a positive bounded count and child")
  return node("repeat", { count = count, child = child })
end

local function finish(runner, current, state, reason)
  if current.kind == "action" and current.exit then current.exit(runner.context, reason) end
  runner.states[current] = nil
  local count = runner.freeCount + 1
  runner.freeCount = count
  runner.freeStates[count] = state
end

local function clear(runner, current, reason)
  local state = runner.states[current]
  if not state then return end
  if current.children then
    for _, child in ipairs(current.children) do clear(runner, child, reason) end
  elseif current.child then clear(runner, current.child, reason) end
  finish(runner, current, state, reason)
end

local function visit(runner, current, dt, depth)
  runner.steps = runner.steps + 1
  assert(runner.steps <= runner.maxSteps and depth <= runner.maxDepth, "behavior tree tick budget exceeded")
  local state = runner.states[current]
  if not state then
    local count = runner.freeCount
    if count > 0 then
      state = runner.freeStates[count]
      runner.freeStates[count] = nil
      runner.freeCount = count - 1
      if state.results then
        for index in pairs(state.results) do state.results[index] = nil end
      end
      state.cursor, state.elapsed, state.repeats = 1, 0, 0
    else
      state = { cursor = 1, elapsed = 0, repeats = 0 }
    end
    runner.states[current] = state
    if current.kind == "action" and current.enter then current.enter(runner.context) end
  end
  local status
  if current.kind == "action" then
    status = current.tick(runner.context, dt)
  elseif current.kind == "condition" then
    local result = current.test(runner.context, dt)
    assert(type(result) == "boolean", "condition must return boolean")
    status = result and "success" or "failure"
  elseif current.kind == "wait" then
    state.elapsed = state.elapsed + dt
    status = state.elapsed >= current.seconds and "success" or "running"
  elseif current.kind == "invert" then
    local result = visit(runner, current.child, dt, depth + 1)
    status = result == "success" and "failure" or result == "failure" and "success" or "running"
  elseif current.kind == "repeat" then
    local result = visit(runner, current.child, dt, depth + 1)
    if result == "failure" then status = "failure"
    elseif result == "running" then status = "running"
    else
      state.repeats = state.repeats + 1
      status = state.repeats >= current.count and "success" or "running"
    end
  elseif current.kind == "parallelAll" or current.kind == "parallelAny" then
    state.results = state.results or {}
    local running = false
    for i, child in ipairs(current.children) do
      local result = state.results[i] or visit(runner, child, dt, depth + 1)
      if result == "running" then running = true else state.results[i] = result end
      if (current.kind == "parallelAll" and result == "failure") or (current.kind == "parallelAny" and result == "success") then
        status = result
        break
      end
    end
    if not status then
      status = running and "running" or (current.kind == "parallelAll" and "success" or "failure")
    end
    if status ~= "running" then
      for _, child in ipairs(current.children) do clear(runner, child, status) end
    end
  elseif current.kind == "sequence" or current.kind == "selector" or current.kind == "reactiveSequence" or current.kind == "reactiveSelector" then
    local reactive = current.kind == "reactiveSequence" or current.kind == "reactiveSelector"
    local sequence = current.kind == "sequence" or current.kind == "reactiveSequence"
    local previous = state.cursor
    if reactive then state.cursor = 1 end
    while state.cursor <= #current.children do
      local result = visit(runner, current.children[state.cursor], dt, depth + 1)
      if result == "running" then status = "running"; break end
      if (sequence and result == "failure") or (not sequence and result == "success") then
        status = result; break
      end
      state.cursor = state.cursor + 1
    end
    if reactive and previous ~= state.cursor then clear(runner, current.children[previous], "interrupted") end
    status = status or (sequence and "success" or "failure")
  else error("unknown behavior node kind: " .. tostring(current.kind)) end
  assert(status == "success" or status == "failure" or status == "running", "behavior action must return success, failure, or running")
  if status ~= "running" then finish(runner, current, state, status) end
  return status
end

function Runner:update(dt)
  assert(finite(dt) and dt >= 0, "behavior dt must be finite and non-negative")
  assert(not self.updating, "behavior runner cannot update recursively")
  self.steps = 0
  self.updating = true
  local ok, result = pcall(visit, self, self.definition.root, dt, 1)
  self.updating = false
  if not ok then
    clear(self, self.definition.root, "error")
    error(result, 2)
  end
  return result
end
function Runner:reset()
  assert(not self.updating, "cannot reset during a behavior update")
  clear(self, self.definition.root, "reset")
end
function Runner:cancel()
  assert(not self.updating, "cannot cancel during a behavior update")
  clear(self, self.definition.root, "cancel")
end
function Definition:instance(context)
  assert(self.root, "behavior tree needs a root node")
  self.locked = true
  return setmetatable({ definition = self, context = context, states = {}, freeStates = {}, freeCount = 0, steps = 0, maxSteps = 1000, maxDepth = 64 }, Runner)
end
function Definition:setRoot(root)
  assert(type(root) == "table" and root.kind and not self.locked, "root must be a node and cannot change after instancing")
  local seen = {}
  local function validate(current, depth)
    assert(depth <= 64 and not seen[current], "behavior tree has a cycle or shared node")
    seen[current] = true
    if current.children then
      for _, child in ipairs(current.children) do validate(child, depth + 1) end
    elseif current.child then validate(current.child, depth + 1) end
  end
  validate(root, 1)
  self.root = root
  return self
end
---@param root? BehaviorNode
---@return BehaviorDefinition
function BehaviorTree.new(root)
  local definition = setmetatable({}, Definition)
  if root then definition:setRoot(root) end
  return definition
end

return BehaviorTree
