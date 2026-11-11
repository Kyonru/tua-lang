# Installed sidebar and inspector soak recheck

Starting product revision: `6f76afb66affd5c2aa996b5a4e9b43fa36d29823`; November 12, 2026, macOS arm64. This addendum closes two qualifications in the November 11 [verification log](VERIFICATION.md). The VS Code host was 1.142.0. The tests used a disposable synthetic project; no independent game project was edited.

## Project files and Modules

The initial real-server host replay exposed a Medium editor defect. After an external `helper.lua` export changed from `old` to `fresh`, Modules still showed `old` for the full ten-second wait. `tua.workspace.refresh` immediately showed `fresh`. The sidebar's filesystem watcher could run before the language client's watcher advanced the server's analysis revision, so its cached graph held the pre-change response. The existing unit tests simulated invalidation but not the order of those two watchers.

The sidebar now records the last graph `analysisRevision` and makes bounded follow-up refreshes while a filesystem event still returns that same revision. A focused unit test reproduces a stale response followed by an advanced one. The permanent [host suite](../../../editors/vscode/test/real-server/suite/sidebar-filesystem.js) is included in `test:vscode-real` and checks eight steps against the real server: initial Lua visibility in Project files; external Lua create/delete; initial module dependencies; an unsaved dependency add/remove; external Lua export change; and Tua module deletion.

Run the development-host gate:

```sh
TUA_REAL_SERVER_PATH="$PWD/target/release/tua" npm --prefix editors/vscode run test:vscode-real
```

The final installed-package check used the current `editors/vscode/tua-0.7.0.vsix` in a fresh VS Code profile, with its bundled server and no development server override:

```sh
TUA_TEST_VSIX="$PWD/editors/vscode/tua-0.7.0.vsix" \
  TUA_SIDEBAR_EVIDENCE="$PWD/internal/audits/feature-followup-2026-11-11/sidebar-host.json" \
  node editors/vscode/test/real-server/sidebar-run.js
```

The installed replay passed twice, with all eight steps in each run. [Captured rows](sidebar-host.json) show `helper.lua` and `added.lua` in Project files, the dependency count changing 1 → 2 → 1 on unsaved edits, `fresh` under the changed Lua module, and `extra.tua` removed after deletion. The VSIX passed `unzip -t`; SHA-256: `ec081f69d80ebf8f301242d4e08f38c70f6245e2868fb143fd6abc868541387a`. These checks verify rendered host state for this fixture, not a large or multi-root game workspace.

The checksum above identifies that historical package. After the separate
FUP-01 class-completion fix, `tua-0.7.0.vsix` was rebuilt and replaced it.
The newer package has SHA-256
`094ecf7d460640f0ab3ca0eb04011bb332e024dfc306330f89ea69b1955fdc9a`;
both installed real-server suites passed again with the newer package.

## Inspector retained-memory assertion

The prior ordinary `node editors/vscode/scripts/runtime-inspector-soak.js` run failed its 2 MiB Node heap-range assertion with a 6,988.6 KiB range. Its samples mixed pre-collection and post-collection heap because `global.gc` was unavailable. The same workload with explicit `--expose-gc` passed with a 423 KiB range. This is a test-oracle difference, not evidence of retained-memory growth.

The ordinary command now relaunches the script with `--expose-gc` when needed, calls GC before each Node heap sample, and labels the metric `post-gc`. A 120-second ordinary-command check passed (213 snapshots, three reconnects, 145.5 KiB Node heap range). The full ordinary command also passed:

```sh
TUA_SOAK_OUTPUT="$PWD/internal/audits/feature-followup-2026-11-11/inspector-soak-post-gc.json" \
  node editors/vscode/scripts/runtime-inspector-soak.js
```

Its [raw result](inspector-soak-post-gc.json) records 600 seconds, 1,126 snapshots, 19 forced reconnects recovering in 1,090–1,127 ms, a 451.8 KiB post-GC Node heap range, 21.5 KiB active LOVE heap growth, −0.5 KiB no-shim LOVE heap growth, and zero errors. The retained-memory assertion passed. Post-GC `heapUsed` is a repeatable retained-memory proxy for this workload, not a heap-dominator analysis or a multi-hour game-session result.

## Other checks and limits

- `npm --prefix editors/vscode run check`: passed, 190 tests plus syntax, grammar, theme, manifest, launch, and activation checks. The first restricted-sandbox attempt failed two inspector tests because loopback binding returned `EPERM`; the rerun with local loopback allowed passed.
- `node --check` on both new host scripts and `git diff --check`: passed.
- `make extension-package`: passed and produced the tested VSIX.
- `TUA_REAL_SERVER_PATH=... npm --prefix editors/vscode run test:vscode-real`: passed with the new sidebar replay in the real-server gate.

Still unqualified: visible latency in a real game, large compiler-produced graph fidelity, non-PNG native previews, multi-hour inspector use, and Windows/Linux installed hosts. The cross-file inferred forwarding-function completion finding [FUP-01](FINDINGS.md) was fixed in a later November 12 pass; its pre-fix evidence remains separate.
