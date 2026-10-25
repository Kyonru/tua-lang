// A real LÖVE pixel shader so `tua check` can compile the example asset.
extern number time;
vec4 effect(vec4 color, Image texture, vec2 texture_coords, vec2 screen_coords)
{
    vec2 uv = texture_coords;
    uv.x += sin(uv.y * 20.0 + time) * 0.01;
    return Texel(texture, uv) * color;
}
