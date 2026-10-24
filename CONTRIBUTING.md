# Contributing to Tua

Thank you for helping improve Tua. Contributions to the compiler, CLI,
language server, VS Code extension, documentation, examples, and tests are all
welcome.

For a bug or support request, use the matching GitHub issue template first. Do
not put vulnerability details in a public issue; follow [SECURITY.md](SECURITY.md)
instead.

## Before You Start

- Check the open issues and [ROADMAP.md](ROADMAP.md) for related work.
- Keep a change focused. If a language or editor fix exposes a broader design
  question, describe that question before building a parallel one-off path.
- For substantial language syntax, compatibility, architecture, or product
  changes, open an issue before investing in an implementation.

Local development uses:

- the stable Rust toolchain and Cargo;
- Node.js 22 for extension checks and JavaScript tooling;
- Python 3 for documentation and release-tool checks;
- Lua and LOVE only for the optional runtime checks that need them.

Build the CLI and install the extension's locked dependencies from the
repository root:

```sh
cargo build -p tua-cli
npm ci --prefix editors/vscode
```

`make docs-build` creates a Python virtual environment under `.venv/docs` and
installs Zensical when it is not already available.

Specialized benchmark, extension, generated-catalog, and evaluation workflows
are indexed in [`internal/README.md`](internal/README.md).

## Make A Change

1. Add or update a focused regression test at the layer that owns the behavior.
2. Implement the smallest coherent fix. Shared parser, semantic, project, and
   VFS facts belong in `tua-core`; LSP code should adapt those facts to the
   protocol rather than create a second language engine.
3. Update user-facing documentation, examples, diagnostics references, and the
   `[Unreleased]` changelog when behavior changes.
4. Run focused checks while iterating, then the relevant integration checks.
5. Review the diff for generated files, accidental build output, credentials,
   private paths, and unrelated formatting.

Useful focused checks are:

| Change area | Command |
| --- | --- |
| Parser, checker, inference, lowering, emission, or IDE facts | `cargo test -p tua-core` |
| CLI, builds, source maps, asset importers, or watch mode | `cargo test -p tua-cli` |
| LSP routing, snapshots, scheduling, or protocol conversion | `cargo test -p tua-lsp` |
| Editor activation and JavaScript behavior | `make extension-check` |
| Real editor and closed-file behavior | `make editor-regression-smoke` |
| Documentation | `make docs-build` |
| Bundled examples | `make build-examples` |
| Main local suite | `make verify` |
| Full trusted CI-equivalent suite | `make ci-check` |

`make ci-check` also requires `cargo-audit` 0.22.2 so the exact locked Rust
dependency graph can be checked against RustSec:

```sh
cargo install cargo-audit --locked --version 0.22.2
```

Run `make diff-check` before submitting. `make extension-test` launches a VS
Code Extension Development Host and may download a matching test build of VS
Code. The generated LOVE catalog has its own pinned-source check; see
`make help` and `docs/installation.md` for its prerequisites.

## Generated Files

Do not edit generated surfaces independently of their source. The common
regeneration and drift checks are:

```sh
make generate-config-schema
make config-schema-drift-check
make generate-llms
make llms-drift-check
make sync-version
make version-drift-check
```

LOVE API catalogs are generated from the pinned revision recorded in
`scripts/love-api-revision.txt`. Use `make generate-love-api
LOVE_API_ROOT=/path/to/love-api` only with a checkout at that revision.

## Security Boundaries

Treat project files, configuration, asset paths, symlinks, protocol messages,
and media as untrusted inputs. Changes at those boundaries should fail closed,
use explicit size or time limits, and include adversarial regression tests.
Tua must not execute source code to analyze it.

External asset-pipeline commands are explicitly configured project tools, not
a sandbox. Do not run `check`, `build`, or `watch` on an untrusted project that
configures one. In VS Code Restricted Mode, preserve the rule that only the
bundled language server may start; custom server paths, LOVE commands, and
project initialization require workspace trust.

See [SECURITY.md](SECURITY.md) for the supported security model and private
reporting instructions.

## Pull Requests And Automation

Open or untrusted pull requests intentionally do **not** execute repository
code in GitHub Actions. This avoids granting untrusted changes a runner-based
execution path. Do not add `pull_request` or `pull_request_target` workflows as
a workaround.

Because the hosted checks will not run on an open pull request, include the
exact local commands and results in the pull-request description. Maintainers
run the trusted workflow from reviewed code through a push to `main`, manual
dispatch, or release invocation.

A good pull request includes:

- the problem and user-visible effect;
- the chosen fix and important tradeoffs;
- regression tests and documentation updates;
- local verification commands and results;
- any platform-specific behavior that was not exercised.

Keep release credentials out of source, logs, fixtures, and pull-request
artifacts. Release and registry publishing are maintainer operations: tagged
releases must pass the trusted CI workflow before packaging, and extension
publishing consumes checksum-verified assets from a completed GitHub release.
See [docs/releases.md](docs/releases.md) for the release procedure.

By contributing, you agree that your contribution is licensed under the
repository's [MIT License](LICENSE).
