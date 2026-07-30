# tua

Typed Lua source-to-source tooling for LuaJIT and LOVE projects.

[Documentation](https://kyonru.github.io/tua-lang/) ·
[GitHub Releases](https://github.com/Kyonru/tua-lang/releases) ·
[VS Code Extension](editors/vscode/README.md)

Tua starts with normal Lua syntax, adds erased type syntax and a small amount
of LuaJIT-safe sugar, then emits readable Lua 5.1-compatible code. It is built
for projects that want stronger editor feedback, static checks, and build
tooling without leaving the normal Lua and LOVE workflow.

This repository contains the compiler core, CLI, language server, VS Code
extension, documentation site, and bundled example projects.

## What Tua Provides

- Erased local, function, alias, table-shape, optional-field, and union types.
- Readable Lua 5.1-compatible output for LuaJIT and LOVE 11.x.
- A CLI for `init`, `check`, `fmt`, `lint`, `build`, `traceback`, `watch`, and
  `lsp`.
- A compiler-backed language server and VS Code extension with diagnostics,
  completion, hover, signature help, rename, references, semantic tokens,
  inlay hints, document/workspace symbols, and more.
- LOVE-aware helpers, API catalogs, callback guidance, and asset-path
  validation for both Tua helpers and native LOVE constructors.
- Opt-in built-in helpers such as `Vec2`, `spriteSheet({...})`, Baton-backed
  typed input, state stores, state machines, ECS, and object pools.
- Mixed-source builds where `.tua` files compile to `.lua`, while plain `.lua`
  files and assets are copied into the build output for runnable LOVE folders.

## What You Can Do With It

- Write typed gameplay code that still ships as ordinary Lua source.
- Keep existing Lua modules and gradually adopt `.tua` where stronger tooling
  helps.
- Build LOVE projects with editor-aware assets, generated-Lua/source-map
  navigation, source-mapped Lua debugging, and project graphs for modules,
  signals, ECS, and object pools.
- Use one versioned toolchain across the CLI, language server, release VSIXs,
  and documentation.

## VS Code Extension

The extension lives in `editors/vscode` and is distributed through GitHub
release VSIXs plus registry publishing for Visual Studio Marketplace and Open
VSX. Published VSIX packages bundle the matching `tua` server binary for each
platform, so extension installs do not download a server at activation time.

It provides compiler-backed Tua editing, LOVE-aware completion and diagnostics,
generated Lua inspection, project commands for running or debugging LOVE, and
explorers for assets, modules, signals, ECS, and object pools.

Experimental local Ollama ghost-text completion is opt-in and compiler
validated before display. Trusted local tools can also explicitly start the
read-only `tua api --stdio` JSON-RPC session or `tua mcp --stdio`; neither
service opens a network socket or starts automatically.

## Learn More

- [Documentation site](https://kyonru.github.io/tua-lang/)
- [Installation](https://kyonru.github.io/tua-lang/installation/)
- [Quickstart](https://kyonru.github.io/tua-lang/quickstart/)
- [Language overview](https://kyonru.github.io/tua-lang/language/)
- [Typed state machines](https://kyonru.github.io/tua-lang/state-machines/)
- [Typed state stores](https://kyonru.github.io/tua-lang/stores/)
- [Typed input](https://kyonru.github.io/tua-lang/input/)
- [CLI reference](https://kyonru.github.io/tua-lang/cli/)
- [Language server](https://kyonru.github.io/tua-lang/lsp/)
- [External API and MCP](https://kyonru.github.io/tua-lang/external-api-bridge/)
- [Debugging](https://kyonru.github.io/tua-lang/debugging/)
- [Examples](https://kyonru.github.io/tua-lang/examples/)
- [Releases and compatibility](https://kyonru.github.io/tua-lang/releases/)
- [CHANGELOG.md](CHANGELOG.md)

## Development

Pending contributor priorities are tracked in [`ROADMAP.md`](ROADMAP.md).

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
- `examples/linter` intentionally breaks every currently emitted standard lint
  rule and is expected to fail `tua lint`.
- `examples/love-minimal` is a small LÖVE smoke-test project.
- `examples/love-full` is a larger LÖVE conversion example with Lua interop.
- `examples/love-tua` is a mixed Tua/Lua bullet-hell showcase combining ECS,
  object pools, stores, state machines, typed input, and vendored Feel feedback.
