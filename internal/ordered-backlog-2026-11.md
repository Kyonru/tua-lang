# Ordered porting backlog implementation — November 2026

This pass starts at `6b27932` and follows the requested order. Each independent
behavior gets its own commit, regression coverage and roadmap update. Evidence
is retained under `target/ordered-backlog/`. Items are only closed after their
reproductions and relevant checks pass.

## 1. Completion and inference

- Completed: indexed expression receivers, including unsaved imported types.
  Core/LSP suites pass (1,599 core and 375 LSP tests), followed by six focused
  receiver checks after the final scanner guard. The full Gravity Wars replay
  restores `State.allBullets[b].` members (`x`, `y`, `vx`, `vy`) and numeric hover.
  Evidence: `indexed-core-lsp.log`, `indexed-final.log`, `gravity-indexed.json`.
- Completed: chained call completion, hover and signatures, including incomplete
  LOVE calls, void boundaries and unsaved imported factory return types.
  Core/LSP suites pass (1,602 core, one ignored; 375 LSP); the feature smoke
  passes. Evidence: `chained-core-lsp.log`, `chained-smoke.log`.
  Call-result navigation remains in the navigation phase.
- Completed: recursive imported instance aliases retain nominal `Self` references.
  Selected/namespace imports, repeated arrays/fluent returns, and unsaved producer
  edits have regression coverage. SNKRX follower completion/hover is restored
  (`self-snkrx.json`); source-class parameter/parent contexts still fail and stay
  in the class-context phase. Recursive LuaCATS controls remain unchanged.
- Completed: imported method result locals propagate into cross-file event
  handlers. A bounded import-aware payload pass shares annotation resolution
  with editor queries; unknown/cyclic event inference stays conservative.
  The regression checks unsaved return-type changes and old snapshot isolation.
  All 1,604 core and 375 LSP tests pass (`event-core-lsp.log`); the gameplay
  replay reports `dead: boolean` (`event-gameplay.json`).
- Completed: source-class `Self` parameters and guarded parent receivers use
  resolved contracts and correct source/parser coordinates. SNKRX parameter
  fields and parent metadata work (`class-snkrx.log`); core/LSP suites pass
  (1,605/375, `class-core-lsp.log`).
- Completed: Emoji Merge captured locals retain non-nil guards after member
  writes and OR-guard fallthrough. Root reassignment still invalidates narrowing.
  Saved and unsaved callback completion now pass (`capture-emoji.log`);
  1,606 core and 375 LSP tests pass (`capture-core-lsp.log`).

## 2. Navigation and rename

- Completed: native class parameters use lexical binding IDs and source-backed
  header fragments for definition/references/rename. Multiline/defaulted headers
  and nested shadowing pass; the Emoji Merge rename replay succeeds
  (`navigation-emoji.log`). All 1,607 core and 375 LSP tests pass
  (`navigation-core-lsp.log`). Default-expression errors now point to the header.
- Completed: unique global functions reachable through static runtime imports
  navigate across files; cycles, local shadows and competing definitions have
  coverage. Snapshot revisions prevent stale dependency navigation. Gravity Wars
  now reaches `globals.tua` from `loadExternalAssets()` (`globals-gravity.log`).
  All 1,608 core and 375 LSP tests pass (`globals-core-lsp.log`).

- Completed: native call-result receivers support definition/references/rename;
  chained LOVE methods navigate to builtin API definitions, and void results
  yield no invented target. All 1,609 core and 375 LSP tests pass
  (`fluent-nav-core-lsp.log`), followed by a passing nested-call field regression
  after broadening the shared receiver fallback (`fluent-nav-final.log`).

## 3. Callback syntax and checker support

- Completed: anonymous optional/default parameters use parsed contracts and
  syntax-ordered lowering. Single-line/nested callbacks, helper shadowing, long
  strings and existing formatting regressions pass. All 1,613 core and 375 LSP
  tests pass (`callback-core-lsp.log`). Both saved optional-callback port repros
  check; `callback-runtime` checks/builds and executes successfully with Lua.
  Nested-default-only outer reads still cause a false TL4005 warning; retained
  in the roadmap's usage-tracking follow-up.
- Completed: builtin assertions narrow locals consistently across checker and
  editor queries. Local/global shadows, `_G.assert` replacement, reassignment
  and argument calls have conservative regression coverage. Core/LSP suites
  pass (1,616/375, `assert-core-lsp.log`); final argument-call controls pass
  (`assert-final.log`).
- Pending: stable member-path narrowing.
- Pending: contextual `table.sort` comparator types.

## 4. LOVE and assets

- Pending: File/FileData overload accuracy.
- Pending: binding-aware asset metadata and editor behavior.
- Pending: nullable LOVE configuration flow.

## 5. Migration support

- Pending: reusable timer/engine declarations and concrete mixin/service recipes.
- Pending: initialization-order discoverability without changing semantics.

## Revisit remaining work

Recheck the roadmap after these phases, including unclosed classes, annotation
editing, cold completion and runtime/editor qualification. Protocols/generics
and changed initialization order need explicit compatibility design. Do not
substitute check/build success for installed-editor, Steam, legacy-runtime,
visual or long-session evidence.
