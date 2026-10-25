// Expected: compiles on desktop, fails on GLES at line 4: GLSL ES does not
// convert the int literal 1 to float.
vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) {
    float brightness = 1;
    return Texel(tex, uv) * color * brightness;
}
