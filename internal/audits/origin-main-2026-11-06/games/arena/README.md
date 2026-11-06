# Pulse Arena: simulated Tua development trial

This is an agent-built vertical slice, not a human usability study. The game has player movement, normalized diagonal movement, increasingly frequent pursuing enemies, circle collision, a two-second pulse cooldown, score, a 45-second win condition, loss and restart. It needs no external assets. Source is split between an entity module exporting `Body` and a LOVE game consuming that type and runtime functions.

## Run

From the repository root:

```sh
sh internal/audits/origin-main-2026-11-06/games/arena/run.sh
# Deterministic actual LuaJIT test, with input/render functions stubbed:
sh internal/audits/origin-main-2026-11-06/games/arena/run.sh smoke
node internal/audits/origin-main-2026-11-06/games/arena/probe.js
```

`LOVE_BIN` can override the installed macOS LOVE path. The script copies the source project to ignored `target/game-arena`, then checks and builds there. WASD/arrows move, Space pulses nearby enemies, R restarts, Escape exits. Survive 45 seconds.

## Evidence

- `tua check`: checked two source files, no diagnostics. `tua build`: built two files with source maps.
- Actual LOVE 11.5 / Lua 5.1 / LuaJIT 2.1.1700008891 smoke passed movement, pulse/score, collision/loss, restart, isolated survival/win timer and draw dispatch. `runtime-smoke.lua` is the exact runner. It deliberately stubs keyboard, window and graphics; this proves emitted game logic executes in LOVE's LuaJIT, **not** graphical appearance or human playability/balance. Sandbox LOVE startup aborted with exit 134; the same command with approved local execution exited 0.
- `lsp-evidence.json` preserves actual stdio request positions, results and timings. Imported `player.speed` and indexed `enemy.radius` hover as number; player member completion supplies x/y/speed/radius; module completion supplies create/moveToward/touches. Imported function signature includes body/target/dt, selects parameter 1 at the second argument, and definition navigates to entities.tua. Diagnostics are empty. Formatting returned an edit in 1.44ms in the initial run.
- Initial single-session protocol timings: cold field hover 38.06ms; subsequent hover 5.85ms/1.35ms; signature 0.95ms; completion 0.51ms/0.20ms; definition 0.15ms. These are request durations on this small project, not whole keystroke latency or a benchmark distribution. Later probe runs overwrite the JSON with current evidence.

## Confirmed finding ARENA-FMT-01

**Medium; confirmed and independently reproduced.** A mixed inline/multiline conditional ending on an `elseif` or `else` line makes following source visibly overindented. `format-before.tua` and `format-after.tua` preserve minimal input and original faulty output. The file remains valid and produces `2`; this is formatting correctness, not a Lua semantic change. A second `fmt --check` accepted the wrong indentation, demonstrating why idempotence alone was insufficient.

Reproduce by copying `format-before.tua` to `src/main.tua` in any project using this trial's tua.toml, then running `tua fmt`; compare to `format-after.tua`. Before the fix, formatter `format_line` always re-added one indentation level for middle branches, even when that branch also ended its owning conditional. The fix uses the existing parsed/source-mapped formatter layout metadata to identify that conditional's end token; nested inline function or conditional ends must not be mistaken for the outer end. Permanent formatter regressions assert exact indentation plus idempotence for else/elseif and nested controls. See the parent verification report for final fix checks.

## Ease of use and remaining opportunities

Typed records and type-only imports were straightforward and did not need casts or `any` workarounds. Cross-file editor assistance was useful on the first protocol pass. Ordinary LOVE calls and table operations checked without false errors. Type import plus runtime require remains two declarations; this is supported syntax and a workflow consideration, not a defect.

A no-return imported function displays `: any` in hover and signature. This loses information compared with a clear no-return indication, but this trial did not establish that Tua promises inferred void or show an unsafe assignment accepted; record as a usability/inference capability to assess, not an additional confirmed defect.

The formatter issue is the main new concrete problem encountered. A real editor window, interactive play session, screenshot, long-session memory profile and adversarial large-project editing were not exercised by this agent. The runner tests draw dispatch only, not visual output. Source-map files were generated; traceback mapping was not independently triggered here.

Fixture mistakes excluded from findings: the first project config put output outside its root and was correctly rejected with TL2002; the runner now copies the project to target. An initial probe script had an extra closing brace, corrected before requests were sent. Neither was a Tua failure.

## Final lead rerun

The final release server `1874c3b464b33e51c85f7be225b461b6071abffd82110df602f3475a25643f59` passed this game's asserted LSP probe and check/build/native smoke replay. Initial timings and intermediate-binary notes above are historical; [integrated verification](../../VERIFICATION.md) records final provenance and limits.
