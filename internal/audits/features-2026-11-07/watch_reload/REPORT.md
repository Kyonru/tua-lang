# Watch / dev / hot reload audit

Two confirmed medium defects. Polling CLI workflows and the Lua hot-reload regression suite pass their exercised cases. Actual LOVE startup aborted (signal 6, no diagnostic) in this sandbox; GPU shader compilation, visual overlay, actual process restart and GUI/editor tasks remain unverified here.

## Commands and evidence

From repository root (release binary version/baseline in ../BASELINE.md):

```sh
python3 internal/audits/features-2026-11-07/watch_reload/probe.py
python3 internal/audits/features-2026-11-07/watch_reload/native-reconciled.py
python3 internal/audits/features-2026-11-07/watch_reload/config-probe.py
/opt/homebrew/bin/lua internal/audits/features-2026-11-07/watch_reload/runtime.lua
/opt/homebrew/bin/lua internal/audits/features-2026-11-07/watch_reload/entry-probe.lua
/Applications/LOVE.app/Contents/MacOS/love internal/audits/features-2026-11-07/watch_reload/love-runtime
```

Python harnesses create disposable projects only in this directory, launch the absolute release CLI, and terminate/wait for their children in finally blocks. LOVE was invoked through subprocess.run(timeout=15); it exited -6. Lua is 5.4.8. runtime.lua runs the existing vendor suite with its stub LOVE filesystem/graphics; it does not constitute a real GPU test.

| Area | Status / evidence |
| --- | --- |
| Polling watch and dev initial build, saved edit, create/delete, type-error preservation and repair | Passed, results.json, poll.log, dev.log |
| Invalid config retains old layout; valid output-root migration rebuilds | Passed for watch; dev defect below |
| Output nested in source excluded from snapshot | Exercised with invalid ignored.tua in output, no feedback build before config migration |
| Native notifications | 1.2-second checks inconclusive in sandbox (false entries in results.json); all nine checks pass with 6-second waits for reconciliation, native-reconciled-results.json |
| Module/table state, nested class identity over successive reloads, private helper cycles, callback reassignment, failed reload repair | Existing runtime suite passed, lua-runtime.log |
| Shader dependency reload, exported shader replacement, failed compilation preservation, main shader restart boundary | Existing suite passed with mock shader API only |
| New conf.lua restart boundary | Confirmed defect, entry-probe.log |
| GUI overlay, VS Code task lifecycle, real GPU validation | Not exercised / LOVE abort |

## WATCH-01 — dev keeps its original output root after configuration reload

**Medium, confirmed defect, new relative to searched roadmap/prior audit/release notes.**

Reproduce with config-probe.py: start `tua dev --no-launch` with `source="src", out="build"`, wait for ready, change only `out="new-build"`, wait for rebuild. Expected: the current build is hot-reloadable and session consumers are informed of the new output, or output relocation is explicitly rejected/requires restarting dev. Actual: CLI reports `configuration rebuild: rebuilt 1 file(s)` and `dev: sent reload 2 to the game`, but writes that signal to the old directory. New main.lua is `print(1)` with no hook; new runtime and signal are absent. Existing game remains rooted in obsolete output. Evidence: config-probe.py, config-dev.log, config-result.log.

Root: crates/tua-cli/src/dev/mod.rs:31-32 captures out_dir/output_guard once; hook at :50-58 continues using them after crates/tua-cli/src/watch/mod.rs swaps the layout/session during configuration reload. Docs cli.md:228-232 promise layout reload, and :246 says dev builds like watch.

Existing blind spot: CLI tests cover watch configuration reload (cli.rs:4198) and dev preparation separately (cli.rs:634); dev unit tests exercise output preparation without a changing layout. Smallest maintainable improvement: pass current build/output context through the shared watch hook, with an explicit dev relocation policy (restart/relaunch or actionable stop); avoid independently reloading configuration in the hook. Regression: repeat this clean harness and assert current directory hook/runtime/signal and launched/no-launch lifecycle behavior.

## WATCH-02 — conf.lua created after startup is permanently absent from restart tracking

**Medium, confirmed defect, new relative to searched roadmap/prior audit/release notes.**

Run entry-probe.lua. It installs the real hot-reload runtime with main.lua present and conf.lua absent, creates conf.lua and advances the signal, then edits conf.lua and advances again. Expected: entry configuration changes request restart, per docs/hot-reload.md's conf entry boundary. Actual restart counts are 0 after creation and 0 after subsequent edit; editing main.lua produces 1 as a positive control. Evidence: entry-probe.lua, entry-probe.log.

Root: vendor/runtime/tuahotreload.lua:353 assigns nil for absent conf.lua, removing its table key; :240 loops only existing entry_sources keys. Therefore the new configuration is never considered for the lifetime of this session. Existing vendor test covers main edits and main-associated shader edits, not initially absent conf creation. Smallest improvement: retain explicit absent entry snapshots (sentinel or a fixed entry-path list) and compare every entry on signal. Regression should cover conf creation, edit and deletion with both initially present and initially absent files.

## Limits / design questions (not defect totals)

- Runtime module deletion is silently skipped by `if latest` at tuahotreload.lua:252; clarify whether preserving captured deleted modules until restart is deliberate. CLI deletion itself was exercised and removes outputs. No separate defect claimed without a documented deletion contract.
- Module function removal/type changes, arbitrary private state, captured methods, partial cross-module reload ordering and dynamic shader filenames were not independently expanded. The documented private-state/captured-method/dynamic-file limits are not findings.
- No CLI actual game-close lifecycle probe, polling-backend failure injection, filesystem permission-race probe, or exhaustive dropped-event simulation.
- Shared source ownership/multi-root output defects belong to the projects/assets lanes and are not counted here.
- Trusted CI runs cargo workspace tests and Linux LuaJIT runtime-test (.github/workflows/ci.yml:108-111); absence of open-PR execution is intentional. No additional CI defect asserted.

Harness note: the first broad run left an intentionally invalid file in the former nested output before relocating out; the former output then correctly became source and blocked the build. That harness mistake was removed before the recorded successful rerun. The isolated config-probe.py avoids nested-output migration entirely and is the authoritative WATCH-01 reproduction.
