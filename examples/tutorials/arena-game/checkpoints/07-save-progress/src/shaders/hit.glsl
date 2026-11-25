#include "shaders/lib/flash.glsl"
vec4 effect(vec4 color, Image texture, vec2 uv, vec2 pixel) {
    vec4 texel = Texel(texture, uv) * color;
    return vec4(hitColor(texel.rgb), texel.a);
}
