#include "assets/pulse.glsl"
extern float time;

vec4 effect(vec4 color, Image texture, vec2 uv, vec2 pixel) {
  vec4 texel = Texel(texture, uv);
  return vec4(texel.rgb * pulse(time), texel.a) * color;
}
