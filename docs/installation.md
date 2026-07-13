---
icon: lucide/package
---

# Installation

Tua is currently developed from source as a Rust workspace.

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

Install dependencies and check the activation script:

```bash
make extension-deps
make extension-check
```

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
