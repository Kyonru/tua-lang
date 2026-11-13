extern vec4 tintColor;

vec4 applyTint(vec4 color) {
    return color * tintColor;
}
