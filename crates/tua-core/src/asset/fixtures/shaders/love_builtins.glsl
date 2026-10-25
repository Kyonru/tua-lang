// Uses uniforms and helpers that LOVE's header declares.
// Expected: compiles on desktop and GLES.
vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) {
    vec2 centered = love_PixelCoord / love_ScreenSize.xy - 0.5;
    float vignette = 1.0 - dot(centered, centered);
    return gammaCorrectColor(Texel(tex, uv)) * color * vignette;
}
