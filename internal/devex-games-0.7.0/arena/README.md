# Meteor Dodge (Tua 0.7.0 trial)

A small LOVE game: move the blue player left and right to dodge falling red meteors. Survival time is the score. After a collision, press **R** to restart.

## Run

From this directory:

```sh
../../../target/debug/tua check
../../../target/debug/tua build
/Applications/love.app/Contents/MacOS/love build
```

Use **A/D** or the **left/right arrows** to move. The game uses no external assets.

For a deterministic headless check of the generated Lua:

```sh
cd build
lua ../smoke.lua
```

`probe_lsp.py` starts the real Tua language server and checks hover, completion, and definition for `score:get()` from the imported generic class.

The game exercises a cross-file `Actor` protocol, two unrelated conforming classes, a mixed `Actor[]`, a bound generic update function, and an imported generic `Box<number>` instance type. Type and protocol declarations erase from generated Lua.
