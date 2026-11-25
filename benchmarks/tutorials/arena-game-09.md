# Arena game: whole-path qualification

C9 audits the eight playable chapters as one learning path. The startup-title fix is
`ba917ef5`; the reusable qualification command is `f17e8537`. Source remained typed;
no generated gameplay Lua was edited and no compiler/runtime changes were needed.

## Repeat the clean-copy check

From the repository root:

```sh
python3 scripts/qualify-arena-tutorials.py \
  --tua target/release/tua \
  --love /Applications/love.app/Contents/MacOS/love \
  --output /tmp/tua-arena-c9-clean
```

Choose a new output directory. The script copies each checkpoint's tracked files into
a fresh temporary checkout-shaped directory without a build folder. It exposes the
selected compiler as `tua` on PATH, runs the guides' `tua check --deny-warnings` and
`tua build`, checks formatting for typed and explicitly listed authored Lua, then runs
its emitted-Lua suites. Native scenarios build/launch the clean copies through the
existing replay runner, using isolated save identities and fixed input. The originals
are not rebuilt or altered by qualification.

Omit `--love` for headless-only checks; the output explicitly records native work as
not requested. A missing native run is not a complete-path pass. Detailed logs,
frames and per-scenario provenance remain in the selected output directory.

## Results

All eight checkpoints passed. The six suite names accumulate by chapter:
movement, session, combat, decisions, soundscape and conversation.

| Chapter | Headless suite runs | Native replay scenarios | Separate save qualification |
| --- | ---: | --- | --- |
| C1 | 2 | 125-frame playable loop | — |
| C2 | 3 | 125-frame combat | — |
| C3 | 4 | 350-frame enemy decisions | — |
| C4 | 5 | 350-frame gameplay/audio | — |
| C5 | 5 | Gameplay/audio; 50-frame shader recovery | — |
| C6 | 6 | Gameplay/audio; shaders; 60-frame dialogue | — |
| C7 | 6 | Gameplay/audio; shaders; dialogue; 40-frame saves | Two independent processes |
| C8 | 6 | Gameplay/audio; shaders; dialogue; saves; 45-frame workload | Two independent processes |

Total: **37 headless suite runs, 18 native scenarios, and four persistence-process
runs**. Native scenarios use seed 71 at 30 fixed updates per second. The save processes
validate real filesystem lifecycle and recovery, not just serialization mocks.
The C8 differential/performance/inspector qualification remains in its own report;
this audit did not rerun timings or infer a new speed result.

`arena-game-09-results.json` records all chapter hashes and native provenance. Compiler:
development release 0.11.0, embedded revision
`e0033fc65d3b78f9be8aba3e427aa9a239782354`, aarch64-apple-darwin, SHA-256
`3e49f76e0ec726d9b0fb6cb24c8497b242c92ce170ba6320a31f6a16b85e48ec`.
Headless Lua 5.4.8; native LÖVE 11.5.0 / LuaJIT 2.1.1700008891 with JIT disabled by
the runners. The recorded pre-commit source identity is separate from compiler identity.
The copied files include the corrected C8 startup title, verified against the recorded
chapter hash. README navigation edits followed the run; they change no runtime source.

Representative captures were inspected: C1 pause, C6 French choices at 640×480, and
C8's bounded larger wave. Existing chapter images remain the public illustrations;
full repeated capture sequences are not committed. Native audio checks verify Source
behavior and clocks; captures have no audio recording.

## Guide, snippet and asset audit

The eight chapter pages contain 23 Tua snippets. They were compared with their owning
checkpoint sources: most are direct excerpts, while differences are comments,
formatting/trailing commas, or explicitly partial examples. The audio declaration
shows two of four cues, and the music transition illustrates the active branch of
its host condition. These are not advertised as complete replacement modules.
GLSL, dialogue and catalog-configuration examples were checked against their source
files too. Complete owning projects compile; context-dependent snippets were not
claimed to compile as independent programs.

The rendered-site check validates local links and anchors in the arena pages and
navigation, local README targets, and GitHub main-branch repository links against
this checkout. Its counts and exclusions are recorded in `arena-game-09-links.json`.
Third-party URLs were not fetched, and resolving a repository target locally does
not claim that an unpushed commit is already available on GitHub.

Bundled source asset SHA-256 values were checked against provenance, with generated
build folders excluded. The original Kenney CC0 license and audio CC0 notices remain.
No new asset download, image generation, grid background or UI framework was added.

## Corrections and learning-path decisions

- Corrected C8's stale C7 startup title in `conf.tua`; the normal loaded window already
  used the C8 title. This fix is separate from documentation changes.
- Replaced stale planned/next-audit wording with links through the completed chapters.
- Clarified that guides explain complete checkpoint files, not extra pasted modules.
- Kept the first playable result to Tua, LÖVE and bundled assets. Python, Node and
  contributor regressions are optional, not early setup requirements.
- Clarified that C7/C8 intentionally share save identity/schema, and how an independent
  experiment can choose a separate identity.
- Reworded C2's ambiguous “consumes its slot”: zero-length aim consumes firing cooldown,
  not a projectile capacity slot.
- Retained focused standalone guides, adding links into the integrated series.

Standalone retention follows a consumer audit: `pause-input` and `love-save` are
Makefile targets, and feature pages link to game-flow/save/dialogue first-use guides.
The smaller playable-character/enemy-wave foundations and dialogue starter still have
unique introductory scope and regressions. None was deleted or redirected merely
because a later chapter composes the same API. Future cleanup needs another explicit
consumer/coverage review.

## Limits and final gates

This is an agent-led functional walkthrough of clean complete checkpoints, not a human
study, a manual reconstruction from fragments, or an installed-extension UX test.
Native execution uses the isolated replay launcher rather than leaving an unattended
interactive `love build` process open. The guides' ordinary play command remains the
same LÖVE entry; the runner supplies deterministic input and cleanup for qualification.
No Windows/Linux, controller, international-script, new benchmark, cloud-save or
release-installation claim follows from these checks.

Final gates: documentation build with no reported issues, JSON validation, rendered
local-link/anchor checks, asset hashes and staged whitespace checks. All C1–C9 items
in the current tutorial plan are complete; no push or publication was performed.
