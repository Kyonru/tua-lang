local host
local function tap(context, key)
  context.key(key, true)
  host.update(0)
  context.key(key, false)
  host.update(0)
end
local function setup(context)
  host = require("__tua_replay_game")
  assert(host.combat.options.enemyCount == 8)
  tap(context, "f6")
  assert(host.combat.options.enemyCount == 32)
  tap(context, "f6")
  assert(host.combat.options.enemyCount == 128)
  tap(context, "return")
  assert(host.combat.enemies:count() == 128)
  tap(context, "f6")
  assert(host.combat.options.enemyCount == 128, "workload changes only at title")
  context.key("space", true)
  context.pointer(900, 320)
end
local function beforeFrame(context)
  if context.frame == 30 then
    context.key("space", false)
    tap(context, "backspace")
    tap(context, "f6")
    assert(host.combat.options.enemyCount == 8)
    tap(context, "return")
    assert(host.combat.enemies:count() == 8 and #host.combat.bullets == 0)
  end
end
local function verify()
  assert(host.combat.state.shots == 0)
  print("ARENA_WORKLOAD_NATIVE_OK bounded wave controls, 128-enemy rendering, reset to default")
end
return { setup = setup, beforeFrame = beforeFrame, verify = verify }
