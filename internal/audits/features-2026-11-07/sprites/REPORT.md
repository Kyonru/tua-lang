# Sprites and animation audit

Bounded audit of candidate 0.6.0 at baseline revision 9d490dc. Three confirmed defects: one high (accepted zero-duration lists can hang playback), two medium (loop method/count semantics). No product files changed. All three appear new relative to searched prior audit Markdown and ROADMAP.md; the roadmap's general sprite preview refinement does not identify them.

## Execution and coverage

Used tua-language-auditor, tua-audit-lead and tua-domain-design guidance. Public compiler: repository `target/release/tua`. Runtime: installed `/Applications/LOVE.app/Contents/MacOS/love` (version in love-version.log); direct bundled anim8 comparison with `/opt/homebrew/bin/lua` 5.4.8. LOVE initially aborted in the sandbox with no output; an approved escalated run succeeded. No desktop/editor automation or rebuilds were needed.

Reproduction from repository root:

1. `cd internal/audits/features-2026-11-07/sprites/project` and run the absolute repository `target/release/tua build` (build.log).
2. From repository root run `/Applications/LOVE.app/Contents/MacOS/love internal/audits/features-2026-11-07/sprites/project/build` (runtime-love.log). The preserved build/main.lua is the audit harness; emitted sheets.lua and bundled runtimes are preserved beside it.
3. `python3 internal/audits/features-2026-11-07/sprites/diagnostics.py` regenerates six isolated CLI cases (diagnostics.json). Existing build/main.lua harness files are retained by builds.
4. `node internal/audits/features-2026-11-07/sprites/lsp.js` performs real stdio LSP diagnostics/completion/hover (lsp.json).
5. `/opt/homebrew/bin/lua internal/audits/features-2026-11-07/sprites/anim8-oracle.lua` compares underlying anim8 behavior (anim8-oracle.log).

| Surface | Coverage and result |
| --- | --- |
| Frame selectors/timing | Executed inclusive range, single column, multi-row frame list, FPS and duration list; passed. |
| Player behavior | Executed independence, default initial animation, same-name play preserving place, clamped seekFrame, seek, stop/resume and negative speed; passed. |
| Markers | Long update crossing two loops emitted all five expected marker events; passed. Callback-driven animation switch was exercised but remaining-time semantics were not established. |
| Nested clips / batching | Actual LOVE child update and transformed draw; leaf SpriteBatch insertion and parent batch rejection; passed. No screenshot/pixel comparison. |
| Loop policies | Actual LOVE exposes SPRITES-02/03 below. pauseAtEnd control passed. |
| Static validation | Conflicting FPS/durations and out-of-range marker rejected. Missing timing rejected with broad "requires at least one animation" diagnostic. Zero duration list incorrectly accepted. |
| Editor | Actual LSP offers all six animation names, typed play hover and rejects player:play("absent"). Callback arity followup below. |
| Inspected only | Trim metadata/direct-batch agreement, runtime override fallback and sharing tests; no fresh custom runtime/metadata probe. |
| Not exercised | VS Code preview UI; disabled domain configuration; animated flip rendering; malformed metadata/range-duration matrix; performance under very large positive dt. |

## SPRITES-01 — Zero duration lists build successfully and hang update

**High · confirmed defect · new.** `zero_list/src/sheets.tua` declares two frames with `durations = { 0, 0 }`. Public build exits 0 (diagnostics.json). Run `/Applications/LOVE.app/Contents/MacOS/love internal/audits/features-2026-11-07/sprites/zero_list/build` from repository root. The controlled harness calls `S:player('idle'):update(0.1)` with a debug instruction hook, aborts after over one million instructions, removes the hook and exits. zero-duration-love.log reports `AUDIT_INSTRUCTION_BUDGET: update failed to terminate`, timer 0, frame 2. Without the guard the main thread cannot progress.

Expected: reject nonpositive duration entries, or at minimum fail boundedly at runtime. Actual: accepted list cycles forever. `crates/tua-core/src/sprite.rs:2486` validates only scalar text parsed as f64. `vendor/runtime/anim8.lua:139` accepts numeric list entries without positivity checking. `vendor/runtime/tuasprite.lua:140` consumes no remaining time when every duration is zero (line 151). This is a domain validator plus runtime robustness boundary, not an assertion that every malformed duration form was tested.

Smallest maintainable improvement: validate every statically known duration entry with the shared duration parser and require finite positive runtime entries in animation construction; don't paper over the loop with an arbitrary iteration cap. Add CLI list/range-duration rejection cases and bounded emitted-runtime regression for zero/negative entries. Existing sprite runtime fixture tests focus on draw, clones, metadata and zero-dt steady state, leaving positive-dt invalid-duration playback untested.

## SPRITES-02 — pauseAtStart stops at the end

**Medium · confirmed defect · new.** In project/src/sheets.tua, `stopStart` has four frames at 4 FPS and `onLoop = "pauseAtStart"`. Actual LOVE `S:player('stopStart'):update(2.2)` yields frame 4, timer 1, playing false (runtime-love.log). Expected frame 1, timer 0, paused; direct bundled anim8 gives precisely that (anim8-oracle.log). The docs accept animation-method names and editor schema suggests these methods.

`crates/tua-core/src/sprite.rs:2027` maps pauseAtStart, pauseAtEnd and pause to looping=false. `vendor/runtime/tuasprite.lua:162` then always chooses the final frame without invoking the configured callback. Preserve the configured loop action and invoke it at the boundary; retain explicit legacy nonloop policy separately if necessary. Add emitted LOVE/runtime tests for each supported string method, not only lowered text and looping flags. Existing parser tests assert the looping flag but don't exercise method semantics.

## SPRITES-03 — Loop callback count never exceeds one

**Medium · confirmed defect · new.** The documented callback `if loops > 1 then animation:pause() end` appears in docs/sprites.md and examples/features/src/sprites.tua. project/src/sheets.tua copies it into `loopFn`. A 2.2-second update of a one-second cycle invokes the callback twice with 1 and leaves playing=true (runtime-love.log). Direct bundled anim8 invokes once with 2 and pauses (anim8-oracle.log).

`vendor/runtime/tuasprite.lua:159` hardcodes 1 per boundary (reverse uses -1). Expected documented/anim8-compatible count for the update, including multiple wraps; actual count makes the documented condition unreachable. Define callback aggregation while preserving every crossed marker, then implement that centrally in the runtime. Regression should cover multi-wrap positive/negative updates, callback pause and marker delivery together. Existing runtime fixture does not exercise multi-loop callback counts; source/lowering tests preserve the callback but do not establish runtime equivalence.

## Cross-feature followups and design questions (not counted as sprite defects)

- **Return-call validation gap:** `unknown_name/src/sheets.tua` returns `S:player("absent")`; build accepts it and actual LOVE crashes with unknown sprite animation (unknown-name-love.log). `unknown_local/src/sheets.tua` assigns the same call to a local and receives TL1003. This is likely a shared return-expression checker traversal issue, not missing sprite name types (schema is in sprite.rs:512). Lead should deduplicate with language/checker work. No generalized root cause claimed.
- **Callback parameter policy:** LSP rejects `player:on("step", function() end)` with "callback requires 0 parameters, but it may be called with only 4" (lsp.json). Four supplied parameters are ordinarily harmless in Lua; this may share the broader callback assignability policy. Kept separate from confirmed sprite defects pending contract review.
- **Remaining time after marker-driven animation switches:** The harness switches run to single during a marker callback; update returns successfully but the new animation retains timer 0. Whether leftover dt should advance it needs an explicit policy and test. No defect claimed.
- No concrete missing capability is proposed from this bounded pass. Editor preview and configuration options remain coverage gaps, not evidence of absence.
