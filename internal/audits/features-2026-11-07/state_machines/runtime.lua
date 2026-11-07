local runtime = dofile(arg[1] .. "/vendor/runtime/tuastatemachine.lua")
-- Cancellation should prevent onleave side effects.
local left = 0
local m = runtime.create({initial="idle",events={{name="go",from="idle",to="run"}},callbacks={
 onbeforego=function() return false end,
 onleaveidle=function() left=left+1 end,
}})
print("cancel",m:go(),m.current,"leave callback count",left)
-- Completion callbacks commonly advance another transition.
local n = runtime.create({initial="idle",events={{name="go",from="idle",to="run"},{name="finish",from="run",to="done"}},callbacks={
 onaftergo=function(self) self:finish() end,
 onleaverun=function(self) return self.ASYNC end,
}})
print("chain",n:go(),n.current,n.asyncState,tostring(n.currentTransitioningEvent))
print("resume finish",tostring((n:transition("finish"))),n.current,n.asyncState)
-- DOT labels must preserve spaces and wildcard syntax.
local d = runtime.create({initial="game menu",events={{name="start game",from="*",to="game over"}}})
d:todot(arg[2])
