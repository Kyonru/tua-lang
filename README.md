# tua

tua is a typed Lua superset that compiles to plain Lua 5.1-compatible source
for LuaJIT and LÖVE projects. Types are erased at compile time, so emitted Lua
can run without a tua runtime.

This repository is currently in the v1 scaffold stage.

- [SPEC.md](SPEC.md) defines the v1 language and toolchain contract.
- [ROADMAP.md](ROADMAP.md) breaks the spec into committable implementation
  slices.

## Development

```sh
cargo test --workspace
cargo run -p tua-cli -- --help
```
