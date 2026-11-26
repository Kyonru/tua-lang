-- Deliberately incomplete test catalog; shipped files remain complete.
local french = require("locales.fr")
french.fr.ui.restLine = nil
local I18n = require("tuai18n")
local Conversation = require("conversation")
local starts = 0
local conversation = Conversation.new(function()
  starts = starts + 1
  return true
end)
conversation.start()
local before = conversation.player:snapshot()
I18n.setLocale("fr")
for _ = 1, 10 do
  conversation.locale()
end
local after = conversation.player:snapshot()
assert(before.visit == after.visit and before.generation == after.generation)
assert(after.node.text:find("Vous avez", 1, true) and after.node.speaker == "Gardienne du port")
assert(starts == 0)
conversation.update(0, false, 1)
assert(conversation.state.selected == 1, "line navigation must not break initial focus")
conversation.update(0, true, 0)
assert(conversation.player:snapshot().state == "awaiting_choice")
conversation.update(0, false, 1)
assert(conversation.state.selected == 2)
I18n.setLocale("en")
conversation.locale()
assert(conversation.state.selected == 2)
I18n.setLocale("fr")
conversation.locale()
conversation.update(0, true, 0)
assert(
  conversation.player:snapshot().node.text == "Take your time. The harbor can wait.",
  "missing French key falls back to English"
)
conversation.update(0, true, 0)
assert(not conversation.state.active and starts == 0)
conversation.start()
conversation.update(0, true, 0)
conversation.update(0, true, 0)
local visit = conversation.player:snapshot().visit
conversation.locale()
assert(conversation.player:snapshot().visit == visit and starts == 0)
conversation.update(0, true, 0)
for _ = 1, 10 do
  conversation.update(0, true, 0)
end
assert(starts == 1 and conversation.state.wavesStarted == 1)
conversation.start()
conversation.update(0, true, 0)
conversation.update(0, true, 0)
conversation.close()
conversation.update(0, true, 0)
assert(starts == 1, "closing a ready line must cancel its pending host request")
assert(I18n.t("ui.stats", {
  health = 7,
  wave = 2,
  enemies = 3,
  hits = 4,
  shots = 1,
}):find("Vie 7/10", 1, true))
assert(I18n.dialogue("absent.key", "Authored fallback", {}, "fr") == "Authored fallback")
assert(I18n.t("ui.speaker", nil, "fr-CA") == "Gardienne du port")
conversation.dispose()
conversation.dispose()
conversation.start()
assert(not conversation.state.active)
print(
  "ARENA_CONVERSATION_OK locale identity, focus, stable replies, fallback, one-shot wave request, cancellation"
)
