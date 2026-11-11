# Tua feature and preview audit — 2026-11-11

Baseline `c08edbca` (0.7.0); this report records the audit and local fixes since that revision. The audit used bundled feature projects, focused cross-feature fixtures, actual CLI/LSP requests, installed VS Code webviews, generated Lua execution under Lua 5.4, and three LOVE 11.5 example smoke tests outside the restricted graphical sandbox. It is a risk-selected investigation, not proof of every possible feature combination or platform.

## Executive assessment

The normal first-project path is dependable in the tested environment: init, check, lint, format, build, Busted tests, source-map traceback, watch recovery, `.love` packaging, and macOS app packaging worked. The installed editor handled core generic/protocol completion, hover, navigation, signatures, unsaved edits, and representative rendered previews. A 100-module release-binary protocol replay was responsive once warm, but it did not measure the visible completion menu.

The audit found real interaction failures that isolated tests missed: unrelated classes from different modules could pass runtime `is`; imported protocol bounds could be lost in generic class methods; a SignalBus stored in a field lost `emit` varargs; and a previously generated helper could remain stale. Imported ECS field typing and nilability of absent `get`/`remove` results also failed. These are fixed with focused regression tests in separate commits. An absolute-path project/LSP replay caught and fixed the remaining imported-component hover loss. Three preview usability defects and a misleading feature-check description were fixed as well. See FINDINGS.md for exact dispositions and remaining qualification.

The biggest verification limits are selected rather than exhaustive language/domain coverage, 21 of 74 historical November 7 findings not individually rechecked, no standalone LuaJIT executable on this host, graphical LOVE failing inside the restricted sandbox, long editor sessions, cross-platform packages, and a previous-release upgrade/rollback run. LOVE smoke tests passed outside the sandbox, but that does not qualify all game interactions. `COVERAGE.md` records 391 inventory rows with tested, inspected-only, or blocked results; a tested grouped row covers only its cited subcases.

## Where to review

- [FINDINGS.md](FINDINGS.md): confirmed defects, reproductions, causes, regressions, and dispositions.
- [COVERAGE.md](COVERAGE.md): initial inventory, expanded commands/settings/methods, and executed interaction cases.
- [INTERACTIONS.md](INTERACTIONS.md): risk-selected game-domain combinations and exact outcomes.
- [PREVIEWS.md](PREVIEWS.md) and [EDITOR.md](EDITOR.md): rendered-host task observations and editor evidence.
- [LANGUAGE.md](LANGUAGE.md), [DOMAINS.md](DOMAINS.md), [DOCS.md](DOCS.md), and [DOCS_DEEP.md](DOCS_DEEP.md): specialist evidence and documentation checks.
- [PRIOR.md](PRIOR.md): historical November 7 finding IDs with current recheck status.
- [BACKLOG.md](BACKLOG.md): ordered remaining correctness, capability, usability, performance, and qualification work.
- [VERIFICATION.md](VERIFICATION.md): exact commands, outcomes, host/runtime versions, and blockers.

## Handoff status

All ten confirmed findings in this audit have local fixes (one High, eight Medium, one Low). The final post-edit `make ci-check` and installed-VSIX real-server test passed. The 0.7.0 test package is [tua-0.7.0.vsix](../../../editors/vscode/tua-0.7.0.vsix), SHA-256 `febe8955858cbad84853f9917e9dfe7ac536242f6a8bc05ec40d0b64fc029deb`; it is for manual testing and was not published. The [follow-up](../feature-followup-2026-11-11/README.md) individually rechecked the remaining 21 historical IDs; the 74-item ledger now records 72 fixed/rechecked, one design question, and one superseded documentation claim. Independent final review is recorded in FINAL_REVIEW.md. This is local qualification, not a published-release approval.
