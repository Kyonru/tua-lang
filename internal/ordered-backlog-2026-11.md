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
- Pending: recursive imported `Self`.
- Pending: callback/event call results across modules.
- Pending: captured narrowed locals and full-method nested receivers.

## 2. Navigation and rename

- Pending: native class parameter definition, references and rename provenance.
- Pending: side-effect import global navigation.

## 3. Callback syntax and checker support

- Pending: anonymous optional/default parameters.
- Pending: assertion and stable member-path narrowing.
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
