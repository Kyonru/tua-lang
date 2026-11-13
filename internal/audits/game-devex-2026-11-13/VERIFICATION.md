# Verification log and limits

## Environment and baseline

- Repository revision before the trials: `6bb460d0`; tracked worktree clean. The trial projects and reports were created under this ignored `internal/audits/` directory. No private game project was read or copied.
- CLI: `target/debug/tua --version` → `0.9.0`. Host tools: LÖVE 11.5 at `/Applications/love.app/Contents/MacOS/love`, Lua 5.4.8, Busted 2.3.0. `luajit` was not on `PATH`.
- `target/debug/tua check` from `examples/features`: passed, 45 files. `target/debug/tua build`: passed, 53 files.
- `node editors/vscode/scripts/lsp-features-smoke.js target/debug/tua examples/features`: passed. This is LSP protocol evidence, not visible suggestion timing.
- `target/debug/tua package --target love` from `examples/love-minimal`: passed, produced a 1.0 KiB archive; `unzip -t` passed. No desktop/web/custom package was exercised.
- In a temporary project under `/private/tmp/tua-game-devex-actions.d83jTL`, `tua init` and `tua actions init` passed; rerunning `actions init` preserved the workflow. The generated GitHub workflow was not executed remotely.

## Trial runs

The complete command logs, sources, and limits are in the [arena](trials/arena/REPORT.md), [alchemy](trials/alchemy/REPORT.md), and [maze](trials/maze/REPORT.md) reports. The lead did not equate a checker pass with runtime success. All three games were built and run under LÖVE with bounded smoke modes. Arena passed 5 Busted tests, alchemy 3, and maze 4, for 12 trial tests total. Arena and alchemy also replayed included-GLSL hot reload in LÖVE. Trial reports distinguish checked source from actual gameplay assertions and from unverified visuals or physical input.

After removing generated output, the lead reran `tua check`, `tua build`, and `tua test` for each game from its own trial directory. All nine commands exited 0; tests again reported 5/5, 3/3, and 4/4. The clean build file counts were 27, 25, and 26 respectively, versus the agents' smaller incremental build counts; the two observations used different cache states. Each check/build still emitted the documented conservative TL5007 pool warning. The lead's freshly built alchemy LÖVE smoke also exited 0 with recipe, signal, store, ECS, pool, and shader assertions. A fresh arena LÖVE replay exceeded its expected bound twice with no output, although the agent's earlier 90-frame run exited 0. A subsequent bounded plain-Lua three-frame LÖVE control also timed out before output, making this a current macOS graphical-host qualification blocker rather than evidence of an arena/Tua regression. See the [arena recheck](trials/arena/REPORT.md).

## Independent checks of high-impact findings

| Finding | Lead recheck | Observed result |
| --- | --- | --- |
| GAME-01 | Copied `trials/maze/repros/inheritance_alias/` into `/private/tmp/tua-game-devex-alias-recheck/`; ran `tua check` on original, no-bound, no-generic, and unused-import controls. Also checked the arena's independent `imported-base` form. | Original and second form report TL1003 on subclass instance alias; removing the protocol-bound generic or protocol-typed array makes the positive upcast pass. `tua build --emit-on-error` output runs successfully under Lua 5.4. |
| GAME-02 | Ran `tua check` and `python3 run_love_smoke.py repros/shader_source_path` / `repros/shader_build_path` from the alchemy trial. | Both check; source-root literal fails at LÖVE load, built-root literal loads. Maze's shader trial independently hit the same source/build mismatch. |
| GAME-03 | Copied the three-file `factory_forward` project into `/private/tmp/tua-game-devex-forward-recheck/`; checked original, explicit instance annotation, and direct imported constructor variants; ran emitted Lua and `lsp_probe.py`. | Inferred factory result and direct imported constructor both report `expected Guard, found Guard`; explicit imported instance type passes. Lua runtime passes; LSP completion/hover/definition resolve the method. |
| GAME-04 | Ran a four-variant matrix from `trials/arena/repros/event-signature/`: valid imported call, wrong imported local payload, wrong direct module-property payload, wrong same-file payload. | Wrong imported forms pass checking; wrong same-file call fails TL1003 with the `damage:number` expectation. |
| GAME-08 | Ran `tua check`, `tua build`, and `lua -e 'local result=dofile("main.lua"); print("inline",result.inline,"multiline",result.multiline)'` from the arena's `init-order` fixture (`lua` from `build/`). Inspected emitted constructors. | Check/build pass. Runtime prints `inline 0 multiline 120`; inline fast constructor skips the intended assignment. |

The lead also reran the alchemy LSP complete/incomplete/edit probes, the formatter fixture, and a bounded copy of the maze LÖVE smoke. The isolated maze copy exited normally with `TUA_MAZE_SMOKE_OK`. One earlier maze GUI launch hung, but subsequent bounded replays passed; cause unestablished, so it is not a confirmed product finding.

For GAME-12, the lead reran `python3 lsp_probe.py` in `trials/arena/`. Completion after `Game.selected.value.` returned `update`, `speed`, `progress`, and `elapsed`, while hover on `.value` returned null at two positions. Hover on `Game.selected` and on the inferred local both returned types. No visible editor-widget timing was measured.

An agent who did not write the consolidated report independently reviewed the finding counts, 28-row matrix, reproduction preconditions, Markdown links, and verification claims. That review caught the missing fresh-build step for GAME-02, which was added; its final pass found no further discrepancies.

## Verification boundaries

- These are small, newly created games and automated trials. There was no human playtest or subjective visual/audio assessment.
- LSP probes use real stdio requests, but the trial games were not opened in an installed VS Code host. The protocol timings cannot be interpreted as trigger-to-menu or format-on-save delay.
- GUI LÖVE runs confirm boot, updates, draw, shaders, and selected gameplay assertions; they do not prove frame timing, audible quality, physical key handling, all platforms, or long sessions.
- The audit did not run web/custom packaging pipelines, GitHub Actions, debugger/source-map stepping, the runtime inspector, large preview graphs, native asset previews, or a previous Tua version. Their absence is a coverage limit, not a passing result.
- No product code changed. Full `make verify` was not rerun for these audit fixtures; a prior preview audit at the same revision ran it, but that does not validate new game behavior. Trial-specific check/build/test/LÖVE and minimal reproductions are the relevant evidence here.
