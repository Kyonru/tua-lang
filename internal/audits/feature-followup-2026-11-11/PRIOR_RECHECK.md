# Historical finding recheck

Revision: `ab1b69c506f962ab239e015d2db4c38764fd592a`, macOS arm64, 2026-11-11. This pass revisits the 21 entries previously marked “not individually rechecked” in [PRIOR.md](../feature-and-preview-2026-11-11/PRIOR.md). The historical reports describe an earlier checkout. Their bug-asserting verification scripts are not current acceptance tests: a failure to reproduce an old bug is a successful recheck only when the current response or runtime result is inspected.

The old fixture trees were copied into the ignored `feature-followup-2026-11-11` working directory before execution; source and build outputs in the original reports were preserved. The copied fixtures and raw JSON remain local, while the original tracked historical fixtures permit replay. No private game project was used.

| ID | Current result | Current evidence and limit |
| --- | --- | --- |
| API-MCP-01 | Fixed | Stdio API replay captured 71 events. `analysisRevision` advanced 7 → 12 → 16 → 20 through overlay, refresh, and disk changes. |
| API-MCP-02 | Fixed | New unsaved `src/new.tua` produced TL1003 in both path-specific and project diagnostics before and after refresh. |
| API-MCP-03 | Fixed | Invalid JSON-RPC version and object ID returned `-32600`. No-ID `initialize` produced no response in API and MCP over a 350 ms observation window, followed by a normal ID-bearing response. The old probe expected a notification reply and timed out until its isolated copy was adjusted. |
| ASSETS-01 | Fixed | Changing the output root to collide with user-authored `src/generated/b/data.txt` returned TL2011 and preserved the file's exact `USER AUTHORED CONTENT` bytes. |
| ECS-GAP-01 | Addressed | Current LSP hover on system callback `position` was `{ x: number }`; CLI rejected a wrong `string` assignment with TL1003; imported `world:get` hover included `{ x: number } | nil`. |
| EDITOR-01 | Fixed | Current LSP method completion was `yes`/`no`; audio completion offered domain names `ping`, `master`, `sfx`, with no receiver internals. The historical `verify.js` asserted the old broken label and is obsolete. |
| EDITOR-02 | Fixed | Current shader hover has one balanced `glsl` Markdown fence, followed by source path and expected Lua value. |
| EXPLORERS-01 | Fixed in current tests | `node --test editors/vscode/test/unit/workspace-sidebar.test.js` passed 4/4, including edit/open/close/create/change/delete invalidation. Historical sidebar mock cannot run against current VS Code API (`createFileSystemWatcher` missing). Actual installed-host file event sequence was not separately replayed. |
| EXPLORERS-02 | Fixed in current tests | Same current unit suite includes `.lua` in project files and excludes generated directories. Installed-host explorer interaction was not separately replayed. |
| LINTER-01 | Fixed | Current LSP same-line “Remove unused local” edit retained `print("keep")`; applying `source.fixAll` and running Lua printed `keep`. |
| LINTER-02 | Design choice remains | Explicit `source.removeUnusedImports` still removes an effectful `require`, changing runtime output; organizing imports still changes initialization order. [LSP docs](../../../docs/lsp.md) warn about both actions and exclude them from safe fix-all. No new defect is claimed. |
| LOVE-01 | Fixed | Current CLI and LSP diagnose `Text:set(...):getWidth()` as TL1003, “cannot call a method on nil.” |
| LOVE-02 | Fixed | Current callback parameter hovers include `dt: number`, `key: string`, and `isrepeat: boolean`. |
| LUA-INTEROP-01 | Fixed | Metatable colon-call fixture checked, built, and ran under Lua; final LSP diagnostics were empty. |
| LUA-INTEROP-02 | Fixed | Imported overloaded Lua function rejected the wrong call with TL1003; LSP signature help listed both string and number overloads. |
| PACKAGING-01 | Fixed | Disposable `tua package --target macos` app had no `CFBundleDocumentTypes` or `UTExportedTypeDeclarations` in `Info.plist`. This verifies local macOS packaging, not Windows/Linux packaging or signing. |
| POOLS-02 | Fixed | Nested release returned false for inner release, true for outer; counts remained 0/1/1 and no duplicate checkout occurred. Clear-during-reset remained 0/0/0. |
| SHADERS-02 | Fixed | Same current shader-hover LSP replay as EDITOR-02: no nested Tua fence. |
| SIGNALS-01 | Fixed | Current fixture built and ran with `LUA_PATH='build/?.lua;;'`; nested emits printed `once count 1`. First attempt without runtime module path failed for a harness reason, not a product failure. |
| SIGNALS-03 | Fixed | Current LSP definition reaches `events.tua`; references include declaration and emit sites. |
| TESTING-01 | Substantively fixed | Missing Lua executable path appears in test-runner stderr, and combined coverage report includes a LuaCov hint. Generic report error still appears before the actionable backend line, a minor clarity issue. |

## Replay commands

These commands run against **copies** of the historical fixtures because some probes write generated Lua, JSON, or edited source. Set `AUDIT_DIR` to a disposable copy of `internal/audits/features-2026-11-07`; retain the original layout and run from the repository root. The copied API probe needs to skip its old notification-response wait before its later assertions, because current API/MCP notifications correctly produce no response.

```sh
python3 "$AUDIT_DIR/api_mcp/probe.py"
python3 "$AUDIT_DIR/assets/ownership-repro.py" /tmp/tua-asset-ownership-project "$PWD/target/release/tua"
python3 "$AUDIT_DIR/ecs/lsp_probe.py"
node "$AUDIT_DIR/editor/probe.js"
node "$AUDIT_DIR/explorers/probe.js"
node --test editors/vscode/test/unit/workspace-sidebar.test.js
node "$AUDIT_DIR/linter/probe.js"
node "$AUDIT_DIR/love/probe.js"
node "$AUDIT_DIR/lua_interop/probe.js"
LUA_PATH='build/?.lua;;' lua build/main.lua
python3 "$AUDIT_DIR/signals/lsp_probe.py"
python3 "$AUDIT_DIR/testing/probe.py"
```

For SIGNALS-01, build the copied `signals/once_nested` fixture with `target/release/tua build --project <fixture>` and run the displayed Lua command in that fixture's directory. For POOLS-02, `lua "$AUDIT_DIR/pools/runtime.lua" "$PWD"` uses the checked-in runtime module. For PACKAGING-01, run `tua package --target macos --love /Applications/love.app --name AuditGame --identifier org.example.auditgame` from a disposable minimal project directory, then inspect `Contents/Info.plist` with `plutil`. The asset ownership script accepts its project path from the existing fixture layout; use its source to prepare the initial tree.

## Count and interpretation

All 21 items received a current individual recheck: 18 fixed by direct CLI/LSP/runtime/package replay, two explorer items supported by current focused tests but still lacking a fresh installed-host event sequence, and one explicit import-action design question. The ledger status is 20 fixed/rechecked and one design question. None of these are newly confirmed defects. The remaining host qualification is listed in the [follow-up summary](README.md).
