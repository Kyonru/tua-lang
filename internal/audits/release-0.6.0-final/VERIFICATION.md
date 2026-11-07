# Verification log

All commands below run from the repository root unless indicated. Local
candidate platform is macOS arm64. Revision `7652a810` contains the last product,
test, roadmap and screenshot changes; later report commits change only evidence.

## Main gates

```sh
CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 make ci-check
npm audit --prefix editors/vscode --audit-level=high
CARGO_INCREMENTAL=0 make extension-package
unzip -t editors/vscode/tua-0.6.0.vsix
make extension-screenshots-check
```

- First complete CI gate passed. It ran during the investigation and is not
  the final post-fix qualification by itself.
- First post-fix CI attempt passed workspace tests, 176 extension tests and
  earlier gates, but activation smoke failed: the newly generated mock
  `tua-smoke --version` exceeded its deadline. No product timeout was changed.
  The subsequent unchanged full-gate retry passed, exit 0.
- Final `make ci-check`: passed. Workspace tests: **2,348 passed**, zero
  failed, three explicitly ignored. Extension unit tests: **176 passed**.
  Example builds, real LSP regression smoke, docs build, Clippy, metadata gates,
  representative/ranking benchmarks and both inline evaluator gates passed.
  The two ignored native shader tests passed separately; the manual latency
  probe remains unrun. The representative synthetic contention gate measured
  completion p95 30.62 ms, with zero stale results (not a real-project GUI result).
- npm audit: zero vulnerabilities.
- Cargo audit: no vulnerability failure; existing allowed `paste 1.0.15`
  unmaintained warning, RUSTSEC-2024-0436.
- Final VSIX package and ZIP integrity: passed. Package version 0.6.0,
  40 files, approximately 8.24 MB.
- Screenshot artifact check: five matching hashes, 1440×960 images, matching
  0.6.0 compiler/extension and bundle hash. Every image was visually inspected.

## Actual editor qualification

```sh
VSCODE_EXECUTABLE_PATH=/Applications/VSCodium.app/Contents/MacOS/VSCodium \
TUA_TEST_VSIX="$PWD/editors/vscode/tua-0.6.0.vsix" \
node editors/vscode/test/real-server/run.js

VSCODE_EXECUTABLE_PATH=/Applications/VSCodium.app/Contents/MacOS/VSCodium \
npm --prefix editors/vscode run test:vscode

CARGO_INCREMENTAL=0 make extension-server
VSCODE_EXECUTABLE_PATH=/Applications/VSCodium.app/Contents/MacOS/VSCodium \
make extension-screenshots
```

All passed. The installed-VSIX runner installs into a temporary extensions and
user-data directory, and uses the bundled server without a development override.
See `installed-editor.json` for returned completions, hover/signature, navigation,
formatting and inlay counts. Extension-host tests use mock protocol fixtures;
they complement, not replace, the installed real-server run.

The gallery's state-machine regression was reproduced before the fix. After the
fix, all routes and labels are visible in the initial view. Sprite frames and
controls are populated; module topology is populated; live inspector displays
actual state, pool and store values from the temporary LOVE game. Capture
provenance is `editors/vscode/docs/screenshots/capture.json`.

## Language, runtime and documentation probes

```sh
make runtime-test
CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 \
TUA_LOVE_BIN=/Applications/love.app/Contents/MacOS/love make love-runtime-test
CARGO_INCREMENTAL=0 TUA_LOVE=/Applications/love.app/Contents/MacOS/love \
cargo test -p tua-cli --lib real_ -- --ignored --nocapture
(cd examples/unit-testing && ../../target/release/tua test)
python3 internal/audits/release-0.6.0-final/check-doc-examples.py
python3 scripts/package_release.py notes --changelog CHANGELOG.md \
  --version 0.6.0 --output /tmp/tua-060-release-notes.md
```

- All six bundled runtime suites passed under local Lua 5.4.8.
- Three LOVE example tests passed with actual runtime execution enabled.
- Two normally ignored native shader tests passed (glslang corpus and LOVE
  valid/invalid/reused-session validation).
- Busted example: 3 successes, zero failures/errors/pending.
- Five complete documentation blocks passed checking in isolated projects.
  Fragmentary snippets were inspected rather than claimed as executable tests.
- Release-note generation passed and was inspected for migration guidance and
  tagged changelog/checksum links. Generated text is preserved in `release-notes.md`.

The real-project probe uses the existing read-only script:

```sh
node internal/audits/typing-completion-2026-11-08/probe.js \
  target/release/tua /absolute/path/to/zombie src/core/state.tua src/main.tua
```

It edits only LSP buffers, opens main before state, and does not execute or alter
the game. Results are in `project-completion.json`; they are protocol latency,
not GUI latency or a statistical benchmark.

## Artifact identity

Final local VSIX SHA-256:
`59c1e9f3e9ef465778e2ae43b14a9986073c92b3947bd956dc9896b337cb2bdc`

Packaged server SHA-256:
`9ea4fc50ab2b8e931094923dc9df74a77a60d54ea6c1d92d6ef727bd3e89cced`

Packaged client bundle SHA-256:
`1697de0a7c7be01f6de94232c3dbef419af98ece2c379b0235595fc59ffc4260`

Extracted archive bytes matched the tested release executable and client bundle.
The client hash also matched the screenshot capture manifest. The local VSIX is
macOS arm64; other platform artifacts remain the release workflow's responsibility.

## Explicit limitations

- Remote trusted CI and published artifacts were not run or created here.
- Standalone LuaJIT is unavailable on PATH; local plain-Lua tests used Lua 5.4.8.
  Native LOVE testing exercises the installed LOVE runtime separately.
- LuaCov is not installed; coverage UI and continuous-run interaction remain
  unqualified, rather than counted as passing tests.
- The manual project-corpus latency test remains ignored in the main suite.
- No prolonged session, real-project upgrade/rollback, or all-platform GUI claim.
