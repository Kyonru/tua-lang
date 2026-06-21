# love-minimal

Minimal tua project for validating emitted Lua under LOVE 11.x.

```sh
cargo run -p tua-cli -- build
love build
```

The example is configured to run headless and quit from `love.load`, which keeps
runtime validation suitable for local smoke tests and CI environments where LOVE
is installed.
