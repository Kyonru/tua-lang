# love-full

Full LOVE example adapted from:

https://github.com/Kyonru/creating-games-with-love-demo/tree/main/game

This example intentionally mixes tua and plain Lua:

- `src/*.tua` contains converted typed modules.
- `game/*.lua` contains plain Lua modules kept as interop dependencies.
- `game/assets/` contains the runtime assets used by both module kinds.

The example enables `diagnostics.undefined_variables` and declares the
intentional runtime globals `screen_width` and `screen_height` in `tua.toml` so
stale local names are caught before the generated game runs.

Build the converted modules into the runnable LOVE root:

```sh
cargo run -p tua-cli -- build
love game
```

The generated files are emitted into `game/` and ignored by git. For automated
runtime validation, set `TUA_LOVE_FULL_SMOKE=1`; the game starts, uses stubbed
audio, loads core modules, runs one update tick, and exits.
