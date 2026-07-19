---
icon: lucide/package
---

# Installation

!!! tip "Recommended setup"
    Use Tua through the VS Code extension. Published extension packages include
    the matching Tua language server, so normal editor installation does not
    require Rust, Cargo, Node.js, or a separate server setup.

## Recommended: VS Code Extension

Install Tua from one of these channels:

- [Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=tua-lang.tua)
- [Open VSX](https://open-vsx.org/extension/tua-lang/tua)
- [GitHub release VSIX packages](https://github.com/Kyonru/tua-lang/releases)

The extension requires VS Code 1.94 or newer. Each published platform package
contains the matching `tua` server binary and uses it automatically without
downloading another executable during activation.

### Start A Project

1. Install the extension and open a project folder in VS Code.
2. Run **Tua: Initialize Tua Project** from the command palette.
3. Open `src/main.tua` and use normal completion, hover, diagnostics, inlay
   hints, navigation, and formatting.
4. If LOVE 11.x is installed, run **Tua: Run LOVE Project** to build and launch
   the project from a visible VS Code task.

For an existing project, open the folder containing `tua.toml`. The extension
starts the bundled server and discovers the project configuration
automatically.

### Optional: LOVE Debugging

Install
[Second Local Lua Debugger](https://marketplace.visualstudio.com/items?itemName=ismoh-games.second-local-lua-debugger-vscode)
to use breakpoints, stepping, variables, watches, and the debug console. Then
run **Tua: Debug LOVE Project**. If the debugger is missing, Tua offers to open
its installation flow.

Tua still executes generated Lua. The extension builds the project, emits
standard source maps, and maps `.tua` breakpoints and stack frames through the
Lua debugger. Keep `emit_maps = true`, which is the default.

### Install A VSIX Manually

Download the VSIX matching your operating system and architecture from
[GitHub Releases](https://github.com/Kyonru/tua-lang/releases). In VS Code, open
the Extensions view, choose **Install from VSIX...**, and select the downloaded
file.

Release assets include individual checksums and a `SHA256SUMS` manifest. See
[Releases And Compatibility](releases.md) for verification commands and the
support policy.

### Select Another Server

The bundled server is the default for published VSIX builds. To use a local or
development binary instead, run **Tua: Select Server** or set
`tua.server.path`. Unbundled development installs fall back to `tua` on
`PATH`.

## Standalone CLI Release

Tagged releases also provide standalone `tua` server/CLI archives for Linux
x64, Windows x64, macOS Intel, and macOS Apple silicon. Choose the archive for
your operating system and architecture from
[GitHub Releases](https://github.com/Kyonru/tua-lang/releases), verify its
checksum, and place the `tua` executable on `PATH`.

Use the standalone CLI when you want Tua in a terminal, build script, or editor
other than VS Code.

## Build The CLI From Source

Building from source requires Rust and Cargo. From the repository root:

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

During development, commands can run directly through Cargo:

```bash
cargo run -p tua-cli -- --help
```

For example:

```bash
cargo run -p tua-cli -- init
cargo run -p tua-cli -- check
cargo run -p tua-cli -- build
```

## Extension Development

Developing or testing the extension requires Node.js as well as Rust/Cargo for
the server. The extension source lives in `editors/vscode`.

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

!!! info "Testing the LOVE debugger bridge"
    Automated extension tests intentionally disable every installed extension.
    To test **Tua: Debug LOVE Project** manually, install Second Local Lua
    Debugger in the same VS Code build and profile, then launch **Run Extension
    (All Examples)** or **Run Extension (Debug Server)**. Both development
    profiles keep installed extensions enabled so the `lua-local` debug adapter
    is available.

Build a VSIX with the current platform's Tua server binary:

```bash
make extension-package
```

Install it with:

```bash
make extension-install
```

## Documentation Development

Building or serving the Zensical documentation requires Python 3.

Serve the docs locally:

```bash
make docs
```

Build the static site:

```bash
make docs-build
```

The docs script creates `.venv/docs` and installs `zensical` there when needed.
