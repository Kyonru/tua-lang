# tua

tua is a typed Lua superset that compiles to plain Lua 5.1-compatible source
for LuaJIT and LÖVE projects. Types are erased at compile time, so emitted Lua
can run without a tua runtime.

The repository contains the compiler core, CLI, VS Code extension, bundled
examples, and a broad language-server feature set.

- [Specification](docs/specification.md) defines the current language and
  toolchain contract.
- [Capability Matrix](docs/capabilities.md) records current language and editor
  support.
- [Roadmap](docs/roadmap.md) tracks product direction as Now, Next, and Later.
- [Object Pooling](docs/object-pooling.md) documents the built-in typed pool
  and when reuse is worth its lifecycle cost.
- [Releases](docs/releases.md) documents artifacts, checksums, compatibility,
  and migration policy.
- [CHANGELOG.md](CHANGELOG.md) records release-level changes.
- [docs/](docs/index.md) contains the Zensical documentation site source.

The original v1 specification and commit-by-commit implementation roadmap are
preserved under [`docs/archive/`](docs/archive/) for historical reference.

## Development

```sh
cargo test --workspace
cargo run -p tua-cli -- --help
```

Useful local workflows:

```sh
make build-features
make editor-regression-smoke
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
