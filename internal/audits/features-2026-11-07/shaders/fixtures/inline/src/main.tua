local fx = love.graphics.newShader([[
extern number alpha;
extern vec4 tint;
extern number unused;
vec4 effect(vec4 c, Image t, vec2 uv, vec2 px) { return c * tint * alpha; }
]])
fx:send("alpha", 1)
fx:sendColor("tint", {1,1,1,1})
