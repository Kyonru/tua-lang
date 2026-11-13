#include "assets/lib/pulse.glsl"
vec4 effect(vec4 color, Image texture, vec2 uv, vec2 screen)
{
  number glow = 0.88 + 0.12 * sin(time * 3.0);
  return Texel(texture, uv) * color * glow;
}
