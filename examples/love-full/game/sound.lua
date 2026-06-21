local sound = {}
local IS_SMOKE = os.getenv("TLUA_LOVE_FULL_SMOKE") == "1"

local function noop_source()
  return {
    play = function()
    end,
    setLooping = function()
    end,
    setVolume = function()
    end,
  }
end

function sound.load()
  if IS_SMOKE then
    sound.music = noop_source()
    sound.sfx = {
      bump = noop_source(),
      hurt = noop_source(),
      disappear = noop_source(),
      move = noop_source(),
      stop = noop_source(),
      catch = noop_source(),
    }
    return
  end

  sound.music = love.audio.newSource("assets/sounds/adventures_in_adventureland.mp3", "stream")
  sound.music:setLooping(true)
  sound.music:play()

  sound.sfx = {
    bump = love.audio.newSource("assets/sounds/sfx_bump.ogg", "static"),
    hurt = love.audio.newSource("assets/sounds/sfx_hit.wav", "static"),
    disappear = love.audio.newSource("assets/sounds/sfx_disappear.ogg", "static"),
    move = love.audio.newSource("assets/sounds/sfx_move.wav", "stream"),
    stop = love.audio.newSource("assets/sounds/sfx_stop.ogg", "static"),
    catch = love.audio.newSource("assets/sounds/sfx_catch.ogg", "static"),
  }
end

function sound.update(dt)
  sound.music:setVolume(0.5)
end

function sound.play_bump()
  sound.sfx.bump:play()
end

function sound.play_hurt()
  sound.sfx.hurt:play()
end

function sound.play_disappear()
  sound.sfx.disappear:play()
end

function sound.play_move()
  sound.sfx.move:setVolume(0.5)
  sound.sfx.move:play()
end

function sound.play_stop()
  sound.sfx.stop:play()
end

function sound.play_catch()
  sound.sfx.catch:play()
end

return sound
