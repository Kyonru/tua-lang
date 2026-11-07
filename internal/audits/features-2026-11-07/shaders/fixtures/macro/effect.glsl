#define NAME alpha
extern number NAME;
vec4 effect(vec4 c, Image t, vec2 uv, vec2 px) { return c * NAME; }
