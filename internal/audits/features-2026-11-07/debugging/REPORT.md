# Debugging, source maps and runtime inspector audit

Baseline: revision 9d490dcd299bd25218c9fa4adc83e080e617453c, prebuilt release CLI 0.6.0; pending version/release edits preserved. Applied tua-audit-lead, tua-language-auditor and tua-lsp-parity. No product edits, installs or commits.

Fresh checks support the main workflow: the installed Second Local Lua Debugger 0.3.9 stopped native LOVE at the expected original Tua breakpoint, default-argument stack frames remapped correctly, and the native inspector delivered five categories across two independent players. One confirmed medium defect affects traceback paths containing spaces. This is a bounded audit, not exhaustive debugger certification.

## Coverage and evidence

| Surface | Status | Evidence/result |
|---|---|---|
| CLI emission + default argument traceback | Tested | `fixture/`, `raw/build.log`, `raw/default-trace.log`, `raw/default-remapped.log`; defaultValue error maps to line 2, default invocation line 5, caller line 8 |
| Lowered class map provenance | Tested | Method error generated line 17 maps to original line 7; caller line 33 to line 12; source maps preserved in fixture/build |
| Actual stdio LSP generated-file routing | Tested | `node probe.js`; `raw/lsp-navigation.json`, two `tua/projectLayout` requests, including escaped space in URI |
| Source/generated line navigation | Tested at production helper boundary | Same probe loads emitted maps and queries forward/backward nearestMapping; class line 7 ↔ Lua line 17 |
| Mixed separators and relative paths | Tested | `raw/mixed-input.log`, `raw/mixed-remapped.log`; normal, backslash and ./ paths remap; spaced directory fails |
| Actual DAP breakpoint/stack/continue | Tested | `raw/debugger-e2e-unrestricted.log`; installed adapter + native LOVE, mapped breakpoint and stack at src/main.tua:4, continue to termination |
| Debug launch/session restore | Tested with mocked VS Code | `raw/focused-unit.log` (17 pass across debug instances and inspector origins/view), `raw/activation-smoke.log` (exit 0; script silent); launch opt-in, bootstrap/map shifts, restore, independent sessions are assertions in existing scripts |
| Inspector data and control | Tested native LOVE | `raw/inspector-e2e-unrestricted.log`: two players, ECS/state/signal/pool/store data, cycle/string bounds, pause/manual refresh/resume, file changes, independent exit, cleanup, package without shim |
| Compiled Tua runtime creation-site routing | Tested native LOVE | `node runtime-probe.js`; `raw/runtime-mapped.json` shows generated main.lua:2 → player at src/main.tua:1, state.score 13, updates 0 for direct mutations |
| Inspector options/transport | Tested focused suite | `raw/inspector-unit-unrestricted.log`: 8 pass; opt-in, category filtering, empty categories, interval clamps, transport limits, bounded history |
| Full VS Code interactive UX | Not exercised | No GUI automation or normal profile changes; cannot claim actual sidebar reveal/picker, F5 UI or generated-editor focus tested |
| Debug expressions/watch/conditional breakpoints; Windows platform | Not exercised | Adapter delegated features; direct DAP smoke establishes breakpoint/stack/continue only; separator input is not a Windows run |

Commands were run from repository root unless noted. Native commands:

```sh
TUA_BIN="$PWD/target/release/tua" TUA_LOVE=/Applications/LOVE.app/Contents/MacOS/love node editors/vscode/scripts/runtime-inspector-e2e.js
TUA_BIN="$PWD/target/release/tua" TUA_LOVE_COMMAND=/Applications/LOVE.app/Contents/MacOS/love TUA_DEBUGGER_EXTENSION_PATH=/Users/kyonru/.vscode/extensions/ismoh-games.second-local-lua-debugger-vscode-0.3.9 node editors/vscode/scripts/love-debugger-e2e.js
node internal/audits/features-2026-11-07/debugging/probe.js
node internal/audits/features-2026-11-07/debugging/runtime-probe.js
```

Initial sandbox inspector runs failed with `listen EPERM 127.0.0.1`; approved escalation allowed isolated localhost tests to pass. Initial runtime fixture used unsupported compound assignment through a call result; corrected to a local state binding before final run, not counted as product defect. Lua oracle: /opt/homebrew/bin/lua 5.4.8; native LOVE executable as above.

## DBG-01 — Spaces in generated paths disable traceback remapping

**Medium / confirmed defect / new relative to checked roadmap and prior devex audit.** A normal project subdirectory containing a space loses source links in CLI traceback output and the extension traceback helper, while LSP path routing and emitted provenance remain correct.

Reproduce from `debugging/fixture` (set `TUA` to repository target/release/tua):

```sh
"$TUA" build
/opt/homebrew/bin/lua 'build/nested dir/crash.lua' > ../raw/class-trace.log 2>&1
"$TUA" traceback ../raw/class-trace.log
```

Expected first error location: `src/nested dir/crash.tua:7 (generated build/nested dir/crash.lua:17)`. Actual: original `build/nested dir/crash.lua:17` remains unchanged. `probe.js` passes the same real runtime lines through production extension `remapTracebackLine`; result is null. Maps independently report generated 17 → source 7. The non-space default function control remaps all three frames.

Cause: `crates/tua-cli/src/traceback/mod.rs:161–176` and `editors/vscode/src/editor/source-maps.js:207–223` treat any whitespace as a path boundary, extracting `dir/crash.lua` instead of `build/nested dir/crash.lua`. Normal Lua traceback filenames are not quoted. This is shared behavior implemented separately in Rust and JavaScript; one coordinated fix should cover both.

Existing blind spot: CLI traceback tests cover direct and ambiguous basename paths, not whitespace-containing paths; extension activation smoke uses build/main.lua. Smallest improvement: match known generated map paths ending at `.lua:<line>` before falling back to token scanning, retaining ambiguity protection and prefixes. Add real Lua traceback regression tests for spaces in filename/directory, absolute project roots with spaces, slash/backslash forms, and duplicate suffixes in both transports.

## Missing options and bounded follow-ups

No additional confirmed defect. Docs explicitly require a fresh Tua launch after source edits; debugger-only Restart does not rebuild. A Tua rebuild-and-restart action could improve this workflow but is a documented feature gap, not a hidden regression. Inspector arbitrary user tables and custom runtimes are intentionally excluded; sampling/category/interval controls are documented and tested. Further verification should target actual VS Code F5/provider failure cleanup, generated-editor focus, instance picker/reveal, and expression/watch stepping across lowered class helpers. No unsupported UX claims or absent capabilities were counted as defects.

Checked ROADMAP.md and prior devex-2026-11-06 findings/backlog plus release-prep 0.6.0 baseline for known issues; no matching whitespace traceback item found. No cargo rebuild or full suite required; focused extension checks and actual public-process workflows used instead.
