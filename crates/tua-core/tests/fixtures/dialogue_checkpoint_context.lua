do
  local Dialogue = require("tuadialogue")
  local Save = require("tuasave")
  local function clone(value)
    if type(value) ~= "table" then return value end
    local result = {}
    for key, child in pairs(value) do result[key] = clone(child) end
    return result
  end
  local context = Dialogue.context({
    facts = Save.shape(Save.record({ coins = Save.number(0) })),
    locals = Save.shape(Save.record({
      visits = Save.number(0),
      flag = Save.boolean(false),
      names = Save.array(Save.string(), {}),
      note = Save.optional(Save.string()),
    })),
  })
  local source = {
    schemaVersion = 9,
    documentId = "context.checkpoint",
    context = context,
    bindings = {
      { name = "visits", path = "locals.visits" },
      { name = "coins", path = "context.coins" },
    },
    entry = "gate",
    nodes = {
      gate = {
        kind = "branch",
        condition = {
          { op = ">=" },
          { op = "path", path = "locals.visits" },
          { op = "value", value = -2 },
        },
        yes = "line",
        no = "end",
      },
      line = { kind = "line", text = "Visit %{visits}, coins %{coins}", next = "choice" },
      choice = {
        kind = "choice",
        fallback = "end",
        options = {
          {
            id = "buy",
            text = "Buy",
            target = "wait",
            enabled = {
              { op = ">" },
              { op = "path", path = "context.coins" },
              { op = "value", value = 0 },
            },
          },
        },
      },
      wait = { kind = "wait", duration = 2, next = "line" },
      ["end"] = { kind = "end" },
    },
  }
  local function fresh(coins, document, settings)
    return context.new(document or source, { coins = coins or 10 }, settings or {
      checkpointVersion = "context-v1",
      history = { capacity = 4 },
    })
  end
  local player = fresh()
  player:start()
  player:setLocals({ visits = -1.5, flag = false, names = { "Ada" } })
  local saved = player:checkpoint()
  assert(saved.version == 2 and saved.locals.visits == -1.5 and saved.locals.flag == false)
  assert(saved.facts == nil and saved.context == nil and saved.history == nil)
  saved.locals.names[1] = "caller"
  assert(player:getLocals().names[1] == "Ada")
  saved = player:checkpoint()
  local target = fresh(99)
  target:restore(saved)
  saved.locals.names[1] = "changed again"
  assert(target:getLocals().names[1] == "Ada")
  assert(target:getContext().coins == 99)
  assert(target:snapshot().node.text == "Visit -1.5, coins 99")
  assert(target:snapshot().restored and #target:history().entries == 0)
  target:advance()
  local choice = target:checkpoint()
  local blocked = fresh(0)
  assert(not pcall(blocked.restore, blocked, choice))
  assert(blocked:snapshot().state == "idle" and blocked:getLocals().visits == 0)
  blocked:updateContext({ coins = 5 })
  blocked:restore(choice)
  assert(blocked:choose("buy"))
  blocked:update(0.25, false)
  blocked:suspend()
  local waiting = fresh()
  waiting:restore(blocked:checkpoint())
  assert(waiting:snapshot().state == "waiting" and waiting:snapshot().suspended)
  waiting:resume()
  waiting:update(1.75, false)
  assert(waiting:snapshot().node.text == "Visit -1.5, coins 10")
  local good = player:checkpoint()
  for _, mutate in ipairs({
    function(v) v.version = 1 end,
    function(v) v.locals = nil end,
    function(v) v.locals.visits = "wrong" end,
    function(v) v.locals.flag = nil end,
    function(v) v.locals.extra = true end,
    function(v) v.locals.visits = math.huge end,
    function(v) v.locals.names = { [2] = "sparse" } end,
    function(v) v.locals.note = string.rep("x", 262145) end,
    function(v) v.locals.note = string.char(255) end,
    function(v) v.locals.note = function() end end,
    function(v) v.locals.self = v.locals end,
    function(v) setmetatable(v.locals, {}) end,
  }) do
    local bad = clone(good)
    mutate(bad)
    local retry = fresh()
    assert(not pcall(retry.restore, retry, bad))
    assert(retry:snapshot().state == "idle" and retry:getLocals().visits == 0)
    retry:restore(good)
    retry:dispose()
  end
  for _, mutate in ipairs({
    function(d) d.nodes.gate.condition[3].value = -3 end,
    function(d) d.nodes.gate.yes = "end" end,
    function(d) d.nodes.choice.options[1].enabled[3].value = 1 end,
    function(d) d.bindings[1].path = "context.coins" end,
    function(d) d.bindings[1].fallback = -5 end,
  }) do
    local changed = clone(source)
    changed.context = context
    mutate(changed)
    local incompatible = fresh(10, changed)
    assert(not pcall(incompatible.restore, incompatible, good))
    assert(incompatible:snapshot().state == "idle")
    incompatible:dispose()
  end
  local changed = clone(source)
  changed.context = context
  changed.nodes.line.text = "New text: %{visits}"
  local edited = fresh(10, changed)
  edited:restore(good)
  assert(edited:snapshot().node.text == "New text: -1.5")
  local localized = clone(source)
  localized.context = context
  localized.nodes.line.textId = "hello"
  local original = fresh(10, localized)
  original:start()
  original:setLocals({ visits = 7, flag = true, names = {} })
  local input = original:checkpoint()
  input.locale = "en"
  local fail = true
  local resolving = fresh(25, localized, {
    checkpointVersion = "context-v1",
    locale = "en",
    resolve = function(_, _, args)
      if fail then error("resolver failure") end
      input.locals.visits = 1000
      return tostring(args.visits)
    end,
  })
  assert(not pcall(resolving.restore, resolving, input))
  assert(resolving:getLocals().visits == 0 and resolving:snapshot().state == "idle")
  fail = false
  resolving:restore(input)
  assert(resolving:getLocals().visits == 7 and resolving:snapshot().node.text == "7")
  local files = {}
  local backend = {
    mkdir = function() return true end,
    info = function(path) return files[path] and { size = #files[path] } or nil end,
    read = function(path) return files[path] end,
    write = function(path, bytes)
      files[path] = bytes
      return true
    end,
    remove = function(path)
      files[path] = nil
      return true
    end,
    -- Test-only checksum. Production Save backends use SHA-256.
    hash = function(bytes)
      local sum = 0
      for index = 1, #bytes do sum = (sum * 31 + bytes:byte(index)) % 2147483647 end
      return string.format("%08x", sum):rep(8)
    end,
  }
  local schema = Save.record({
    version = Save.number(2),
    documentId = Save.string(""),
    contentVersion = Save.string(""),
    graph = Save.string(""),
    nodeId = Save.string(""),
    state = Save.enum({ "awaiting_advance", "awaiting_choice", "waiting" }, "awaiting_advance"),
    elapsed = Save.number(0),
    revealedElapsed = Save.optional(Save.number()),
    revealedAt = Save.optional(Save.number()),
    suspended = Save.boolean(false),
    locale = Save.optional(Save.string()),
    fired = Save.array(Save.number(), {}),
    locals = Save.record({
      visits = Save.number(0),
      flag = Save.boolean(false),
      names = Save.array(Save.string(), {}),
      note = Save.optional(Save.string()),
    }),
  })
  local disk = Save.contract({ slot = "context", version = 1, schema = schema, backend = backend })
  assert(disk:save(good))
  local reader = Save.contract({
    slot = "context",
    version = 1,
    schema = schema,
    backend = backend,
  })
  local decoded = assert(reader:load())
  local roundTrip = fresh(50)
  roundTrip:restore(decoded)
  assert(roundTrip:getLocals().names[1] == "Ada" and roundTrip:getLocals().visits == -1.5)
  assert(roundTrip:getLocals().flag == false and roundTrip:getContext().coins == 50)
  roundTrip:dispose()
  local events = Dialogue.events({ ping = Save.shape(Save.record({ value = Save.number(0) })) })
  local registered = Dialogue.context({
    facts = Save.shape(Save.record({ coins = Save.number(0) })),
    locals = Save.shape(Save.record({ visits = Save.number(0) })),
    events = events,
  })
  local withEvents = clone(source)
  withEvents.context, withEvents.events = registered, events
  local unsafe = registered.new(withEvents, { coins = 10 }, { checkpointVersion = "context-v1" })
  unsafe:start()
  assert(not pcall(unsafe.checkpoint, unsafe), "typed event registries remain excluded")
  unsafe:dispose()
  for _, value in ipairs({ player, target, blocked, waiting, edited, original, resolving }) do
    value:dispose()
  end
  print("typed-context checkpoint lifecycle passed")
end
