extern number flash;
vec3 hitColor(vec3 original) {
    return mix(original, vec3(1.0, 0.85, 0.60), clamp(flash, 0.0, 1.0));
}
