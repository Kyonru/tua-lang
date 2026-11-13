# Tua 0.8.0 local release-readiness audit

Date: 2026-11-13 (JST). Starting revision: `03ad8603`. Product candidate revision:
`e5906bcb`. Host: macOS arm64, Rust stable, Node 24, VSCodium, LÖVE 11.5.
This is a **local release candidate**, not a published-release certification.
No tag, push, registry upload, or GitHub release was created.

## Decision

The candidate passes the local CI gate, graphical editor and LÖVE checks, and
packaged-server smoke tests. No local product blocker remains from this audit.
Before publishing, the release checklist still requires the tagged Linux,
Windows, and macOS CI matrix, provenance attestations, checksums, a maintainer
download of published artifacts, and registry version checks. Qualification in
an independent game project and a representative previous-version migration
remain limited; the disposable upgrade smoke below covers only a new minimal
project. Do not describe those release-only or pilot checks as passed.

## Findings resolved here

| Finding | Evidence | Resolution |
| --- | --- | --- |
| A newly generated game-packaging workflow selected the old `v0.7.0` compiler and default project config omitted `package_pipelines`. | Two core config tests failed in the initial full gate; a fresh 0.8.0 `tua actions init` workflow contained the stale tag. | `772d8fd0` restores the schema default and renders the CLI package version into the workflow. Fresh output now uses `v0.8.0`. |
| Parallel shader runtime tests intermittently shared a temporary project. | Repeated `shader_scan` runs showed output contamination or a missing build directory despite successful builds. | `02ccb7cf` gives each fixture a process-local unique ID and creates its directory exclusively. Ten consecutive focused runs passed. |
| Formatting an inline-closing table argument leaked indentation into subsequent statements. | `examples/audio-system/src/main.tua` failed `tua fmt --check` in full CI; a two-call reproduction gained two then four spaces on following locals. | `8082c678` tracks open table block depths and closes them on suffix `})`; added a regression and idempotence assertion. All 68 formatter tests and the audio example format check pass. |
| A shader-validator timeout test could fail under parallel test scheduling. | The first exact-revision CI rerun failed `headless_timeout_applies_per_process_not_per_batch` even though its 0.3-second child usually finishes within the two-second deadline; the prior full run passed. | `e5906bcb` keeps the total batch longer than one timeout while allowing more per-process startup margin. Three focused repetitions and the final full CI run passed. This changes the test oracle, not the production timeout. |

`2077a005` synchronizes the 0.8.0 Cargo and extension versions and prepares
release notes. `01c7220c` refreshes five README screenshots from a graphical
VSCodium session using the 0.8.0 server and extension. The screenshot capture
script now moves the pointer off the Project tree before capture so a tooltip
does not obscure the image. Images were visually reviewed for populated data,
legibility, clipping, and errors; the artifact hash check passed.

## Verification

| Check | Result and scope |
| --- | --- |
| `make ci-check` after final product edit | Passed. Includes `cargo fmt`, Clippy, RustSec policy, workspace tests (1,948 core, 455 LSP, 148 CLI tests among the suite), all bundled example builds, LSP feature smoke, extension tests, 466 tracked JSON files, docs build, representative completion/performance smoke, and inline-completion evaluation. Core reports one pre-existing ignored test; CLI API reports two ignored tests. Final log: `/tmp/tua-release-audit-ci-080-exact2.log`. |
| `make love-runtime-test` | Passed three LOVE example tests in the installed runtime. |
| `make runtime-test` | Passed plain Lua runtime suite before the final formatter-only edit; emitted Lua paths were unchanged by that edit, and the later full CI runtime tests passed. |
| Graphical extension host | `test:vscode`, `test:webviews`, and `test:vscode-real` passed with the staged 0.8.0 server. The final `tua-0.8.0.vsix` was then installed in a disposable VSCodium profile and `node editors/vscode/test/real-server/run.js` passed with `TUA_TEST_VSIX`, exercising the **bundled** server, completion, hover, navigation, formatting, and ECS/pool/Signal preview navigation. Host coverage is selected workflows, not every command or long session. Log: `/tmp/tua-release-audit-installed-vsix-080.log`. |
| `make extension-screenshots-check` | Passed for five 0.8.0 graphical captures. |
| `npm audit --prefix editors/vscode --audit-level=high` | Passed; zero reported vulnerabilities. |
| RustSec (`make ci-check`) | Passed repository policy with two allowed warnings: unmaintained `paste` and `rustix` unsoundness advisory in a test dependency path. Revisit with upstream dependency updates; no new unallowed advisory. |
| Release tooling | `make version-drift-check release-tool-test docs-build diff-check` passed. Changelog notes generation for 0.8.0 succeeded. |
| Package integrity | `make extension-package`, `unzip -t`, manifest inspection, and `scripts/smoke_release.py` on a local macOS arm64 archive plus the final VSIX all passed. The package contains `extension/bin/tua`, `extension/dist/extension.js`, editor docs, and offline LOVE API data. Both extracted servers initialized, checked, and built a disposable project. |
| Game workflow | Fresh 0.8.0 `tua init`, `tua actions init`, `tua check`, and `.love` packaging passed; generated workflow parsed and selected `v0.8.0`. It cannot run the hosted compiler tag until that tag exists. |
| Minimal upgrade and rollback | The bundled 0.7.0 server initialized, checked, and built a disposable project; the 0.8.0 release server checked and built it; 0.7.0 then checked and built it again. This does not cover a substantial older game or editor settings migration. |

### Performance comparison

The same representative smoke fixtures and repetition settings ran against the
packaged 0.7.0 server and the 0.8.0 release server on this host. These are LSP
protocol timings, **not typing-to-visible-menu latency**. A single cold sample
per fixture is not a stable tail-latency estimate.

| Fixture | 0.7.0 cold/warm completion p95 | 0.8.0 cold/warm completion p95 | 0.7.0 / 0.8.0 sampled peak RSS |
| --- | --- | --- | --- |
| 1,000-line file | 288.07 / 6.75 ms | 287.65 / 7.15 ms | 250.1 / 235.9 MiB |
| 100-file project | 162.20 / 0.36 ms | 161.48 / 0.38 ms | 310.3 / 312.4 MiB |

Both runs had zero stale completion responses. The CI workload passed its
completion and memory budgets. The small differences above are within ordinary
single-run variation; they do not establish a performance improvement or
regression. Earlier installed-host Starfall work recorded a roughly 125 ms
warm typed-dot to visible menu median on then-current source, but this final
VSIX was not retimed at the renderer boundary.

## Artifact

Installable local test package:
`editors/vscode/tua-0.8.0.vsix` — SHA-256
`8d540fbe303364826892b0c4ab8bd3dbf9db4b16228fc33b1cad8c27b0c1d91b`.
It is ignored by Git and intentionally remains uncommitted. Local macOS arm64
standalone archive smoke artifact:
`/tmp/tua-release-audit-dist/tua-0.8.0-aarch64-apple-darwin.tar.gz` — SHA-256
`8c7015c1d65e81791b41f1b9e483bc54167943f93e26e52e19def275e6561ccd`.
These are local test artifacts; published workflow artifacts may have different
hashes and must be verified separately.

## Remaining release gates

1. Review the release commit and annotated `v0.8.0` tag, then run the trusted
   cross-platform CI and release workflows. Check LuaJIT oracle, Windows and
   Linux packages, artifact checksums, attestations, and tag-specific notes.
2. Have a maintainer download one published archive and one VSIX, verify their
   checksums and attestations, and exercise the packaged binary.
3. Confirm Visual Studio Marketplace and Open VSX show the same version and
   platform packages as the GitHub release.
4. Exercise check/build/format/watch/dev/packaging/editor workflows in a real
   independent game project and perform an upgrade/rollback trial there. The
   disposable fixture is deliberately too small to certify that path.
5. Run a longer editor/inspector session and fresh visible completion timing
   on the final installed VSIX if release scheduling permits. Existing host
   and soak reports remain useful risk evidence, not a substitute for that run.

No private game source was opened or copied into this report.
