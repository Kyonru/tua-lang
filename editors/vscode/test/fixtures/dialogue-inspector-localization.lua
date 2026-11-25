package.path = arg[2] .. '/?.lua;' .. package.path
package.preload.tuai18n_config = function()
  return { catalogs = {}, default_locale = 'en', fallback_locale = 'en' }
end
local I18n = require('tuai18n')
local smiti = require('smiti18n.init')
smiti.set('fr.greeting', 'PRIVATE traduction')
smiti.set('en.fallback', 'PRIVATE fallback')
local readers = setmetatable({}, { __mode = 'k' })
local inspect = {
  register = function(_, player, read)
    readers[player] = read
  end,
}
local instrument = assert(loadfile(arg[1] .. '/dialogue.lua'))()(inspect, {}, {})
local Dialogue = instrument(require('tuadialogue'))
local function player(key, resolver, plain)
  local result = Dialogue.new({
    schemaVersion = 8,
    entry = 's',
    nodes = {
      s = {
        kind = 'line',
        textId = key,
        text = 'PRIVATE source',
        richText = '[em]PRIVATE source[/em]',
        next = 'e',
      },
      e = { kind = 'end' },
    },
  }, { locale = 'fr', resolveTemplate = resolver, resolve = plain })
  result:start()
  return result
end
local function sample(value)
  return readers[value](value).localization
end
local function status(value)
  return sample(value).fields[1].status
end
local exact = player('greeting', I18n.dialogueTemplate)
assert(status(exact) == 'requested')
exact:setLocale('fr-CA')
assert(status(exact) == 'locale_fallback')
assert(sample(exact).fields[1].resolvedLocale == 'fr')
assert(status(player('fallback', I18n.dialogueTemplate)) == 'locale_fallback')
assert(status(player('missing', I18n.dialogueTemplate)) == 'source_fallback')
assert(status(player('missing', nil, I18n.dialogue)) == 'unknown')
assert(status(player('missing')) == 'unlocalized')
local calls = 0
local custom = player('missing', function(_, source)
  calls = calls + 1
  return { template = source }
end)
assert(status(custom) == 'unknown')
assert(status(custom) == 'unknown' and calls == 1)
local choices = Dialogue.new({
  schemaVersion = 8,
  entry = 's',
  nodes = {
    s = {
      kind = 'choice',
      fallback = 'e',
      options = {
        {
          id = 'greeting',
          text = 'PRIVATE choice',
          richText = '[em]PRIVATE choice[/em]',
          target = 'e',
        },
        {
          id = 'missing',
          text = 'PRIVATE choice',
          richText = '[em]PRIVATE choice[/em]',
          target = 'e',
          enabled = { { op = 'value', value = false } },
          reason = 'PRIVATE reason',
          richReason = '[em]PRIVATE reason[/em]',
          reasonId = 'fallback',
        },
      },
    },
    e = { kind = 'end' },
  },
}, { locale = 'fr', resolveTemplate = I18n.dialogueTemplate })
choices:start()
local fields = sample(choices).fields
assert(#fields == 3)
assert(fields[1].status == 'requested')
assert(fields[2].status == 'source_fallback')
assert(fields[3].field == 'reason 2' and fields[3].status == 'locale_fallback')
-- Bound raw presentation sampling independently of runtime choice filtering.
exact.presentation = { kind = 'choice', options = {} }
for index = 1, 40 do
  exact.presentation.options[index] = {
    id = 'choice.' .. index,
    text = 'PRIVATE choice',
    presentation = { resolvedLocale = 'fr', variant = 'other' },
    reasonId = 'reason.' .. index,
    reasonPresentation = { resolvedLocale = 'en' },
  }
end
local bounded = sample(exact)
assert(#bounded.fields == 64 and bounded.truncated)
assert(bounded.fields[1].field == 'choice 1')
assert(bounded.fields[1].variant == 'other')
assert(bounded.fields[2].field == 'reason 1')
for _, entry in ipairs(bounded.fields) do
  for _, value in pairs(entry) do
    assert(type(value) ~= 'string' or not value:find('PRIVATE', 1, true))
  end
end
exact.presentation = setmetatable({}, { __index = function() error('must use raw fields') end })
assert(#sample(exact).fields == 0)
custom:dispose()
assert(sample(custom) == nil)
print('dialogue localization inspection passed')
