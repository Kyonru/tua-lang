# Tua 0.6.0 final release pass — 2026-11-08

## Assessment

The local macOS Apple silicon candidate passed the final full release gate,
installed-VSIX language checks, native LOVE smoke tests, Busted example, and
visual review. Three issues were found and repaired: one preview-layout defect,
one qualification-harness defect, and stale roadmap entries. Both normally ignored native shader integration tests also passed when explicitly
enabled with installed glslangValidator and LOVE. No new language
correctness defect was reproduced during this pass. Final post-fix `make ci-check` passed on an unchanged retry after a mock-process
startup timeout. See [verification](VERIFICATION.md) for both outcomes.

This is local candidate qualification, not authorization to publish or proof of
universal correctness. Cross-platform release jobs, independent-project
upgrade/rollback, prolonged editing, and installed LuaCov/continuous-test UI
qualification remain outstanding. No tag, push, release, registry publication,
or modification to the user's normal editor profile was performed.

Baseline: `22b2a1da`, clean working tree. Product/test/gallery revision:
`7652a810`. All four changes use unsigned local commits without co-author trailers.

## Findings and repairs

### R060-01 — VSCodium installed-package qualification could not start

Low severity; confirmed test-harness defect, fixed in `4cfa803c`.

The `TUA_TEST_VSIX` runner asked test-electron for a CLI path. That library
returned the Microsoft `.../Resources/app/bin/code` name for the explicitly
selected VSCodium app. Installation failed with ENOENT before any product test.
The shared test helper now uses the existing sibling `codium` when `code` is
absent; arguments and temporary-profile isolation are preserved. It retains
normal Code behavior and does not invent a missing executable.

Files: `editors/vscode/test/vscode-executable.js`,
`test/real-server/run.js`, and `test/unit/vscode-executable.test.js`.
Three new tests cover VSCodium, Code, and missing CLIs. The final VSIX subsequently
installed and passed real-server requests in an isolated VSCodium profile.
This was not a defect in ordinary extension activation.

### R060-02 — Shipped Busted declarations still appeared as pending work

Low severity; confirmed documentation/planning defect, fixed in `065bbf37`.

Two roadmap bullets duplicated a request for test-scoped Busted/LuaAssert
declarations already implemented in `crates/tua-core/src/busted.rs`, tested in
core/LSP suites, and described in `docs/testing.md`. Removed those completed
items. LOVE-specific test mode and installed continuous-run/coverage
qualification remain explicitly pending. Public typing docs were already correct.

### R060-03 — Backward state-machine transitions escaped the initial viewport

Medium severity; confirmed editor usability defect, fixed in `b69b7169`.

Open the Starfall state-machine preview in a 1440×960 editor. The three state
nodes were visible, but the backward `play` edge and label ran below the initial
viewport. CSS stretches the graph canvas to fill available space; routing used
`canvas.height` instead of respecting the reserved corridor in `layout.height`.
See [pre-fix capture](state-machine-before.png) and the
[post-fix gallery](../../../editors/vscode/docs/screenshots/state-machine.png).

`editors/vscode/src/state-machines/template.js` now bounds the backward corridor
by the calculated layout height. The regression in
`editors/vscode/test/unit/bundled-preview-helpers.test.js` executes the minified
production helper, stretches the canvas by 800 pixels, and checks that the path
and label remain stable, below the nodes and within graph bounds. Earlier tests
exercised routing but did not vary canvas size independently of layout size.

All five production-bundle screenshots were recaptured and visually reviewed in
`7652a810`: populated workspace, complete state graph, sprite playback controls,
module graph, and expanded live store/pool/state values. Captures use a temporary
game/profile, not fabricated runtime values. No errors or personal data were
visible; ordinary panel scrolling remains available for larger content.

## Coverage matrix

| Area | Evidence | Status |
| --- | --- | --- |
| Parser, checker, emitter, modules, domains | Full Rust suite; bundled example builds | Exercised |
| Generated Lua/runtime | Lua syntax/runtime integration and six runtime suites | Exercised |
| Native LOVE | Three actual LOVE runtime example tests; live gallery game | Exercised |
| Shader validation | Native LOVE valid/invalid shaders and glslang fixture corpus | Exercised |
| Busted | Documented example: 3 successes, zero errors/failures | Exercised |
| Editor inference, completion, hover, signatures, navigation | Core/LSP tests, installed VSIX, real-server smoke | Exercised |
| Incomplete edits/cache freshness | Existing and newly added regression tests; editor regression smoke | Exercised |
| Independent project typing | Read-only main-then-state unsaved-buffer replay | Exercised, protocol only |
| Client activation/settings/actions | Unit/activation and extension-host suites | Exercised |
| Views and minified bundle | Five actual graphical views, geometry tests, gallery provenance | Exercised |
| Docs and discovery | Language/typing/classes/modules/comments/testing, README/manifests and releases reviewed; docs build | Exercised/inspected |
| Copied documentation examples | Four complete Tua comments examples and first class example checked | Exercised, not every snippet |
| Version/config/annotation metadata | Drift checks and release-note generator | Exercised |
| Dependency advisories | Cargo audit and npm audit | Exercised |
| Release pipeline | Trusted CI required before artifact jobs; checksums/attestations and platform matrix inspected | Inspected |
| Windows/Linux/Intel macOS binaries | Trusted platform CI required at release | Not run locally |
| Standalone LuaJIT oracle | No standalone LuaJIT on PATH; Linux CI pins it | Not run locally |
| Coverage UI | LuaCov unavailable in the local Lua environment | Blocked |
| Long sessions, real-project upgrade/rollback | Not established by these finite checks | Pending |

## Documentation and release consistency

The version is synchronized at 0.6.0. Release notes generated from CHANGELOG.md
include Busted/LuaCov prerequisites, optional/nil checking changes, class
initialization behavior, and the explicit import-cleanup caveat. Generic and
protocol abstractions are not advertised as shipped. Comments/annotations docs
distinguish implemented typing from metadata-only LuaLS tags. Offline annotation
catalog/config drift checks pass as part of the release gate.

`release.yml` requires the reusable trusted `ci.yml` before platform packaging;
platform binaries are version-smoked, and artifacts receive checksums and
attestations. This pass inspected those controls; it did not execute remote CI.
No `v0.6.0` tag existed during the check. The local generic VSIX contains the
macOS arm64 executable; the release workflow builds separate platform packages.

## Remaining risks and release follow-ups

1. Require green trusted platform/release jobs before publication. Validate the
   downloaded published artifacts and attestations afterward, as the repository
   release checklist requires; this local VSIX cannot substitute for that.
2. Complete an independent-project upgrade/rollback trial and prolonged editing
   qualification. The existing roadmap records unexplained RSS growth in an
   earlier ten-minute BYTEPATH replay; this pass did not resolve it.
3. Completion no longer stalls in the reported project, but is not uniformly
   instantaneous: initialization 733 ms, first completion 224 ms, subsequent
   probes 1–337 ms during this audit. Chain queries were 161/317 ms. These are
   individual protocol measurements on a machine doing other verification,
   not percentile benchmarks or key-to-menu GUI latency. See
   [raw replay](project-completion.json). Preserve dependency freshness while
   profiling further; do not relax correctness to meet a timing target.
4. Real LuaCov coverage and continuous-testing UI remain unqualified here.
   `require("luacov")` fails in local Lua 5.4.8. The normal Busted path passed.
5. npm audit reports zero vulnerabilities. Cargo audit passes but reports the
   existing allowed `paste 1.0.15` unmaintained warning (RUSTSEC-2024-0436).
   This is a maintenance risk, not evidence of an exploitable vulnerability.

No new high-severity blocker remains from the reproduced findings above.
The pending qualification items prevent an unconditional all-platform,
long-session release sign-off.
