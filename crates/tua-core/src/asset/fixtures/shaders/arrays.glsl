// Array and matrix uniforms.
// Expected: compiles on desktop and GLES.
extern vec4 lights[4];
extern number weights[4];
extern mat4 colorMatrix;

vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) {
    vec4 total = vec4(0.0);
    for (int i = 0; i < 4; i++) {
        float falloff = 1.0 / (1.0 + distance(screen, lights[i].xy));
        total += lights[i].w * weights[i] * falloff * vec4(1.0);
    }
    return colorMatrix * (Texel(tex, uv) * color + total);
}
