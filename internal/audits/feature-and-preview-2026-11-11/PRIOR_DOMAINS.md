# Recheck of November 7 game-domain findings

Rechecked at `69e03d1a` on November 11. This is a targeted current-source recheck, not a claim that all game-domain workflows or the graphical editor were retested. I built `target/debug/tua` with `cargo build -q -p tua-cli` and copied old `tua.toml`/`src` fixtures into fresh temporary projects before running `check`, `build`, and Lua 5.4.8. No historical fixture or product source was edited. The concurrent [domain audit](DOMAINS.md) separately rechecked ECS-01/02/03, MODULES-02, SIGNALS-01, POOLS-02, and ASSETS-01; those IDs are not double-counted here.

## Exact finding dispositions

| Historical ID | Current disposition | Recheck evidence |
| --- | --- | --- |
| AUDIO-01 | Fixed and rechecked | `lua internal/audits/features-2026-11-07/audio/probe.lua "$PWD"`: overlapping global/cue cap returned `stolen`, high active count 1; ancestor cap output `active 2` is consistent with its two-bus config and not a separate demonstrated failure. |
| AUDIO-02 | Fixed and rechecked | Same probe: long frame crossed the bar and switched to `two`; output `crossed bar two beats 6 bars 2`. |
| AUDIO-03 | Fixed and rechecked | Same probe: `loop beats 4 6`, so beats continue after wrap. |
| AUDIO-04 | Fixed and rechecked | Same probe: untimed loop region sought back to `1`. |
| AUDIO-05 | Fixed and rechecked | Same probe: mutating snapshot analysis did not mutate live state (`detached analysis nil`). |
| ASSETS-02 | Fixed and rechecked | Fresh temp pipeline with a Python importer: first/second `check` used cache (`runs=1`); after replacing the sole cached `files/data.txt` with `CORRUPT`, next `check` exited 0, reran importer (`runs=2`), and materialized `ORIGINAL`. |
| CAMERA-01 | Fixed and rechecked | Fresh copy of `camera/alias`: `check` and `build` exit 0, Lua prints `0`, and build contains `tuacamera.lua`. |
| CAMERA-02 | Fixed and rechecked | Current `vendor/runtime/tuacamera.lua` with historical `camera/runtime.lua`: 45-degree bounded view reports bounds beginning at approximately `(0,0)`, with centered position `(494.9747,494.9747)`. |
| CAMERA-03 | Fixed and rechecked | Same probe: rendered pixel center `(201,151)` agrees with `toScreen(0,0)=(201,151)`. |
| CAMERA-04 | Fixed and rechecked | Same probe: shaken viewport upper-left `(-408.1951,-294.8351)` equals reported visible-bounds upper-left. |
| CAMERA-05 | Fixed and rechecked | Same probe: preset deadzone remains `100x200`; `docs/camera.md:80` and runtime annotation now accurately say narrow horizontally and tall vertically. This is a documentation resolution, not a change in camera feel. |
| INPUT-01 | Fixed and rechecked | Fresh copies of `input/negative_deadzone` and `negative_hex`: `check` exits 1 with TL5008, `deadzone must be between 0 and 1`. |
| INPUT-02 | Fixed and rechecked | `input/empty_unknown` and `known_unknown`: `check` exits 1 with TL1003 at `name` instead of accepting the missing control. |
| POOLS-01 | Fixed and rechecked | `pools/bad_reset` and `bad_dispose`: `check` exits 1 with TL1003 naming reset/dispose parameter 1 and expected object type. |
| POOLS-03 | Fixed and rechecked | `pools/zero_hook`: `check` and `build` exit 0; valid zero-argument callback accepted. |
| TWEEN-01 | Fixed and rechecked | `lua internal/audits/features-2026-11-07/tween/runtime.lua`: all three cancellation probes return `true` with no exception; cancelled on-start tween value remains 0. |
| TWEEN-02 | Fixed and rechecked | Fresh `tween/after_invalid`: `check` exits 1, TL1003 at `seconds`, expected number, found string. |
| VECTORS-01 | Fixed and rechecked | Fresh `vectors/chained_constructor` build/run exits 0, prints `0.6 0.8`. |
| VECTORS-02 | Fixed and rechecked | Fresh `vectors/comment_math` and `bracket_math` build/run exit 0 and each prints `10`. |
| VECTORS-03 | Fixed and rechecked | Fresh `vectors/underflow_div` build/run exits 0, prints `1.0 1.0`. |
| VECTORS-04 | Fixed and rechecked | Fresh `vectors/normalization` build/run exits 0; both huge and tiny nonzero vectors normalize to `(1,0)`, length 1. |
| SM-01 | Fixed and rechecked | Fresh `state_machines/reserved`: `check` exits 1, TL1003: event `current` conflicts with a built-in member. |
| SM-02 | Fixed and rechecked | Fresh `state_machines/dynamic`: `check`/`build`/Lua exit 0; prints `idle` then `true`, without fictional `none` diagnostic. LSP graph/hover was not rerun. |
| SM-03 | Fixed and rechecked | `lua internal/audits/features-2026-11-07/state_machines/runtime.lua "$PWD" internal/audits/features-2026-11-07/state_machines/special.dot`: cancelled transition prints `leave callback count 0`. |
| SM-04 | Fixed and rechecked | Same probe: after-callback chain prints `finishWaitingOnLeave finish`; resume reaches `done`. |
| SIGNALS-02 | Fixed and rechecked | Fresh `signals/invalid_handlers`: `check` exits 1 with three TL1003 errors for number, boolean, and string handlers. |
| SHADERS-01 | Fixed and rechecked | Fresh `shaders/fixtures/macro`: `check` exits 0 (`checked 1 file(s)`); original macro uniform false positive absent. |
| SPRITES-01 | Fixed and rechecked | Fresh `sprites/zero_list`: `check` exits 1, TL5004: animation durations must be greater than zero. The current runtime was not separately stress-tested for malformed dynamic duration lists. |
| SPRITES-02 | Fixed and rechecked | `lua -e 'package.path="vendor/runtime/?.lua;"..package.path; dofile("vendor/tests/sprite_test.lua")'` exits 0. It checks `pauseAtStart` position 1 and timer 0 after 2.2 seconds, alongside pause-at-end behavior. LOVE rendering was not exercised. |
| SPRITES-03 | Fixed and rechecked | Same runtime suite exits 0. Multi-wrap callback receives aggregate count `2` forward and `-3` reverse, then pauses; crossed markers are asserted. |
| STORES-03 | Fixed and rechecked | `lua internal/audits/features-2026-11-07/stores/runtime-probes.lua "$PWD"` prints `REENTRANT_SET returned_next=true returned_current=false`. The old script then exits 1 because it asserts the former buggy result (`result==second`). Current `vendor/runtime/tuastore.lua:110` returns `nextState` as documented. |
| WATCH-01 | Fixed and rechecked | Fresh temp `tua dev --no-launch` with polling, config changed `out="build"` to `out="new-build"`: new `main.lua`, `.tuahotreload`, and `tuahotreload.lua` all exist; CLI logs `dev: ready .../new-build`. |
| WATCH-02 | Fixed and rechecked | `lua internal/audits/features-2026-11-07/watch_reload/entry-probe.lua`: prints restart counts 1 after conf creation, 2 after edit, 3 after main edit. Its terminal assertion `restarts==1` fails because it encoded the old defect; the printed counts show the desired restart behavior. Current `vendor/tests/hotreload_test.lua:162-167` also covers conf creation. |

The following old IDs were **inspected only, not individually rechecked** in this lane: EXPLORERS-01/02, SHADERS-02, SIGNALS-03. Their historical reports remain evidence of the old behavior, not proof of current status. In particular, no installed VS Code host was used here to retest explorer freshness, signal navigation, or shader Markdown rendering; no actual LÖVE render was used to retest sprite loop behavior. These require a separate current-state result before calling them fixed. No current actionable defect was established by the probes above.

## Reproduction recipe

From the repository root, build the current CLI once with `cargo build -q -p tua-cli`. For checker and emitted-Lua cases, copy the named historical fixture's `tua.toml` and `src/` into a new temporary directory, run absolute `target/debug/tua check`, and, for accepted programs, `target/debug/tua build` followed by `lua main.lua` from the generated `build/` directory. Do not run the historical `probe.py` scripts in place: some write into their own fixture directories or call the old release binary. Lua runtime probes above read current `vendor/runtime/` sources; they do not verify visual output in LÖVE. See the historical [reports](../features-2026-11-07/) for each minimal source and original expected/actual result.
