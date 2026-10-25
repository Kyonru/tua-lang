# Development and verification

Start with [`CONTRIBUTING.md`](../CONTRIBUTING.md). This page collects
specialized repository workflows that users do not need in the product docs.

## Main checks

Use focused checks while iterating, then run the appropriate integration gate:

```bash
make fmt-check
make test
make verify
make ci-check
```

`make verify` checks formatting, Rust tests, strict Clippy, extension
activation, generated metadata, JSON, and whitespace. `make ci-check` adds
bundled examples, documentation, and benchmark smoke tests. Run
`make diff-check` before committing.

## VS Code extension

Extension work requires Node.js plus Rust/Cargo for the server:

```bash
make extension-deps
make extension-check
make extension-test
make extension-package
```

`extension-test` launches an isolated Extension Development Host. Set
`VSCODE_EXECUTABLE_PATH` to choose a VS Code build. For the real LOVE debugger
bridge, install Second Local Lua Debugger and run:

```bash
make love-debugger-e2e LOVE=/path/to/love
```

The debug launch profiles build the unminified extension bundle with source
maps before starting the host.

## Documentation

```bash
make docs
make docs-build
```

The documentation script manages its Zensical environment under `.venv/docs`.
Public pages live under `docs/`; contributor-only plans and evidence belong in
this directory.

## Language server and performance

```bash
cargo test -p tua-lsp
make lsp-smoke
make lsp-features-smoke
make editor-regression-smoke
```

Use [Completion benchmarks](completion-benchmarks.md) for profiling and
contention tests. Generated reports belong under ignored `target/`,
`artifacts/`, or `reports/` paths rather than version control.

## Examples and generated fixtures

`make build-examples` checks every buildable example. `examples/linter` is an
intentional diagnostic fixture and exits unsuccessfully when linted.

The checked-in `benchmarks/lsp/{small,medium,large}` projects and generated
fixtures under `target/` support latency and cache testing. Use the benchmark
guide rather than the public examples page for profiling commands.

## Generated catalogs and diagnostic codes

LOVE API catalogs come from the revision pinned in
`scripts/love-api-revision.txt`:

```bash
make generate-love-api LOVE_API_ROOT=/path/to/love-api
make love-api-drift-check LOVE_API_ROOT=/path/to/love-api
```

When changing a diagnostic, update `DiagnosticCode` and its registry in
`crates/tua-core/src/diagnostic/mod.rs`, add focused core and LSP tests, and
update the public diagnostic and configuration references.

## Real-server editor gate

Trusted CI and release verification run `make extension-real-server-test` in
addition to the mock-server host suite. It stages the release server and tests
actual Lua/Tua/LOVE completion, navigation, diagnostics and editor behavior in
VS Code. Run it locally after changes that cross the extension/LSP boundary.

Release packaging additionally runs `scripts/smoke_release.py` on both archive
and VSIX server bytes. The Linux release job installs the built VSIX into an
isolated extensions directory and runs the real-server suite using bundled
server discovery. To reproduce, set `TUA_TEST_VSIX` to a local VSIX and run
`node editors/vscode/test/real-server/run.js`.
