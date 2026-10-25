// Custom pixel entry point that writes several canvases at once.
// Expected: compiles on desktop and GLES.
void effect() {
    love_Canvases[0] = VaryingColor;
    love_Canvases[1] = vec4(VaryingTexCoord.xy, 0.0, 1.0);
}
