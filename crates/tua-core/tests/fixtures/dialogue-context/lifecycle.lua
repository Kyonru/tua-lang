local Save = require('tuasave')
local Dialogue = require('tuadialogue')
local definition = Dialogue.context({
  facts = Save.shape(Save.record({
    name = Save.string(''),
    nick = Save.optional(Save.string('')),
  })),
  locals = Save.shape(Save.record({ visits = Save.number(0), flag = Save.boolean(false) })),
})
local document = {
  schemaVersion = 2,
  context = definition,
  bindings = {
    { name = 'name', path = 'context.name' },
    { name = 'nick', path = 'context.nick', fallback = 'friend' },
    { name = 'visits', path = 'locals.visits' },
    { name = 'flag', path = 'locals.flag' },
  },
  entry = 'hello',
  nodes = {
    hello = {
      kind = 'line',
      text = 'Hello %{name} %{nick} %{visits} %{flag}',
      next = 'hello',
      events = { { name = 'tick', payload = '', anchor = 'shown', delay = 1 } },
    },
  },
}
local p = definition.new(document, { name = 'A' })
local q = definition.new(document, { name = 'B' })
p:start()
assert(p:snapshot().node.text == 'Hello A friend 0 false')
assert(#p:update(1, false) == 1)
p:setLocals({ visits = 4, flag = true })
assert(p:snapshot().node.text == 'Hello A friend 4 true')
assert(#p:update(1, true) == 0)
assert(q:getLocals().visits == 0)
p:updateContext({ name = '%{raw}' })
assert(p:snapshot().node.text == 'Hello %{raw} friend 4 true')
p:advance()
assert(p:getLocals().visits == 4)
p:start()
assert(p:getLocals().visits == 0)
assert(p:getContext().name == '%{raw}')

local function fails(action)
  local ok = pcall(action)
  assert(not ok, 'expected rejection')
end

local before = p:snapshot()
for _, invalid in ipairs({
  { name = 3 },
  { name = function() end },
  { name = 'A', unknown = 1 },
  setmetatable({ name = 'A' }, {}),
}) do
  fails(function() p:updateContext(invalid) end)
  assert(p:snapshot().presentationRevision == before.presentationRevision)
end
local changed = p:getContext()
changed.name = 'detached'
assert(p:getContext().name == '%{raw}')
local current = p:getLocals()
current.visits = 55
assert(p:getLocals().visits == 0)
assert(p:cancel())
assert(p:getContext().name == '%{raw}')
p:start()
assert(p:getLocals().visits == 0)
p:dispose()
fails(function() p:updateContext({ name = 'C' }) end)
fails(function() p:getLocals() end)
assert(p.facts == nil and p.locals == nil and p.localDefaults == nil)
assert(p.copyFacts == nil and p.copyLocals == nil and p.bindings == nil)
fails(function() Dialogue.new(document) end)

local defaultArray = { 'initial' }
local shape = Save.shape(Save.record({ items = Save.array(Save.string(''), defaultArray) }))
defaultArray[1] = 'outside'
assert(shape.defaults.items[1] == 'initial')
local owned = shape.copy(shape.defaults)
owned.items[1] = 'changed'
assert(shape.defaults.items[1] == 'initial')
fails(function() shape.copy({ items = { [2] = 'hole' } }) end)
fails(function() shape.copy({ items = { 0 / 0 } }) end)
fails(function() shape.copy({ items = { function() end } }) end)
local cyclic = {}
cyclic.items = cyclic
fails(function() shape.copy(cyclic) end)
fails(function() Save.shape(Save.string('')) end)

-- Failed resolver refreshes preserve facts, locals and presentation identity.
local reject = false
local localized
localized = definition.new(document, { name = 'A' }, {
  locale = 'en',
  resolve = function(_, source, args)
    if reject then error('resolver failure') end
    if localized then
      fails(function() localized:setLocals({ visits = 9, flag = false }) end)
    end
    return require('tuadialogue.text').format(source, args)
  end,
})
-- Hand-authored test documents have no textId until explicitly added.
local localizedDocument = {}
for key, value in pairs(document) do localizedDocument[key] = value end
localizedDocument.nodes = {
  hello = {
    kind = 'line',
    textId = 'hello',
    text = '%{name}',
    next = 'hello',
  },
}
localized = definition.new(localizedDocument, { name = 'A' }, {
  locale = 'en',
  resolve = function(_, source, args)
    if reject then error('resolver failure') end
    if localized then fails(function() localized:cancel() end) end
    return require('tuadialogue.text').format(source, args)
  end,
})
localized:start()
local accepted = localized:snapshot()
reject = true
fails(function() localized:updateContext({ name = 'B' }) end)
assert(localized:getContext().name == 'A')
assert(localized:snapshot().presentationRevision == accepted.presentationRevision)
assert(localized:snapshot().node.text == 'A')
reject = false
localized:updateContext({ name = 'C' })
assert(localized:snapshot().node.text == 'C')
print('context lifecycle passed')

-- Public shape fields cannot inject a validator or mutate captured defaults.
local safe = Save.shape(Save.record({ name = Save.string("safe") }))
safe.defaults.name = "external"
safe.copy = function() error("must not execute") end
local immutable = Dialogue.context({ facts = safe, locals = safe })
assert(immutable.locals.name == "safe")
fails(function()
  Dialogue.context({ facts = { defaults = {}, copy = function() return {} end }, locals = safe })
end)
