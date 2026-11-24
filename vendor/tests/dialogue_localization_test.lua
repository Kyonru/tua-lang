-- Run from examples/dialogue-localization/build against emitted modules.
local Dialogue = require("tuadialogue")
local I18n = require("tuai18n")
local scene = require("scene")
local function fails(fn, message)
  local ok, err = pcall(fn)
  assert(not ok and tostring(err):find(message, 1, true), tostring(err))
end
assert(I18n.dialogue("absent", "Hi %{name}, 100%%!", { name = "%s %{count} 雨" }, "fr") == "Hi %s %{count} 雨, 100%!")
assert(I18n.dialogue("absent", "%%{name} %{flag}", {
  flag = false,
  unused = 4,
}, "en") == "%{name} false")
fails(function() I18n.dialogue("absent", "%{name}", {}, "en") end, "missing or invalid")
fails(function()
  I18n.dialogue("harbor.hello", "%{name} %{count}", { name = "A" }, "en")
end, "finite count")
local source = {
  schemaVersion = 1,
  documentId = "test",
  entry = "s",
  nodes = {
    s = { kind = "line", textId = "absent", text = "%{name}", next = "e" },
    e = { kind = "end" },
  },
}
local args = { name = "original" }
local one = Dialogue.new(source, { locale = "fr", resolve = I18n.dialogue, args = args })
local two = Dialogue.new(source)
args.name = "mutated"
source.nodes.s.text = "mutated"
one:start()
two:start()
assert(one:snapshot().node.text == "original")
assert(two:snapshot().node.text == "%{name}")
local snapshot = one:snapshot()
snapshot.node.text = "mutated"
assert(one:snapshot().node.text == "original")
assert(not one:setLocale("fr"))
assert(one:setLocale("en"))
assert(one:snapshot().documentId == "test")
one:cancel()
assert(not one:snapshot().node)
one:start()
assert(not one:snapshot().revealComplete)
one:dispose()
fails(function() one:setLocale("en") end, "disposed")
source.schemaVersion = 99
fails(function() Dialogue.new(source) end, "unsupported dialogue schemaVersion")
source.schemaVersion = 1
source.typo = true
fails(function() Dialogue.new(source) end, "unknown dialogue field")
source.schemaVersion = nil
assert(Dialogue.new(source)) -- legacy ignores unknown fields
local broken = Dialogue.new(scene, {
  locale = "en",
  args = {},
  resolve = function()
    error("resolver failed")
  end,
})
fails(function() broken:start() end, "resolver failed")
assert(broken:snapshot().visit == 0 and broken:snapshot().generation == 0)
print("dialogue localization lifecycle passed")

local smiti = require("smiti18n.init")
smiti.set("en.bad", { "Array is not a line" })
fails(function() I18n.dialogue("bad", "Source", {}, "en") end, "must be text or a plural entry")
smiti.set("en.bad", "Extra %{wrong}")
fails(function() I18n.dialogue("bad", "Source", {}, "en") end, "extra dialogue placeholder")
smiti.set("en.bad", "Missing")
fails(function()
  I18n.dialogue("bad", "%{name}", { name = "A" }, "en")
end, "missing dialogue placeholder")
smiti.set("en.bad", { one = "One %{count}", other = "Other %{count}" })
assert(I18n.dialogue("bad", "%{count}", { count = 2 }, "ru") == "Other 2")
local reentrant
reentrant = Dialogue.new(scene, {
  locale = "en",
  args = {},
  resolve = function()
    reentrant:dispose()
    return "invalid"
  end,
})
fails(function() reentrant:start() end, "cannot mutate dialogue while resolving text")
assert(reentrant:snapshot().state == "idle")
reentrant:dispose()
-- Updating another player is allowed; nested resolver calls cannot affect locale.
local sibling = Dialogue.new(scene)
local nested = Dialogue.new(scene, {
  locale = "fr",
  args = { name = "A", count = 1 },
  resolve = function(key, source, args, locale)
    sibling:start()
    return I18n.dialogue(key, source, args, locale)
  end,
})
nested:start()
assert(sibling:snapshot().state == "awaiting_advance")
assert(I18n.getLocale() == "en")

-- Template selection never substitutes, changes global locale, or exposes catalog tables.
local selected = I18n.dialogueTemplate("harbor.hello", "%{name} %{count}", {
  name = "[pause=60] %{count}",
  count = 1,
}, "fr-CA")
assert(selected.locale == "fr" and selected.variant == "one")
assert(selected.template == "Bonjour, %{name} ! Vous avez %{count} pièce.")
selected.template = "changed"
assert(I18n.dialogueTemplate("harbor.hello", "%{name} %{count}", { count = 1 }, "fr").template ~= "changed")
local authored = I18n.dialogueTemplate("absent", "%%{literal} %{name}", {}, "ja")
assert(authored.template == "%%{literal} %{name}")
assert(authored.locale == nil and authored.variant == nil)
local fallback = I18n.dialogueTemplate("harbor.bye", "%{name}", {}, "fr-CA")
assert(fallback.locale == "en" and fallback.variant == nil)
smiti.set("en.only_other", { other = "%{count} items" })
local other = I18n.dialogueTemplate("only_other", "%{count}", { count = 1 }, "en")
assert(other.variant == "other" and other.template == "%{count} items")
for _, resolve in ipairs({ I18n.dialogue, I18n.dialogueTemplate }) do
  fails(function()
    resolve("harbor.hello", "%{name} %{count}", { count = math.huge }, "en")
  end, "finite count")
  fails(function()
    resolve("harbor.hello", "%{name}", { count = 1 }, "en")
  end, "extra dialogue placeholder")
  fails(function()
    resolve("harbor.bye", "%{name} %{extra}", {}, "en")
  end, "missing dialogue placeholder")
  smiti.set("en.invalid_template", { "not scalar" })
  fails(function()
    resolve("invalid_template", "plain", {}, "en")
  end, "must be text or a plural entry")
end
assert(I18n.getLocale() == "en")
print("unexpanded dialogue templates passed")
