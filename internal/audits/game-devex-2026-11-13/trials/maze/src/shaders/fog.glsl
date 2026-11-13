#include "shaders/tint.glsl"

vec4 effect(vec4 color, Image texture, vec2 uv, vec2 pixel) {
    return applyTint(Texel(texture, uv) * color);
}
