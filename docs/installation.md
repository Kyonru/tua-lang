---
icon: lucide/package
---

# Installation

Tua can be installed from a platform release or built from the Rust workspace.

## Requirements

- Rust and Cargo.
- Node.js for the VS Code extension checks and LSP smoke scripts.
- Python 3 only if you want to build or serve these Zensical docs.
- LOVE 11.x is optional and only needed for running the LOVE examples.

## Build The CLI

From the repository root:

```bash
cargo build -p tua-cli
```

The debug binary is written to:

```text
target/debug/tua
```

For a release build:

```bash
cargo build --release -p tua-cli
```

## Install A Release

Each tagged release provides standalone `tua` server archives and VSIX packages
for Linux x64, Windows x64, macOS Intel, and macOS Apple silicon. Choose the
artifact matching your operating system and architecture.

Release downloads live on
[GitHub Releases](https://github.com/Kyonru/tua-lang/releases).

Release VSIX packages contain the matching server binary. They do not download
a server during activation. Verify downloads against the release's
`SHA256SUMS` before installation; see
[Releases And Compatibility](releases.md) for commands and the support policy.

## Run From Source

During development, the shortest path is:

```bash
cargo run -p tua-cli -- --help
```

For example:

```bash
cargo run -p tua-cli -- init
cargo run -p tua-cli -- check
cargo run -p tua-cli -- build
```

## VS Code Extension

The extension lives in `editors/vscode`.

Install it from one of these published channels:

- [Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=tua-lang.tua)
- [Open VSX](https://open-vsx.org/extension/tua-lang/tua)
- [GitHub release VSIX packages](https://github.com/Kyonru/tua-lang/releases)

Install dependencies and check the activation script:

```bash
make extension-deps
make extension-check
make extension-test
```

`extension-test` launches an isolated VS Code Extension Development Host and
verifies native quick suggestions against a controlled LSP server. Set
`VSCODE_EXECUTABLE_PATH` to use a specific VS Code installation; otherwise the
test uses the standard macOS installation when present or downloads the latest
stable test build.

Build a VSIX with the current platform's Tua server binary:

```bash
make extension-package
```

Install it with:

```bash
make extension-install
```

The extension uses `tua.server.path` when set. Otherwise it looks for `tua` on
`PATH`, or uses the bundled binary inside packaged VSIX builds.

## Documentation Site

Serve the docs locally:

```bash
make docs
```

Build the static site:

```bash
make docs-build
```

The docs script creates `.venv/docs` and installs `zensical` there when needed.
