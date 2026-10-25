// Defines neither `effect` nor `position`.
// Expected: love.graphics.newShader raises
// "missing 'position' or 'effect' function?".
extern number time;

float wave(float x) {
    return sin(x + time);
}
