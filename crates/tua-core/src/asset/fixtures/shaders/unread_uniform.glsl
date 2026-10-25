// `strength` is declared but never read, so the GLSL compiler removes it and
// LOVE raises "Shader uniform 'strength' does not exist" when it is sent.
// Expected: compiles; sending `strength` fails at runtime.
extern number time;
extern number strength;

vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) {
    return Texel(tex, uv) * color * time;
}
