# Three game-development trials

Three agents independently built small playable LOVE games to exercise Tua as developers. These are simulated evaluations, not human usability studies or full ports. All use two Tua modules and no external assets. The trials preserve source, actual LSP responses, runnable probes and gameplay smoke scripts.

| Game | Development workflow | Outcome and evidence |
|---|---|---|
| [Rooftop Relay](platformer/README.md) | Platform physics, typed records, imported state and indexed arrays | Check/build and LSP assertions pass; LOVE LuaJIT gameplay smoke and 30 real graphics frames exercised. |
| [Arena survival](arena/README.md) | Entity arrays, imported functions, collision, score and timers | Check/build and LSP assertions pass; LOVE LuaJIT gameplay smoke with graphics/input stubs. Found formatter indentation defect, independently reproduced. |
| [Crate Courier](puzzle/README.md) | Grid indexing, mutable imported nested state, movement and victory | Check/build and LSP assertions pass; actual LOVE graphics/callback and gameplay smoke passes. |

The small typed-record/module workflow was dependable in these slices: useful member inference survived imports and incomplete edits, signatures selected the intended argument, and emitted Lua ran in LOVE. None of the games required `any` workarounds or new language features. That does not establish larger-project usability, native-class ergonomics, long-session latency or every LOVE API overload.

## Confirmed new issue

**ARENA-FMT-01 — inline closing branches leave subsequent code over-indented (Medium, confirmed; fixed; final integrated verification below).** An `elseif` or `else` line can close its owning conditional on that same line. The formatter incorrectly reopened indentation afterward, shifting the function's return/end and following top-level statements. Formatting was stable on a second pass and emitted runtime behavior was unchanged: this is an editor/readability defect, not demonstrated semantic corruption. See arena's original before/after fixtures and lane report. Another agent independently reproduced the defect.

The fix uses the existing AST layout traversal and source mapping to identify the owning conditional's end token. This avoids treating an inner anonymous function or nested conditional's `end` as the outer branch's end. Permanent tests check exact indentation and idempotence, including nested controls.

## Evidence reconciliation

A separate agent checked cursor anchors against the source and strengthened the arena/platformer probes from response recording to assertions. The platformer report was corrected from eight to seven Player fields. Returning a formatting edit is explicitly distinguished from applying and validating it. Arena draw dispatch uses stubs; puzzle and platformer additionally executed real graphics. No screenshot-based visual assessment or human playtest is claimed.

Initial trials used intermediate binary `f8a80336d0859172bc56ec201ef322c679d600794597d984651f47884970a270`. Final rebuilt-server reruns and their exact binary hash are recorded in [the integrated verification log](../VERIFICATION.md).

Initial output directories outside project roots were correctly rejected with TL2002; sandboxed LOVE launches exited 134 but authorized local launches passed. These were setup/environment issues, not counted as Tua defects. A project-selection CLI option and trial scaffolding convenience are usability ideas rather than broken documented behavior.
