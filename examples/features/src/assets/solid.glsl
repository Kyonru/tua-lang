// Small second shader for the incremental loading-screen example.
vec4 effect(vec4 color, Image texture, vec2 texture_coords, vec2 screen_coords)
{
    return Texel(texture, texture_coords) * color;
}
