-- Exercise the emitted LÖVE callbacks while recording presentation requests.
local function noop() end
local function backend()
  return {
    stop = noop,
    pause = noop,
    resume = noop,
    dispose = noop,
    voice = function() return false end,
    speaking = function() return false end,
    chatter = function() return false end,
    volume = noop,
    wordStart = function() return true end,
  }
end
local drawn = {}
package.loaded.backend = { new = backend }
package.loaded.presenter = {
  new = function()
    return {
      update = noop,
      dispose = noop,
      draw = function(node)
        drawn[#drawn + 1] = node.kind
        return 0
      end,
    }
  end,
}
love = {
  event = { quit = noop },
  graphics = {
    newFont = function() return { release = noop } end,
    getDimensions = function() return 960, 640 end,
    clear = noop,
    setColor = noop,
    rectangle = noop,
    line = noop,
    circle = noop,
    setFont = noop,
    print = noop,
    printf = noop,
  },
}
local host = require("main")
host.settings.mode = "bubble"
host.radio:start()
love.draw()
assert(#drawn == 2, "active conversation and radio are presented")
host.player():start("done")
host.player():advance()
host.radio:advance()
assert(host.player():snapshot().state == "completed")
assert(host.radio:snapshot().state == "completed")
drawn = {}
love.draw()
assert(#drawn == 0, "terminal nodes must not create empty panels")
love.keypressed("f5")
assert(host.settings.notice:find("No checkpoint", 1, true))
love.keypressed("r")
host.settings.reducedMotion = true
host.update(0)
love.keypressed("down")
love.keypressed("up")
assert(host.settings.selected == 1, "arrows outside choices preserve focus")
host.settings.selected = 2
love.keypressed("return")
love.keypressed("return")
assert(host.player():snapshot().state == "awaiting_choice")
assert(host.settings.selected == 1, "choice entry initializes the first option")
love.keypressed("down")
assert(host.settings.selected == 2)
love.keypressed("up")
assert(host.settings.selected == 1)
love.keypressed("return")
assert(host.metrics().rewards == 1, "first Enter on a choice executes the focused option")
love.quit()
print("SHOWCASE_UI_OK completed channels hidden; choice focus valid")
