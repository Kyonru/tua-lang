// A water surface with displaced vertices, ripples, and a depth tint, in the
// style of a 2.5D game. Uniform names repeat with numeric suffixes, as games
// send them from loops: ripple1..ripple3.
// Expected: compiles on desktop and GLES.
extern number time;
extern number horizonY;
extern vec4 ripple1;
extern vec4 ripple2;
extern vec4 ripple3;
varying float depth;

float ripple(vec4 source, vec2 point) {
    float distanceToSource = distance(point, source.xy);
    return sin(distanceToSource * 12.0 - time * 4.0) * source.z / (1.0 + distanceToSource * 8.0);
}

#ifdef VERTEX
vec4 position(mat4 transform, vec4 vertex) {
    depth = clamp((vertex.y - horizonY) / 400.0, 0.0, 1.0);
    vertex.y += sin(vertex.x * 0.05 + time) * 2.0 * depth;
    return transform * vertex;
}
#endif

#ifdef PIXEL
vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) {
    vec2 point = vec2(uv.x, depth);
    float wave = ripple(ripple1, point) + ripple(ripple2, point) + ripple(ripple3, point);
    vec4 water = mix(vec4(0.16, 0.55, 0.76, 1.0), vec4(0.05, 0.2, 0.35, 1.0), depth);
    return Texel(tex, uv + vec2(wave * 0.01)) * water * color;
}
#endif
