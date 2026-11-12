# Verification and limits

## Revision and worktree

- Started at `b21dfb48`, clean worktree. The audit used only disposable VS Code projects and `/tmp` host logs/screenshots; no independent game project was read or edited.
- Product fixes: `5276afe6` (module filter layout) and `9eb0aafd` (health measurement labels). The screenshots in `captures/` come from the current VS Code host run before and after those changes. The audit documents are a separate deliverable.
- The historical [November 11 preview review](../feature-and-preview-2026-11-11/PREVIEWS.md) and [November 12 host/soak addendum](../feature-followup-2026-11-11/HOST_AND_SOAK_RECHECK.md) are cited as prior evidence. Their installed VSIX checksums identify older packages, not the current source revision.

## Commands and results

| Command | Result | What it establishes |
| --- | --- | --- |
| `node --test editors/vscode/test/unit/server-health-view.test.js` | Pass, 1 test | Unmeasured versus measured cache, warm, and latency labels. |
| `npm --prefix editors/vscode run check` | Pass with loopback permitted, 195 unit/script tests and extension checks. Initial restricted-sandbox run failed 2 inspector socket tests with `EPERM`; the permitted rerun passed. | JS syntax, manifest/theme/grammar, unit behavior. |
| `TUA_PREVIEW_SCREENSHOTS=/tmp/tua-preview-audit-final6 npm --prefix editors/vscode run test:webviews` | Pass in VS Code 1.142.0 on macOS arm64. | Production webview templates, populated sprite/state/tree/signal/ECS/pool/audio fixtures, empty and error views, narrow captures, keyboard focus, source messages, refresh persistence, and no script errors. The host's workbench viewport override sometimes leaves a mounted iframe at its prior width, so screenshots and assertions are distinguished. |
| `TUA_REAL_SERVER_PATH="$PWD/target/release/tua" npm --prefix editors/vscode run test:vscode-real` | Pass, including `REAL_SERVER_GRAPH_PREVIEWS` and all 8 sidebar filesystem replay steps. | Actual Tua server → extension → rendered Signal/ECS/Pool data and source navigation in a disposable project, plus module/sidebar edit recovery. This does not measure a large game. |
| `npm --prefix editors/vscode run test:vscode` | Pass, including setup integration. | Extension activation and native VS Code commands. It is not a comprehensive media preview or accessibility run. |
| `make verify` | Pass. Rust workspace tests (including 1,936 core and 454 LSP tests), Clippy with warnings denied, examples/config/version/docs drift checks, 195 extension tests, grammar/theme checks, JSON and whitespace checks passed. Existing ignored Rust tests remained ignored. | Main local repository gate. |

`test:webviews` initially failed under the restricted sandbox because its loopback debugging listener received `EPERM`; the permitted run passed. During the module filter test, a too-strict test asserted that a workbench width override always resized the existing iframe. The browser reported an 846 px iframe while the workbench was 640 px. That test-oracle assumption was corrected; the final assertion checks wrapping when the iframe itself is narrow. One attempted CDP emulation command on the iframe was rejected because it is not a top-level target. Neither is a product failure.

## Coverage limits

- Host-rendered synthetic fixtures are **not** proof of compiler fidelity for large graphs or every media decoder. Current actual-server host coverage checks small Signal/ECS/Pool and module/sidebar cases, not all views.
- The screenshots show dark-theme populated data and light/high-contrast class transitions after an error page. Contrast and focus in every populated high-contrast view, actual screen-reader speech, 200% zoom, and reduced-motion playback remain untested.
- Loading/error/stale states are covered by a mix of host tests, `preview-refresh.test.js`, and source inspection. They were not each repeated in every view and theme.
- The ten-minute inspector soak in the prior addendum used synthetic snapshots and a post-GC heap proxy; no long real-game session or Windows/Linux installed-host run occurred here.
- No audio playback, sprite image metadata parity across unusual formats, offline reference search/focus, or generated-Lua source-map round trip was newly established in this pass.

## Reproduction commands for the two fixes

PV-01: run `TUA_PREVIEW_SCREENSHOTS=/tmp/tua-preview npm --prefix editors/vscode run test:webviews`, open `module-graph-narrow.png`, and compare the four role filter buttons with [before](captures/module-before.png) and [after](captures/module-after.png). The old `Roots` control was clipped; the new layout exposes all four in two rows.

PV-02: run `node --test editors/vscode/test/unit/server-health-view.test.js` and the same host command. With a health response containing only `{ "revision": 1 }`, [before](captures/health-before.png) showed a `0ms` recent p95; [after](captures/health-after.png) says “No samples.” A response with nonzero counts keeps numeric rates/latencies.
