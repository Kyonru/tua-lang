local Controller = require("controller")
local sources = 0
local disposed = 0
local function backend()
  sources = sources + 1
  return {
    stop = function() end,
    pause = function() end,
    resume = function() end,
    dispose = function() disposed = disposed + 1 end,
    voice = function() return false end,
    speaking = function() return false end,
    chatter = function() return true end,
    volume = function() end,
    wordStart = function() return true end,
  }
end
local host = Controller.new(backend(), backend())
host.update(0)
assert(host.metrics().shown == 1 and host.metrics().revealed == 0)
for index = 1, 4 do assert(host.enqueue("radio")) end
assert(not host.enqueue("radio"))
for index = 1, 2 do assert(host.enqueue("conversation")) end
assert(not host.enqueue("conversation"))
assert(host.metrics().messages == 4 and host.metrics().conversations == 2)
host.update(10)
assert(host.metrics().messages == 4, "modal conversation owns priority")
host.reset()
assert(host.metrics().messages == 0 and host.metrics().conversations == 0)
host.settings.mode = "bubble"
assert(host.enqueue("radio"))
host.update(0)
assert(host.radio:snapshot().state == "awaiting_advance")
host.settings.paused = true
local before = host.radioView().visible
host.update(20)
assert(host.radioView().visible == before)
host.settings.paused = false
host.settings.reducedMotion = true
host.update(0.2)
assert(host.view().complete)
host.update(0.2)
assert(host.metrics().revealed >= 1)
host.locale()
assert(host.player():snapshot().node.text:find("Ada", 1, true))
assert(host.player():snapshot().node.text:find("Bienvenue", 1, true))
host.advance()
host.update(0)
host.advance()
assert(host.player():snapshot().state == "awaiting_choice")
host.player():updateContext({ name = "Ada", permit = false })
host.advance()
assert(host.player():snapshot().state == "awaiting_choice", "disabled choices must not execute")
host.player():updateContext({ name = "Ada", permit = true })
host.advance()
assert(host.metrics().rewards == 1)
host.update(0.6)
assert(host.player():snapshot().nodeId == "done")
assert(host.save())
host.advance()
host.restore()
assert(host.metrics().rewards == 1, "restore must not replay an action")
assert(host.player():snapshot().restored)
assert(host.player():history().capacity == 8)
for index = 1, 20 do
  host.reset()
  host.update(0)
end
for index = 1, 20 do
  host.player():start()
  host.update(0)
  host.advance()
end
assert(#host.player():history().entries == 8)
assert(host.player():history().dropped > 0, "transcript evicts old entries at its bound")
host.dispose()
host.dispose()
assert(sources == disposed, "each owned backend is disposed once")
print("SHOWCASE_HEADLESS_OK rewards=1 queues=bounded restore=quiet")
