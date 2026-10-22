--
-- tuatween: the runtime behind Tua's built-in `tween` module.
--
-- This is flux 0.1.5 by rxi (https://github.com/rxi/flux, commit
-- bb330231b87eabf84fbd68322f13a6320db30a41), unchanged apart from the LuaDoc
-- annotations and the typed export table at the end. Tua types the `tween`
-- global from these annotations and copies this file into the build when a
-- project uses it. A project that provides its own tuatween.lua replaces it.
--
-- Copyright (c) 2016 rxi
--
-- Permission is hereby granted, free of charge, to any person obtaining a copy
-- of this software and associated documentation files (the "Software"), to
-- deal in the Software without restriction, including without limitation the
-- rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
-- sell copies of the Software, and to permit persons to whom the Software is
-- furnished to do so, subject to the following conditions:
--
-- The above copyright notice and this permission notice shall be included in
-- all copies or substantial portions of the Software.
--
-- THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
-- IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
-- FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
-- AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
-- LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
-- FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
-- IN THE SOFTWARE.
--

---@alias TweenEasing
---| "linear"
---| "quadin" | "quadout" | "quadinout"
---| "cubicin" | "cubicout" | "cubicinout"
---| "quartin" | "quartout" | "quartinout"
---| "quintin" | "quintout" | "quintinout"
---| "expoin" | "expoout" | "expoinout"
---| "sinein" | "sineout" | "sineinout"
---| "circin" | "circout" | "circinout"
---| "backin" | "backout" | "backinout"
---| "elasticin" | "elasticout" | "elasticinout"

---A running tween. Setters return the tween, so calls can be chained.
---@class Tween
---@field obj any the table whose numeric fields are animated
---@field progress number completion from 0 to 1
---@field ease fun(self: Tween, easing: TweenEasing): Tween
---@field delay fun(self: Tween, seconds: number): Tween
---@field onstart fun(self: Tween, callback: fun()): Tween
---@field onupdate fun(self: Tween, callback: fun()): Tween
---@field oncomplete fun(self: Tween, callback: fun()): Tween
---@field after fun(self: Tween, target: any, seconds: number | { [string]: number }, values?: { [string]: number }): Tween
---@field stop fun(self: Tween)

---An independent set of tweens, for example one per scene or for UI that
---keeps animating while gameplay is paused.
---@class TweenGroup
---@field to fun(self: TweenGroup, target: any, seconds: number, values: { [string]: number }): Tween
---@field update fun(self: TweenGroup, dt: number)
---@field remove fun(self: TweenGroup, tween: Tween)

local flux = { _version = "0.1.5" }
flux.__index = flux

flux.tweens = {}
flux.easing = { linear = function(p) return p end }

local easing = {
  quad    = "p * p",
  cubic   = "p * p * p",
  quart   = "p * p * p * p",
  quint   = "p * p * p * p * p",
  expo    = "2 ^ (10 * (p - 1))",
  sine    = "-math.cos(p * (math.pi * .5)) + 1",
  circ    = "-(math.sqrt(1 - (p * p)) - 1)",
  back    = "p * p * (2.7 * p - 1.7)",
  elastic = "-(2^(10 * (p - 1)) * math.sin((p - 1.075) * (math.pi * 2) / .3))"
}

local makefunc = function(str, expr)
  local load = loadstring or load
  return load("return function(p) " .. str:gsub("%$e", expr) .. " end")()
end

for k, v in pairs(easing) do
  flux.easing[k .. "in"] = makefunc("return $e", v)
  flux.easing[k .. "out"] = makefunc([[
    p = 1 - p
    return 1 - ($e)
  ]], v)
  flux.easing[k .. "inout"] = makefunc([[
    p = p * 2
    if p < 1 then
      return .5 * ($e)
    else
      p = 2 - p
      return .5 * (1 - ($e)) + .5
    end
  ]], v)
end



local tween = {}
tween.__index = tween

local function makefsetter(field)
  return function(self, x)
    local mt = getmetatable(x)
    if type(x) ~= "function" and not (mt and mt.__call) then
      error("expected function or callable", 2)
    end
    local old = self[field]
    self[field] = old and function() old() x() end or x
    return self
  end
end

local function makesetter(field, checkfn, errmsg)
  return function(self, x)
    if checkfn and not checkfn(x) then
      error(errmsg:gsub("%$x", tostring(x)), 2)
    end
    self[field] = x
    return self
  end
end

tween.ease  = makesetter("_ease",
                         function(x) return flux.easing[x] end,
                         "bad easing type '$x'")
tween.delay = makesetter("_delay",
                         function(x) return type(x) == "number" end,
                         "bad delay time; expected number")
tween.onstart     = makefsetter("_onstart")
tween.onupdate    = makefsetter("_onupdate")
tween.oncomplete  = makefsetter("_oncomplete")


function tween.new(obj, time, vars)
  local self = setmetatable({}, tween)
  self.obj = obj
  self.rate = time > 0 and 1 / time or 0
  self.progress = time > 0 and 0 or 1
  self._delay = 0
  self._ease = "quadout"
  self.vars = {}
  for k, v in pairs(vars) do
    if type(v) ~= "number" then
      error("bad value for key '" .. k .. "'; expected number")
    end
    self.vars[k] = v
  end
  return self
end


function tween:init()
  for k, v in pairs(self.vars) do
    local x = self.obj[k]
    if type(x) ~= "number" then
      error("bad value on object key '" .. k .. "'; expected number")
    end
    self.vars[k] = { start = x, diff = v - x }
  end
  self.inited = true
end


function tween:after(...)
  local t
  if select("#", ...) == 2 then
    t = tween.new(self.obj, ...)
  else
    t = tween.new(...)
  end
  t.parent = self.parent
  self:oncomplete(function() flux.add(self.parent, t) end)
  return t
end


function tween:stop()
  flux.remove(self.parent, self)
end



function flux.group()
  return setmetatable({}, flux)
end


function flux:to(obj, time, vars)
  return flux.add(self, tween.new(obj, time, vars))
end


function flux:update(deltatime)
  for i = #self, 1, -1 do
    local t = self[i]
    if t._delay > 0 then
      t._delay = t._delay - deltatime
    else
      if not t.inited then
        flux.clear(self, t.obj, t.vars)
        t:init()
      end
      if t._onstart then
        t._onstart()
        t._onstart = nil
      end
      t.progress = t.progress + t.rate * deltatime
      local p = t.progress
      local x = p >= 1 and 1 or flux.easing[t._ease](p)
      for k, v in pairs(t.vars) do
        t.obj[k] = v.start + x * v.diff
      end
      if t._onupdate then t._onupdate() end
      if p >= 1 then
        flux.remove(self, i)
        if t._oncomplete then t._oncomplete() end
      end
    end
  end
end


function flux:clear(obj, vars)
  for t in pairs(self[obj]) do
    if t.inited then
      for k in pairs(vars) do t.vars[k] = nil end
    end
  end
end


function flux:add(tween)
  -- Add to object table, create table if it does not exist
  local obj = tween.obj
  self[obj] = self[obj] or {}
  self[obj][tween] = true
  -- Add to array
  table.insert(self, tween)
  tween.parent = self
  return tween
end


function flux:remove(x)
  if type(x) == "number" then
    -- Remove from object table, destroy table if it is empty
    local obj = self[x].obj
    self[obj][self[x]] = nil
    if not next(self[obj]) then self[obj] = nil end
    -- Remove from array
    self[x] = self[#self]
    return table.remove(self)
  end
  for i, v in ipairs(self) do
    if v == x then
      return flux.remove(self, i)
    end
  end
end



-- Typed exports. Upstream flux returns a table with `to`, `update`, and
-- `remove` bound to the default group and `flux` as its metatable; these
-- exports keep that API while giving Tua a visible module surface.

---Create an independent group, updated with `group:update(dt)`.
---@return TweenGroup
local function group()
  return flux.group()
end

---Animate the numeric `values` of `target` over `seconds` in the default
---group. Each field must already hold a number.
---@param target any a table whose numeric fields are animated
---@param seconds number
---@param values { [string]: number }
---@return Tween
local function to(target, seconds, values)
  return flux.to(flux.tweens, target, seconds, values)
end

---Advance every tween in the default group. Call once per frame.
---@param dt number
local function update(dt)
  flux.update(flux.tweens, dt)
end

---Stop a tween in the default group without completing it.
---@param tween Tween
local function remove(tween)
  flux.remove(flux.tweens, tween)
end

---Easing functions by name, mapping progress from 0 to 1 onto eased progress.
---@type { [string]: fun(p: number): number }
local easings = flux.easing

return {
  _version = flux._version,
  easing = easings,
  group = group,
  to = to,
  update = update,
  remove = remove,
}
