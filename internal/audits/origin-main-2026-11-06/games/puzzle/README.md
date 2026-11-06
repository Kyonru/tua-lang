# Crate Courier: simulated Tua development trial

This is an agent-built development trial, not a human usability study or a bundled production example. It is a complete, deliberately small one-level Sokoban puzzle: push the orange crate onto the green target, restart, or explore the walls and blocked pushes. The two-push solution keeps runtime assertions transparent. There are no downloaded assets or dependencies beyond Tua and LOVE.

## Run and reproduce

From the repository root:

```sh
cd internal/audits/origin-main-2026-11-06/games/puzzle
../../../../../target/release/tua check
../../../../../target/release/tua build
/Applications/love.app/Contents/MacOS/love build
```

Arrows/WASD move; R resets; Escape exits. A delivered crate freezes movement until restart. Pushing into an unsolvable position is intentional Sokoban behavior; use R.

Automated checks, from the repository root:

```sh
node internal/audits/origin-main-2026-11-06/games/puzzle/probe.js
sh internal/audits/origin-main-2026-11-06/games/puzzle/run-smoke.sh
```

Set `LOVE_BIN` for another LOVE installation. `run-smoke.sh` uses a temporary runtime launcher under `target/game-puzzle/runtime`; compiled files stay in the trial's ignored `build/` because Tua intentionally prohibits output escaping the project root. Run build before smoke. The smoke opens a graphics window briefly and exits automatically; it uses the real LOVE graphics APIs, not mocks.

## Assessment

The ordinary typed-Lua path was straightforward: a `Point` record, typed string-array grid, annotated movement functions, mutable exported state, and a separate rendering/input module. No `any`, casts, special declaration file, or runtime type workaround was needed. All source compiled on the first language-check attempt. Module function signatures and nested record fields remained useful through imports and incomplete edits.

No confirmed Tua product defect was found in this trial. This is evidence for the specific workflows below, not universal editor or language correctness.

| Workflow | Observed result | Evidence |
| --- | --- | --- |
| Check/build two modules | Pass, 2 files each | `check.log`, `build.log` |
| Imported movement hover | `function board.move(dx: number, dy: number): boolean` | `lsp-evidence.json`, `imported function` |
| Nested imported state hover | `board.state.moves: number` | `imported nested state` |
| LOVE documentation hover | Rectangle signature plus “Draws a rectangle” and wiki link | `LOVE docs` |
| Signature help second argument | `activeParameter: 1`, correctly named numeric parameters | `move arguments` |
| Cross-file definition | Movement declaration in `src/board.tua`, line 19 | `definition` |
| Fresh diagnostics | Empty list on complete main document | `diagnostics` |
| Formatter request | Returned full-document edit successfully | `format` |
| Unsaved incomplete `board.player.` | Exactly `x`, `y` field candidates | `incomplete nested member` |
| Emitted Lua under LOVE 11.5 | Push, victory, freeze, reset, wall, blocked-push assertions passed | `smoke.lua`, `runtime.log` |
| Emitted callbacks and drawing | Real `love.load`, two key events, `love.draw`, reset invoked successfully | `smoke.lua` |

The protocol script asserts expected results and exits nonzero on mismatches. Request positions are derived from the actual source by the shared audit harness, and full request payloads/responses are retained. Representative one-session observations were 26.32 ms for the first imported hover, 0.61–1.38 ms for later hover/signature requests, and 4.44 ms for completion after an unsaved malformed edit. These are single-run protocol timings, not controlled benchmarks or installed-editor typing latency.

## Friction and limits

- The initial trial configuration placed output under the repository-wide `target/`; build rejected it with TL2002 (`output path escaped the project root`). This is a correct containment safeguard, not a language bug. Using ignored project-local `build/` solved it. A future trial scaffolder could make this convention explicit.
- The first runtime invocation inside the filesystem sandbox exited 134 without output. The same installed LOVE runtime outside that sandbox passed with exit 0. This was an environment limitation, not evidence of emitted Lua failure.
- A first shell check command used a wrong relative binary path from repository root (exit 127). It was corrected before product testing; this is a harness mistake, not a Tua defect.
- Formatter response was exercised, but interactive VS Code formatting and visual appearance were not independently inspected in this lane. No human playtesting, screenshot review, long-session profiling, Unicode controls, or hot reload was performed.
- Arrays use normal Lua indexing semantics. The game explicitly guards bounds before indexing; it does not test static bounds proof or out-of-range diagnostics.
- Classes, generics, protocols, callbacks with contextual types, and complex multi-level content were unnecessary for this slice and are outside its coverage. No feature request is inferred from their absence.

## Provenance

Checked on 2026-11-06 against working-tree audit fixes over HEAD `d7aed2e5f1297b80a948e055985375bceaff6f0c`. Release binary SHA-256 at the initial run: `f8a80336d0859172bc56ec201ef322c679d600794597d984651f47884970a270`. This is an intermediate audit binary; the lead is rebuilding and validating the final package separately. LOVE reported `11.5 (Mysterious Mysteries)`.

Source and evidence remain uncommitted. No compiler, extension, or runtime implementation files were modified by this trial.

## Final lead rerun

The final release server `1874c3b464b33e51c85f7be225b461b6071abffd82110df602f3475a25643f59` passed this game's asserted LSP probe and check/build/native smoke replay. Initial timings and intermediate-binary notes above are historical; [integrated verification](../../VERIFICATION.md) records final provenance and limits.
