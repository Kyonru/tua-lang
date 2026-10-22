# Starfall

Collect stars, dodge meteors. A small complete LOVE game written in Tua:

- `main.tua` wires the scenes with `stateMachine`, follows the ship with the
  built-in `Camera`, and saves the best score.
- `player.tua` reads keyboard and gamepad input with `input` and steers with
  frame-rate independent smoothing.
- `world.tua` keeps stars and pooled meteors (`objectPool`) in a world larger
  than the screen.
- `hud.tua` animates the title, score, and banners with the built-in `tween`.
- `save.tua` stores the high score in LOVE's save directory.

From this directory:

```bash
tua dev        # play with hot reload: edit world.tua or hud.tua while it runs
tua package    # write dist/starfall.love
love build --smoke   # after a build: play a scripted round and quit
```

Controls: arrows or WASD (or a gamepad stick) to move, Enter, Space, or A to
start.
