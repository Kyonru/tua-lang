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
- Completed: stable plain-record member paths share source-position guard queries
  across checking, completion and return inference. Calls, alias/member writes,
  root changes and captures invalidate refinement; metatable paths remain
  conservative. Core/LSP verification: `member-final.log` (1,618/375).
- Completed: `table.sort` shares an array-derived comparator contract across
  checker and editor queries, including omitted/nil comparators and custom
  library shadows. Callback bodies use contextual parameter/return contracts;
  nested parameter shadows do not inherit outer callback types. Core/LSP
  suites pass (1,621/375, `sort-final.log`).

## 4. LOVE and assets

- Completed: generated image decoder and file-data constructor inputs match
  LOVE File/Data conversion rules. Core/LSP tests pass (1,623/375,
  `love-core-lsp.log`); pinned-catalog drift check passes (`love-drift.log`).
  Actual headless LOVE File/FileData/ByteData smoke passes (`love-runtime.log`);
  this is API qualification, not installed-editor or visual game qualification.
- Completed: cached lexical scope facts filter asset metadata, completion,
  navigation/rename, diagnostics and shader inspection. Incomplete strings and
  scope exit keep builtin suggestions. Local/parameter/global helper shadows
  and custom LOVE roots have regression coverage. Core/LSP suites pass
  (1,626/375, `assets-final.log`).
- Completed: initial LOVE window reads share the stable-path resolver while
  retaining nullable writes. Option writes preserve the known initial table;
  replacement, calls, alias writes and capture end the guarantee. Core/LSP
  suites pass (`conf-core-lsp.log`); explicit guard follow-up is covered in
  `conf-final-controls.log`. Headless LOVE confirms the initial table and nil
  disabling (`conf-runtime.log`). Literal-false type modeling remains pending.

## 5. Migration support

- Completed: HUMP instance declaration subset and a native typed timer adapter
  document receiver binding, cancellation and single update ownership. Tagged
  engine timers remain explicit adapter boundaries. The declaration editor test
  passes (`timer-focused.log`); feature check/build and docs build pass. The
  emitted adapter passes delayed/cancelled execution against BYTEPATH's actual
  HUMP runtime (`timer-runtime.log`).
- Completed: suppressible `TL4077` identifies field defaults that follow direct
  source-backed initializer writes, using cached lexical identities. Generated
  assignments, shadowed receivers, branches and deferred callbacks are excluded.
  Core/LSP suites pass (1,630/375, `init-core-lsp.log`); docs and JSON checks
  pass (`init-docs.log`). Runtime initialization order is unchanged.

## Revisit remaining work

Recheck the roadmap after these phases, including unclosed classes, annotation
editing, cold completion and runtime/editor qualification. Protocols/generics
and changed initialization order need explicit compatibility design. Do not
substitute check/build success for installed-editor, Steam, legacy-runtime,
visual or long-session evidence.

- Revisited TL4054: anonymous-function annotation collection already fixed the
  reported nested-callback import use. A permanent linter regression now proves
  that the used alias stays live while an unused sibling import still warns
  (`import-usage.log`). Removed the resolved roadmap entry.

- Completed TL4005 follow-up: recursively parsed default facts feed the shared
  lexical scope map, with a depth bound matching default lowering. Outer reads
  count as uses and support definition/rename; inner parameter shadows remain
  distinct. Core/LSP suites pass (1,633/375, `default-usage-core-lsp.log`).

- Completed unclosed-class recovery: shared class scanning provides precise
  syntax guidance and editor-only closure of missing braces, bounded to 32.
  Cold snapshots retain class members, outer locals and valid nested Lua scopes;
  strings/comments are not repaired as code. Strict source parsing still fails.
  Verification: `class-recovery-core-lsp.log`.

- Completed annotation discovery: parser recognition, tag completion and tag
  hover share an 18-tag catalog. Generated web and offline guides explain
  checking versus metadata semantics; the extension command opens only its
  bundled resource. Focused parser/completion/hover tests and extension/package
  checks pass. Partial annotation hover, signature documentation updates and
  annotation type navigation remain in the roadmap.

## Performance revisit

The first final replay caught new overhead (291 ms cold, 62 ms post-edit,
27 ms incomplete-edit completion). Semantic queries rebuilt the entire alias
map repeatedly. They now borrow the immutable map already owned by the type
environment; inference and snapshot invalidation are unchanged.

Three alternating accepted-baseline/candidate sessions ran without concurrent
builds/tests. Baseline binary SHA-256 is
`725823130d919faad937fcdd3a01237efea999dd6bed8452acd3823b8b3d4799`.
All expected response and completion-label assertions passed. These are local
protocol measurements, not VS Code key-to-menu latency or a cold filesystem.
Evidence: `target/ordered-backlog/borrow-{baseline,candidate}-{1,2,3}.json`
and `borrow-aggregate.json`. The initial regressed measurements are retained as
`performance-*.json`.

| Request | Baseline p50 / p95 ms | Final p50 / p95 ms |
| --- | ---: | ---: |
| `cold.module-completion` | 282.5 / 286.1 | 288.3 / 290.9 |
| `warm.self-completion` | 1.0 / 1.2 | 0.9 / 1.1 |
| `warm.module-completion` | 0.5 / 0.5 | 0.5 / 0.7 |
| `warm.hover` | 9.8 / 23.5 | 9.3 / 21.4 |
| `warm.format` | 3.3 / 3.6 | 3.4 / 3.5 |
| `edit.module-completion` | 53.4 / 67.2 | 54.1 / 65.5 |
| `edit.hover` | 18.0 / 19.6 | 17.0 / 18.7 |
| `partial.self-completion` | 23.9 / 27.7 | 24.1 / 29.5 |
| `cross-file.module-completion` | 26.6 / 77.6 | 23.6 / 71.0 |
| `cross-file.hover` | 15.3 / 21.1 | 17.6 / 20.3 |
| `immediate.format` | 25.9 / 27.9 | 24.4 / 26.1 |
| `post-save.format` | 16.3 / 18.2 | 16.5 / 21.0 |
| `save.hover` | 47.4 / 50.2 | 46.4 / 50.5 |
| `other.main-completion` | 0.4 / 227.8 | 0.4 / 240.2 |
| `other.main-hover` | 0.5 / 13.0 | 0.6 / 13.1 |
| `other.combat-completion` | 0.4 / 0.5 | 0.4 / 0.6 |
| `other.combat-hover` | 1.2 / 71.0 | 1.3 / 73.7 |
| `import.path-completion` | 0.4 / 0.7 | 0.4 / 0.7 |
| `contended.inline` | 0.2 / 0.3 | 0.2 / 0.3 |
| `contended.self-completion` | 1.1 / 1.2 | 0.9 / 1.0 |
| `contended.hover` | 9.6 / 18.4 | 9.3 / 19.8 |

Cold completion and cross-file hover remain optimization work. The larger
semantic feature set is not a claim of zero latency cost: cross-file hover
in this replay rises from 15.3 to 17.6 ms. Installed-editor and long-session
qualification remains pending.
