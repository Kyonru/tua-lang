# tua

tua is a typed Lua superset that compiles to plain Lua 5.1-compatible source
for LuaJIT and LÖVE projects. Types are erased at compile time, so emitted Lua
can run without a tua runtime.

The repository now contains the compiler core, CLI, VS Code extension, bundled
examples, and a broad language-server feature set.

- [SPEC.md](SPEC.md) defines the v1 language and toolchain contract.
- [ROADMAP.md](ROADMAP.md) breaks the spec into committable implementation
  slices.
- [docs/](docs/index.md) contains the Zensical documentation site source.

## Development

```sh
cargo test --workspace
cargo run -p tua-cli -- --help
```

Useful local workflows:

```sh
make build-features
make lsp-features-smoke
make extension-package
```

GitHub Linguist support materials live in `support/github-linguist/`.

Serve the documentation site locally:

```sh
make docs
```

## Examples

- `examples/features` is a buildable tour of the v1 language and LSP features.
- `examples/love-minimal` is a small LÖVE smoke-test project.
- `examples/love-full` is a larger LÖVE conversion example with Lua interop.
