# Tua 0.7–0.9 game-building developer-experience audit

The later [implementation and qualification follow-up](RESOLUTION-2026-11-13.md)
records fixes for the confirmed findings, remaining design questions, editor
timings, and the local test VSIX. The baseline findings below remain the
original trial observations.

## Executive summary

Three agents built distinct small, playable LÖVE games against the current 0.9.0 checkout: [arena defense](trials/arena/REPORT.md), [alchemy](trials/alchemy/REPORT.md), and a [stealth maze](trials/maze/REPORT.md). Each combined 0.7.0 generics/protocols/classes, 0.8.0 game-domain helpers and toolchain workflows, and current 0.9.0 editor/build behavior. All three games passed their final check/build/test runs and bounded LÖVE runtime smokes after documented workarounds; their 12 Busted tests passed. This supports the practical value of the feature set, especially composing behavior trees, ECS, signals, stores, state machines, pools, audio, and procedural helpers in one game.

The audit found **nine confirmed defects**: **five High** and **four Medium**. The most serious is a silent emitted-Lua error: a subclass `init` with `super(); self.speed = 120` on one line constructs an object with speed 0, while the multiline equivalent yields 120 ([GAME-08](FINDINGS.md#game-08-constructor-lowering-discards-assignments-after-inline-super)). Other costly boundaries are shader paths that check/build but fail in the built LÖVE game ([GAME-02](FINDINGS.md#game-02-project-root-shader-paths-fail-after-source-root-build-relocation)), imported class/protocol identity producing false diagnostics ([GAME-01](FINDINGS.md#game-01-imported-protocol-bound-changes-class-subtype-assignability), [GAME-03](FINDINGS.md#game-03-imported-class-constructor-and-factory-values-lose-receiver-identity)), and imported signal payloads silently losing type checks ([GAME-04](FINDINGS.md#game-04-and-game-05-imported-signal-payloads-disagree-across-checker-and-signature-help)). The remaining confirmed editor/formatter defects are detailed in [FINDINGS.md](FINDINGS.md).

Two lower-severity typing/diagnostic usability gaps, one design/documentation question, and one unverified formatter candidate are tracked separately. The trial games were simulated workflows, not a human usability study. There was no installed VS Code visual/menu test in this audit, so LSP responses and timings are labeled as protocol-level evidence. An earlier arena LÖVE smoke passed; later clean-source graphical-host replays stalled, as did a plain-Lua LÖVE control, so the host currently blocks repeat qualification of that game. See the [coverage matrix](COVERAGE.md), [verification log](VERIFICATION.md), and [prioritized backlog](BACKLOG.md).

## Scope and method

Three simulated developer trials build distinct small LOVE games from the current 0.9.0 checkout. Each trial follows the same [feature checklist](COVERAGE.md), records the actual code and commands, and separates checked source, emitted Lua, runtime observations, and editor responses. The games are exploratory fixtures, not claims of human usability or substitutes for a platform qualification run. The lead reconciles overlapping symptoms and rechecks consequential findings before ranking them.

Starting revision: `6bb460d0` on 2026-11-13. The tracked worktree was clean. No `AGENTS.md` was present in this checkout. `target/debug/tua --version` reported 0.9.0; the installed tools included LÖVE 11.5, Lua 5.4.8, and Busted 2.3.0. `luajit` was not on PATH. The three games live under `trials/arena/`, `trials/alchemy/`, and `trials/maze/`; generated build and package output is excluded from the audit artifact.

## Baseline checks

| Check | Result | Limit |
| --- | --- | --- |
| `target/debug/tua check` from `examples/features` | Passed: 45 files | Baseline example, not a trial game |
| `target/debug/tua build` from `examples/features` | Passed: 53 files | Does not prove gameplay |
| `node editors/vscode/scripts/lsp-features-smoke.js target/debug/tua examples/features` | Passed | Protocol-level fixture, not visible editor latency |
| `target/debug/tua package --target love` from `examples/love-minimal` | Passed; 1.0 KiB archive and `unzip -t` passed | Portable archive only; no desktop/web/custom target test |
| `tua init` then `tua actions init` in `/private/tmp/tua-game-devex-actions.d83jTL` | Passed; created project and workflow; a second `actions init` left the workflow unchanged | Generated workflow was not run on GitHub |

The prior preview pass at this revision completed `make verify`, the installed VS Code webview suite, and a 0.9.0 VSIX build; see the [preview review](../preview-peer-design-2026-11-13/README.md). This audit will not treat those earlier checks as evidence that a new trial game works.

## Trial results and ease of use

| Game | Final verification | What composing features felt like |
| --- | --- | --- |
| [Arena defense](trials/arena/REPORT.md) | Check/build/lint/format pass; 5 Busted tests; 90-frame LÖVE smoke; 180-frame shader-include reload | Domain examples made APIs discoverable; generic slots and protocol-bound enemies worked after type and constructor workarounds. Silent constructor behavior and imported class diagnostics were the largest interruptions. |
| [Alchemy](trials/alchemy/REPORT.md) | Check/build/lint/format pass; 3 Busted tests; LÖVE gameplay smoke and synchronized shader reload | Generic recipe/jar and signal/store/ECS composition worked. The documented-looking shader path failed only at runtime; unfinished imported-generic completion disappeared during typing. |
| [Stealth maze](trials/maze/REPORT.md) | Check/build/lint/format pass; 4 Busted tests; LÖVE boot and 20-step winning route; headless cue/music smoke | Rot, behavior-tree, ECS, signal, store, and state-machine interactions were direct once update ownership was clear. Cross-file nominal typing errors required an explicit annotation or direct-construction workaround. |

**Dependable in these bounded trials:** source modules and plain Lua interop; checked generic/protocol positive and negative examples; generated helper inclusion; Busted test running; the exercised runtime interactions among ECS, behavior trees, stores, signals, state machines, pools, Rot, sprites, audio, and shaders after correct paths. **Needs attention:** constructor lowering, cross-file type identity and signal schemas, source/build shader paths, incomplete-edit completion, nested generic hover, formatter state, and warning precision. A passing final game does not erase the workarounds or imply every subfeature was qualified.

The [28-row matrix](COVERAGE.md) records 84 game-feature cells and distinguishes runtime, checker, protocol, inspected, and partial evidence. Most rows have runtime evidence in each game, but physical input, precise sprite timing, audible/visual quality, preview webviews, visible suggestion latency, long sessions, platform packaging, and hosted CI remain unqualified. Small-project command timings varied substantially under concurrent load; they are observations, not a benchmark or release target.

## Recommended next work

Fix the silent constructor miscompile first, then unify shader asset path meaning from source through build and runtime. Next, repair shared imported class/protocol identity and SignalBus event provenance so checker and editor agree. Address incomplete-edit member completion, generic-field hover, and formatter indentation after the shared semantic fixes. The [backlog](BACKLOG.md) gives acceptance criteria and regression coverage. No product source was changed as part of this audit.
