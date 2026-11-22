# Pause and input routing

Run `tua build`, then `love build`. Escape pauses/resumes; Space fires during
play and confirms in the pause menu. Focus loss pauses without automatically
resuming. U grants an upgrade only when the model is explicitly reset into
practice mode; the interactive example starts in normal mode.

`state.tua` uses existing `input` instances and independent `Tween.group()`
objects. Every input instance is polled every frame, including while paused,
so a held key does not become a fresh press on resume. Routing handles global
commands first and consumes a pause-transition frame. UI time and its tween
group keep advancing while gameplay time and movement stop. Resume adds only
the next frame's dt, with no catch-up. Repeated transitions stop the previous
menu tween; reset replaces both tween groups and clears run state while input
edges continue to reflect held keys.

`scenario.tua` is only loaded explicitly by the native runner. From the Tua
repository root:

```sh
python3 scripts/love_replay.py --project examples/pause-input \
  --tua target/release/tua --love /path/to/love \
  --scenario-module scenario --frames 60 --fps 30 \
  --output /tmp/tua-pause-example --video
```

Assertions cover paused gameplay/movement, advancing UI, held keys across
resume, no catch-up, normal/practice upgrade gating, reset, and simultaneous
pause/fire input consumption. Use at least 24 frames at 30 FPS for this scenario.
Video capture needs ffmpeg; omit `--video` for assertions alone. See
[testing documentation](../../docs/testing.md#native-love-scenarios-and-video)
for isolation, provenance and runtime limitations. This is a functional test,
not a timing benchmark or a mandatory game framework.
