#include "src/assets/lib/pulse.glsl"
vec4 effect(vec4 color, Image texture, vec2 uv, vec2 screen)
{
  return Texel(texture, uv) * color * glow;
}
