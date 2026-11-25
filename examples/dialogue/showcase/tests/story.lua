local Controller = require("controller")
local function noop() end
local function backend()
  return {
    stop = noop,
    pause = noop,
    resume = noop,
    dispose = noop,
    volume = noop,
    voice = function() return false end,
    speaking = function() return false end,
    chatter = function() return false end,
    wordStart = function() return true end,
  }
end
local function new()
  local host = Controller.new(backend(), backend())
  host.settings.reducedMotion = true
  return host
end
local function boundary(host)
  for step = 1, 100 do
    host.update(1)
    local state = host.player():snapshot().state
    if state == "awaiting_choice" or state == "completed" then return state end
    assert(state ~= "faulted", "story must not fault")
    host.advance()
  end
  error("story did not reach an input boundary")
end
local function choose(host, text)
  assert(boundary(host) == "awaiting_choice")
  for index, option in ipairs(host.player():snapshot().node.options) do
    if option.text:find(text, 1, true) then
      assert(option.enabled ~= false, "chosen story route must be available")
      host.settings.selected = index
      host.advance()
      return
    end
  end
  error("missing reply: " .. text)
end
local trusted = new()
choose(trusted, "Who is on")
choose(trusted, "believe him")
assert(trusted.player():getLocals().trusted)
assert(trusted.save())
trusted.restore()
assert(trusted.player():getLocals().trusted, "checkpoint retains earlier relationship choices")
choose(trusted, "Return to Mara")
choose(trusted, "decide")
choose(trusted, "Light the beacon")
choose(trusted, "Listen")
choose(trusted, "Stay beside")
assert(boundary(trusted) == "completed")
local sawCallback = false
for _, entry in ipairs(trusted.player():history().entries) do
  sawCallback = sawCallback or entry.text:find("You said you believed me", 1, true) ~= nil
end
assert(sawCallback, "earlier kindness selects the trusted ending variant")
trusted.reset()
assert(not trusted.player():getLocals().trusted, "restarting clears story-local memory")
trusted.dispose()
local reserved = new()
choose(reserved, "decide")
assert(boundary(reserved) == "awaiting_choice")
assert(reserved.player():snapshot().node.options[2].enabled == false, "chart gates the skiff")
choose(reserved, "Light the beacon")
choose(reserved, "Listen")
choose(reserved, "Stay beside")
assert(boundary(reserved) == "completed")
local sawReserved = false
for _, entry in ipairs(reserved.player():history().entries) do
  sawReserved = sawReserved or entry.text:find("I should go and tell", 1, true) ~= nil
end
assert(sawReserved, "neutral relationship has its own ending variant")
reserved.dispose()
local skiff = new()
choose(skiff, "chart")
choose(skiff, "Memorize")
assert(skiff.player():getLocals().charted)
choose(skiff, "Return")
assert(boundary(skiff) == "awaiting_choice")
assert(#skiff.player():snapshot().node.options == 5, "learning the chart reveals an extra reply")
choose(skiff, "decide")
assert(boundary(skiff) == "awaiting_choice")
skiff.player():updateContext({ name = "Ada", permit = false })
assert(skiff.player():snapshot().node.options[2].enabled == false)
skiff.player():updateContext({ name = "Ada", permit = true })
choose(skiff, "skiff")
assert(boundary(skiff) == "completed")
skiff.dispose()
local localized = new()
localized.locale()
assert(localized.player():snapshot().node.text:find("Tu es revenu", 1, true))
local document = require("scene")
local translations = require("translations")
for _, node in pairs(document.nodes) do
  if node.textId then
    assert(translations.resolve(node.textId, node.text or "", {}, "fr").locale == "fr", node.textId)
  end
  for _, option in ipairs(node.options or {}) do
    assert(translations.resolve(option.id, option.text, {}, "fr").locale == "fr", option.id)
    if option.reasonId then
      assert(
        translations.resolve(option.reasonId, option.reason, {}, "fr").locale == "fr",
        option.reasonId
      )
    end
  end
end
localized.dispose()
local dark = new()
choose(dark, "lighthouse")
choose(dark, "together")
choose(dark, "dark")
assert(boundary(dark) == "completed")
dark.dispose()
print("SHOWCASE_STORY_OK three endings, relationship variants, knowledge gates, checkpoint memory")
