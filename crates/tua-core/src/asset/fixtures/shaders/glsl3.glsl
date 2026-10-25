#pragma language glsl3
// GLSL 3 shader: LOVE selects it from the pragma on the first line.
// Expected: compiles on desktop and GLES (GLSL 3.30 / ES 3.00).
extern int frame;

vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) {
    ivec2 size = textureSize(tex, 0);
    float pulse = float((frame + size.x) % 60) / 60.0;
    return Texel(tex, uv) * color * pulse;
}
