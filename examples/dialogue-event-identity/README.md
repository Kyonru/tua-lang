# Dialogue event identity

Two `portrait.pulse` notifications have different authored IDs. The headless host
checks both clocks, source locations and delivery identity after restarting.
It uses no graphics, audio, handlers or save storage.

From this directory, run:

```sh
../../target/debug/tua check
../../target/debug/tua build
cd build
lua main.lua
```

The compiler and emitted runtime must support schema 4.
IDs do not provide persistence or deduplication across runs.
