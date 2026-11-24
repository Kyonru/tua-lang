do
  local Dialogue = require("tuadialogue")
  local source = {
    schemaVersion = 9,
    documentId = "history",
    entry = "hello",
    nodes = {
      hello = {
        kind = "line",
        text = "Hello",
        textId = "hello.text",
        speaker = "Captain",
        speakerKey = "captain",
        next = "reply",
      },
      reply = {
        kind = "choice",
        options = {
          { id = "yes", text = "Yes", target = "hello" },
        },
      },
    },
  }
  local fail = false
  local function resolve(_, text, _, locale)
    if fail then error("resolver failed") end
    return locale .. ":" .. text
  end
  local function create(history)
    return Dialogue.new(source, {
      locale = "en",
      resolve = resolve,
      checkpointVersion = "v1",
      history = history,
    })
  end
  local disabled = create()
  disabled:start()
  assert(#disabled:history().entries == 0 and disabled:history().capacity == 0)
  assert(not disabled:redactHistory(1))
  disabled:clearHistory()
  for _, options in ipairs({
    {},
    { capacity = 0 },
    { capacity = 1025 },
    { capacity = 1.5 },
    { capacity = 1, maxBytes = 0 },
    { capacity = 1, maxBytes = false },
    { capacity = 1, maxBytes = math.huge },
    { capacity = 1, extra = true },
    setmetatable({ capacity = 1 }, {}),
  }) do
    assert(not pcall(create, options), "invalid history options")
  end
  local player = create({ capacity = 3 })
  player:start()
  local first = player:history().entries[1]
  assert(first.kind == "line" and first.text == "en:Hello")
  assert(first.textId == "hello.text" and first.speakerKey == "captain")
  assert(first.speaker == "en:Captain" and first.locale == "en")
  first.text = "caller mutation"
  assert(player:history().entries[1].text == "en:Hello")
  player:setLocale("fr")
  player:update(1, true)
  assert(#player:history().entries == 1, "refresh/update do not duplicate history")
  assert(player:history().entries[1].locale == "en", "preserve original translation")
  player:advance()
  assert(not player:choose("missing"))
  player:suspend()
  assert(not player:choose("yes"))
  assert(#player:history().entries == 1)
  player:resume()
  fail = true
  assert(not pcall(player.choose, player, "yes"))
  assert(#player:history().entries == 1, "failed transition does not record selection")
  assert(player:snapshot().state == "awaiting_choice")
  fail = false
  assert(player:choose("yes"))
  local log = player:history()
  assert(#log.entries == 3)
  assert(log.entries[2].kind == "choice" and log.entries[2].choiceId == "yes")
  assert(log.entries[2].text == "fr:Yes" and log.entries[2].locale == "fr")
  assert(log.entries[3].kind == "line" and log.entries[3].text == "fr:Hello")
  player:advance()
  player:choose("yes")
  log = player:history()
  assert(#log.entries == 3 and log.dropped == 2 and log.entries[1].id == 3)
  assert(player:redactHistory(log.entries[2].id))
  assert(not player:redactHistory(log.entries[2].id))
  assert(#player:history().entries == 2 and player:history().dropped == 2)
  local afterRedaction = player:history()
  local bytes = 0
  for _, entry in ipairs(afterRedaction.entries) do
    for _, value in pairs(entry) do
      if type(value) == "string" then bytes = bytes + #value end
    end
    assert(entry.events == nil and entry.presentation == nil and entry.voice == nil)
  end
  assert(bytes == afterRedaction.bytes)
  local lastId = afterRedaction.entries[2].id
  local saved = player:checkpoint()
  assert(saved.history == nil and saved.entries == nil)
  local restored = create({ capacity = 3 })
  restored:restore(saved)
  assert(#restored:history().entries == 0, "restore does not replay history")
  player:clearHistory()
  assert(#player:history().entries == 0 and player:history().bytes == 0)
  assert(player:history().dropped == 0)
  player:start()
  assert(player:history().entries[1].id > lastId, "redaction IDs are never recycled")
  player:cancel()
  assert(#player:history().entries == 1, "cancel keeps transcript available")
  player:start()
  assert(#player:history().entries == 2, "restart retains history until explicit clear")
  player:dispose()
  assert(#player:history().entries == 0 and player:history().bytes == 0)
  player:clearHistory()
  assert(not player:redactHistory(1))
  for _, id in ipairs({ 0, -1, 1.5, math.huge, "1" }) do
    assert(not pcall(player.redactHistory, player, id))
  end
  local one = create({ capacity = 3 })
  one:start()
  local size = one:history().bytes
  local bounded = create({ capacity = 10, maxBytes = size })
  bounded:start()
  bounded:start()
  assert(#bounded:history().entries == 1 and bounded:history().dropped == 1)
  local oversized = create({ capacity = 2, maxBytes = 1 })
  oversized:start()
  assert(oversized:history().dropped == 1 and #oversized:history().entries == 0)
  assert(oversized:snapshot().state == "awaiting_advance", "limits do not block playback")
  for _ = 1, 1000 do bounded:start() end
  assert(bounded:history().bytes <= size and #bounded:history().entries == 1)
  disabled:dispose()
  restored:dispose()
  one:dispose()
  bounded:dispose()
  oversized:dispose()
  local action = require("forward").new(require("scene"), {
    history = { capacity = 4 },
    handlers = { purchase = function() return true end },
  })
  action:start()
  assert(#action:history().entries == 0)
  action:runAction()
  assert(#action:history().entries == 1)
  action:dispose()
  local guarded
  guarded = Dialogue.new(source, {
    history = { capacity = 4 },
    locale = "en",
    resolve = function(_, text)
      assert(not pcall(guarded.clearHistory, guarded))
      assert(not pcall(guarded.redactHistory, guarded, 1))
      return text
    end,
  })
  guarded:start()
  guarded:setLocale("fr")
  assert(#guarded:history().entries == 1)
  guarded:dispose()
  print("bounded dialogue history passed")
end
