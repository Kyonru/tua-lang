# Independent game evidence review

Reviewed 2026-11-06 by the puzzle-trial agent at the audit lead's request. This is independent review of the other two lanes and a self-check of the puzzle lane; it is not independent validation of every puzzle claim.

## Cursor and output checks

All probe positions were reconstructed against the exact main source text and the shared harness's last-match convention. All documents are ASCII at the tested positions; JavaScript UTF-16 indexing is appropriate.

| Trial | Cursor checks | Assertion outcome |
| --- | --- | --- |
| Platformer | Imported hover points at the beginning of `x`; definition inside `player`; LOVE hover inside `rectangle`; signature after the second comma selects parameter 2; unsaved completion directly after `world.player.` | Added explicit output assertions; script exited 0 |
| Arena | Hover inside `speed`, `radius`, and `moveToward`; signature before `player` in second argument selects parameter 1; completion at beginning of existing member token after dot; definition inside method name | Added explicit output assertions; script exited 0 |
| Puzzle | Hover inside imported `move`, `moves`, and `rectangle`; signature after first comma selects parameter 1; definition inside `move`; unsaved completion after `board.player.` | Existing assertions reviewed; earlier script exited 0 |

Arena and platformer scripts originally only recorded responses. They now assert meaningful expected types, candidate sets, navigation targets, signature selection, and empty diagnostics. Formatter assertions establish only a nonempty response: they do **not** prove that applying the formatter edit preserves valid source. Arena's separate formatter investigation addresses that distinction.

## Evidence corrections

- Platformer report said eight Player members. Both its declared type and response contain seven: `grounded`, `h`, `vx`, `vy`, `w`, `x`, `y`. Corrected the report and asserted the exact set.
- Platformer report called its formatter edit “valid” without applying it. Wording now says it returned an edit and explicitly notes that this probe does not apply/recheck it.
- Platformer `run.sh smoke` runs host Lua. The owner subsequently added [love-smoke.sh](platformer/love-smoke.sh) and [love-smoke.log](platformer/love-smoke.log), preserving separate host-Lua, headless LOVE/LuaJIT and actual graphics-loop runs. The graphical wrapper counts 30 update callbacks and lets the normal draw loop run; it does not instrument an exact draw-call count. Do not infer LOVE qualification from the host-Lua command alone.
- Arena `runtime-smoke.lua` explicitly replaces graphics, keyboard, and window tables with deterministic stubs. It tests game logic under real LOVE/LuaJIT and drawing **dispatch**, not real drawing. Any independent graphical smoke must be separately identified.
- Puzzle `smoke.lua` calls emitted callbacks using actual LOVE window/graphics implementations. It invokes drawing directly before quitting; it establishes API execution, not a screenshot review or human visual assessment.

These replays used intermediate release binary `f8a80336d0859172bc56ec201ef322c679d600794597d984651f47884970a270` (see per-trial provenance for authoritative full hash). Final-binary replay belongs to lead verification. No product implementation files changed during this review.

## Final documentation cross-check

The consolidated severity arithmetic is consistent: prior 18 families (8 High, 10 Medium), three additional Medium runtime/testing issues, and one Medium formatter issue total 22 (8 High, 14 Medium). The constructor-chain symptom is consolidated into existing causes, not counted as another family. At this review point `VERIFICATION.md` had not yet been written; final-gate and rebuilt-binary claims remain lead-owned pending evidence. Initial trial evidence is explicitly intermediate, and must not be represented as the final package hash.
