// Expected: fails on desktop and GLES at line 4: a float cannot initialize a
// vec3.
vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) {
    vec3 shade = 1.0;
    return Texel(tex, uv) * color * vec4(shade, 1.0);
}
