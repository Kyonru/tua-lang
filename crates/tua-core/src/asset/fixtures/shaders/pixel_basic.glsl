// Pixel-only shader using LOVE's dialect: extern, number, Image, Texel.
// Expected: compiles on desktop and GLES.
extern number time;
extern vec4 tint;

vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) {
    vec4 texel = Texel(tex, uv);
    return texel * color * tint * (0.5 + 0.5 * sin(time));
}
