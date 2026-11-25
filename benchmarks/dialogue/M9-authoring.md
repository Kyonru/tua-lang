# M9 authoring, diagnostics and preview qualification

Qualified on 2026-11-24 on macOS arm64. The source baseline was `97ac76a8` plus the
M9 changes committed with this report:
`145e5759` (forwarded definitions), `9aa2eeb5` (reachability/entries),
`f90df324` (Lua dependency overlays) and `c17e4472` (runtime issues). No changes were made to the consuming game,
no release was published, and the user's normal extension profile was not modified.

## Completed contract

| Surface | Evidence and scope |
| --- | --- |
| Static diagnostics | Shared parser rejects dangling targets, duplicate identities and malformed conditions. Shared analysis warns about unreachable sections using the first label and explicit `@entry label` roots. Context-free conditions can prune routes; dynamic context and action outcomes cannot. |
| CLI/LSP parity | Core and CLI regressions verify warning-only behavior, unchanged default entry and preserved emitted nodes. LSP regression verifies warning severity, UTF-16 ranges, unsaved declarations and rename edits. |
| Refactoring | Static text IDs connect `.tdialogue`, configured catalogs and recognized typed `.tua` localization calls. Existing core/LSP tests cover namespace preservation, duplicate/malformed-source rejection, nested project isolation and conflicts. Native checks apply a three-file rename and reject an unsaved catalog conflict. Flow labels and `@entry` references rename together within a script. |
| Authoring | Core/LSP fixtures cover labels, placeholders, context paths, typed actions/events, expressions and authored voice cues. Native editor checks exercise label/character/context/placeholder/expression completion, character hover and definition, references, rename and document formatting. |
| Freshness | Native checks change unsaved imports/catalogs, delete/recreate an imported forwarding module, edit constant branches and recover incomplete targets. Existing CLI watch tests retain last-valid output and recheck duplicate IDs, characters and catalog placeholders. Immutable core snapshots retain their original revision. |
| Preview | Native development-host and installed-VSIX tests cover source navigation, invalid edits, clean/dirty deletion and recreation, dirty discard, hidden views and refresh. Existing graph tests cover loops, bounds, focus and navigation. Mock results remain explicit static probes. |
| Runtime issues | Inspector samples issue code and node/action identity, plus the configured automatic-transition budget on exhaustion. Existing public action counters, translation provenance and optional bounded history supply cancellation/fallback diagnostics without text, payloads or private error messages. Lua and native LÖVE checks verify the runtime adapter. |

## Gaps found and fixed during qualification

- A forwarded shared-character definition navigated to the forwarding module.
  Dialogue character navigation now follows the indexed export's source, consistent
  with expression navigation; the original definition has a core regression.
- Unsaved Lua catalog changes did not reach the server because Lua files are not
  owned by Tua's language providers. Workspace Lua dependency overlays now use the
  existing auxiliary-document synchronization mechanism. This preserves provider
  ownership, sends initial/open/change/close state, skips documents already owned
  by the language client, and disposes/recreates listeners on server restart.
- Native test setup now reports the last server debug information if startup fails.
  Formatting qualification applies a real indentation edit, rather than expecting
  edits for an already formatted document.

## Environment and artifacts

- VS Code 1.142.0, commit `6d62b1d5dace93a33b3a6d3eca1317bd5e15bcb1`, arm64.
- Tua 0.11.0 development compiler built from the current branch using `cargo build
  -p tua-cli`; this is an unoptimized qualification binary, not a release build.
- Compiler SHA-256: `2edacdcff7eac0e9f896e45aaf61043c51edd816d9f07a2408ea82f50002e765`.
- Node v24.15.0; standalone Lua 5.4.8; native LÖVE 11.5.
- Installed development VSIX: `/tmp/tua-m9-qualification.vsix`.
- VSIX SHA-256: `f8db7df1601e75249143012954d1c13daf035031fcc6bc579349685b386402b2`.
  Archive integrity passes. The test installs only into a temporary extension/user
  directory and uses the bundled compiler, with no development-server override.

## Verification and reproduction

```sh
make verify
make docs-build json-check
make lsp-features-smoke
target/debug/tua fmt --check editors/vscode/assets/inspector/dialogue.lua \
  editors/vscode/test/fixtures/dialogue-inspector-localization.lua
TUA_REAL_SERVER_PATH="$PWD/target/debug/tua" TUA_REAL_SERVER_SUITE=dialogue-preview \
  node editors/vscode/test/real-server/run.js
TUA_TEST_VSIX=/tmp/tua-m9-qualification.vsix TUA_REAL_SERVER_SUITE=dialogue-preview \
  node editors/vscode/test/real-server/run.js
TUA_BIN="$PWD/target/debug/tua" node editors/vscode/scripts/runtime-inspector-e2e.js
```

`make verify`, documentation/JSON checks, formatting, LSP feature smoke, and the
native inspector and installed-VSIX suites all pass.

After final parser hardening, the complete core library suite passed 2,190 tests
(with one existing ignored latency probe), the LSP suite passed 523 tests, and the
focused CLI reachability/emission regression passed. Standalone core tests use
`RUST_MIN_STACK=16777216`, as `make verify` does; an initial standalone invocation
without that setting exhausted the stack in an existing module-contract test.

The broad gate retains three intentional ignored checks: external glslang validation,
a dedicated graphical shader-validation probe, and a manual corpus-latency probe.
The separate native LÖVE inspector test is not a substitute for those shader probes.

The extension gate includes 317 unit tests, grammar, theme, activation and listener
lifecycle checks. Native editor runs report `REAL_SERVER_DIALOGUE_AUTHORING` and
`REAL_SERVER_DIALOGUE_PREVIEW`; the LÖVE run verifies independent inspector sessions,
redaction, weak ownership, pause/resume and inspector-free packaged games.

## Explicit boundaries

M9 supports the static identities above; it does not infer arbitrary host strings,
rename action/document identity in saved games, or promise registry-member refactors.
External entries declare analysis roots; they do not restrict which nodes host code
may start. Reachability warnings do not prune emitted data.

Preview does not run host actions, load assets or simulate conversations. Rich
styled/localized preview rendering, voice asset discovery without a host catalog,
automatic discovery of unopened nested projects, and range/on-type formatting are
extensions beyond this milestone's current contract. Runtime history is sampled,
not a complete event trace: internal deferred handles and resolvers without
provenance remain unknown. Release-build packaging, the rendered showcase and game
integration remain M10. These tests do not measure perceived UI latency or qualify
other operating systems.
