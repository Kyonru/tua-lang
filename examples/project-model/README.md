# Project Model Example

This example shows the project-level rules that Tua now models in one place.

- `tua.toml` defines the source root, output root, extension, and target.
- `src/**/*.tua` files are parsed, checked, and emitted as Lua.
- Plain `.lua` files under `src/` are Lua interop files and are copied through
  by `tua build`.
- `require("entities.player")` resolves through the configured source root
  instead of probing relative to the current file's parent directory.

Try it from this directory:

```bash
cargo run --manifest-path ../../Cargo.toml -p tua-cli -- build
```
