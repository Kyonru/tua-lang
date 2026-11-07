# Audio feature audit — 2026-11-07

Scoped pass on the 0.6.0 release binary. Five confirmed runtime defects: four Medium and one Low after independent evidence review. No product edits. Lead independently reran the runtime probe; see `../cross-checks/audio-runtime.txt`. Audit-lead, language-auditor, and domain-design skills applied. No matching audio items found in ROADMAP.md or prior audit reports searched; all five are recorded as new, subject to lead reconciliation.

## Execution and coverage

- `target/release/tua build` in the isolated `audio/project` copy of examples/audio-system: passed, built 9 files; expected low-confidence TL2012 warning for sample2.wav. This exercises built-in analysis, catalog generation, typed main, lowering, transitive helper emission and asset copying. See build.log and project/src/generated/audio/.
- `/opt/homebrew/bin/lua crates/tua-cli/tests/fixtures/audio_runtime.lua "$PWD"`: passed, exit 0 (baseline-runtime.log is empty). Existing happy path covers priority stealing/rejection, stale handles, fades, inherited gains, pooling, snapshots and subscriptions.
- `/opt/homebrew/bin/lua internal/audits/features-2026-11-07/audio/probe.lua "$PWD"`: runtime.log.
- Same command with second argument `$PWD/internal/audits/features-2026-11-07/audio/project/build`: emitted-runtime.log, identical results against emitted runtime helpers.
- `node internal/audits/features-2026-11-07/audio/lsp.js`: actual stdio LSP; lsp.json captures cue, bus and event argument completion.
- `target/release/tua check` in audio/negative: exit 1 with correct unknown cue TL1003; negative maxVoices gets no diagnostic (negative.log).

| Surface | Status | Scope |
|---|---|---|
| Buses/cues/voice handles/fades/events | Exercised | Existing happy path plus limits and snapshot probes |
| Scene timing, quantization, looping | Exercised | Deterministic Source doubles under Lua 5.4.8 |
| Built-in audio analysis | Exercised | Two real WAV inputs; complete example build |
| Cue/bus/event completion | Exercised | Actual release stdio LSP; problematic output preserved |
| Runtime dynamic analysis/cache | Inspected only | Worker/cache code; no actual LOVE worker execution |
| Audible playback, DSP, spatial sources, device latency | Not exercised | LOVE installed, but this bounded pass uses doubles; no hardware claim |
| Audio Explorer UI | Not exercised | No desktop automation requested from lead |

Source doubles implement Source clock/seek/play/pause/clone and gain setters. Clock values are explicitly controlled to reproduce valid Source wrap and long-frame behavior. Runtime findings reproduce using both vendor and emitted helper; they do not assert actual audible quality.

## Confirmed findings

### AUDIO-01 — Medium: Overlapping global and cue limits are not enforced together

_limitVictims unions full groups and steals one global minimum. With global=2 and high cue limit=1, low(priority 1)+high(priority 10), playing high steals low and leaves two high voices. This violates the cue cap, while still respecting the global bound. Independent review rates the observed failure Medium: no hang, crash, or unbounded resource exhaustion was demonstrated.

Contract: docs/audio.md:88–94 promises global, bus and cue limits, stealing from the constrained group. Simultaneous global/cue caps are violated. Applying parent bus limits to descendant voices is also the natural hierarchy expectation; make that policy explicit. Reproduce with probe.lua using the commands above. Actual: `overlap limits: stolen, high count 2; ancestor limit: started, active 2`. Expected: enforce every applicable cap, or reject the incoming voice when no permitted victim can satisfy them. Source: `vendor/runtime/tuaaudio.lua:260`.

Smallest improvement: Respect every applicable constrained group; choose a victim that satisfies all constraints or reject. Decide and document parent-bus aggregation separately. Regression/test blind spot: Existing fixture tests global limits only. Add overlapping global/cue and parent/child bus cases.

### AUDIO-02 — Medium: Bar transitions miss boundaries crossed by a long frame

At 60 BPM, schedule bar transition from scene one after update at position 0, then move leader to 5.1 seconds. Bar 1 is emitted but the transition remains pending in scene one.

Contract: docs/audio.md:111–125 describes bar-quantized transitions and catching up crossed beat/bar boundaries after long frames. Reproduce with probe.lua using the commands above. Actual: `crossed bar: one, beats 6, bars 2`. Expected: start the pending transition when the update crosses the next bar boundary. Source: `vendor/runtime/tuaaudio.lua:508`.

Smallest improvement: Compare the previous and current bar indices or process scheduled transitions while traversing crossed boundaries. Regression/test blind spot: Existing audio fixture contains no music scenes. Test a frame crossing beat 4 and ending at beat 5.

### AUDIO-03 — Medium: Looping music stops emitting beats after the first playback cycle

Default music layers loop. Advance leader to 3.9, wrap to 0.1, then advance to 1.1. Beat count stays 4 because runtime.beat retains the previous cycle maximum.

Contract: docs/audio.md:118–125 uses the leading source clock for ongoing rhythm events; looping layers must not permanently stop that event stream after the first cycle. Reproduce with probe.lua using the commands above. Actual: `loop beats: 4, 4`. Expected: continue beat events after subsequent clock wraps. Source: `vendor/runtime/tuaaudio.lua:501`.

Smallest improvement: Detect Source clock wrap and reset the per-cycle event cursor or maintain a monotonic transport clock with explicit event semantics. Regression/test blind spot: No looping music runtime coverage. Test multiple wraps and a quantized transition scheduled after a wrap.

### AUDIO-04 — Medium: Loop regions are ignored when a scene has no BPM

Create an immediate scene with loopRegion={start=1,finish=2}, no BPM or analysis, and a source at 2.1. update leaves its position at 2.1 instead of seeking to 1.

Contract: docs/audio.md:126 offers sample-position loop regions independently of authored BPM. Reproduce with probe.lua using the commands above. Actual: `untimed loop region: 2.1`. Expected: seek to the region start after crossing its finish, without requiring BPM. Source: `vendor/runtime/tuaaudio.lua:482`.

Smallest improvement: Run source clock, loop-region, and drift synchronization independently of optional rhythm event timing. Regression/test blind spot: Existing fixture does not cover music. Add untimed region-loop and untimed layer-drift tests.

### AUDIO-05 — Low: snapshot analysis state is not detached

Assign snapshot.analysis.fake="corrupted", then take another snapshot. The new snapshot contains fake="corrupted" because analysis returns the internal status table directly.

Contract: docs/audio.md:102 and :164 promises detached snapshot tables, including analysis status. Reproduce with probe.lua using the commands above. Actual: `detached analysis: corrupted`. Expected: editing a snapshot must leave the next snapshot and internal state unchanged. Source: `vendor/runtime/tuaaudio.lua:598`.

Smallest improvement: Copy the analysis status table when constructing the snapshot. Regression/test blind spot: Existing snapshot assertions inspect counts only. Mutate every nested returned snapshot table and verify system state is unchanged.

## Shared symptoms and design questions (not added to audio defect totals)

- **Parent-bus cap policy:** the runtime starts two child-bus voices under a parent configured with maxVoices=1. The docs explicitly inherit gains but do not explicitly define cap aggregation. This observation is retained as a design question, not another confirmed defect.

- **Argument completion mismatch:** lsp.json contains no `ping` for a:play and no configured `master`/`sfx` for a:stopBus; it returns event names, `smoothstep`, and one malformed function-type fragment. `on` has the same output. The checker does correctly reject `wrong` as a cue in negative.log. Shared enum extraction at crates/tua-core/src/completion/mod.rs:2368 splits formatted types on `|`; audio typed member specialization is at crates/tua-core/src/audio.rs:127. Cause remains a cross-feature investigation, passed to lead; add real LSP configured-name argument tests (current audio test primarily exercises member completion).
- **Negative numeric validation:** `maxVoices=-1` is accepted while `play("wrong")` is rejected. audio.rs:640 only extracts Number expressions, and audio.rs:748 checks only extracted values. Shared with input audit; do not count twice. Preserve negative/src/main.tua and negative.log.
- **Gain wording conflict:** docs says voice/cue/bus gains multiply but also says play options override cue settings. Probe gives source volume 0.5 for cue volume 0.5 and play volume 0.5; multiplication would give 0.25. Runtime line 371 implements replacement. Decide intended public contract before changing code or docs; this is a design/documentation ambiguity, not included as a confirmed runtime defect.
- **Runtime analysis options/cache risk:** requestAnalysis cache key at tuaaudio.lua:618 includes bytes and fixed version but omits envelopeHz/tempoMin/tempoMax; changing options for an already cached asset appears to reuse old results. Inspected only, not counted confirmed. A LOVE worker test should vary options between requests and inspect envelope length/BPM bounds.
- **Missing options/design:** built-in analyzer offers tempo/envelope/confidence controls and documented fixed decode ceilings. Timing-only output or lazy per-track catalogs remain explicit documented future optimizations; avoid treating these as defects. Consider clear policy for event numbering across natural loops, interrupted crossfades, and music layers rejected by voice limits. Runtime worker cancellation/cache lifecycle deserves a separate focused real-LOVE pass.
