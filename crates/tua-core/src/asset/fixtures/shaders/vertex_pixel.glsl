// One file with both stages, split by LOVE's VERTEX and PIXEL defines.
// `sway` is declared and read in both sections.
// Expected: compiles on desktop and GLES.
extern number sway;

#ifdef VERTEX
vec4 position(mat4 transform, vec4 vertex) {
    vertex.x += sin(vertex.y * 0.1) * sway;
    return transform * vertex;
}
#endif

#ifdef PIXEL
vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) {
    return Texel(tex, uv) * color * (1.0 - sway * 0.1);
}
#endif
