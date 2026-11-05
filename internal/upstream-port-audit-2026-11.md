# Upstream porting audit — November 2026

## Scope and method

Five agents migrated complete applications with native Tua contracts at stable
boundaries, preserving application systems, assets and licensed Lua dependencies.
The full deliverables are in `full-port` directories. Earlier focused subsystem
trials remain useful regression evidence, but are not counted as full ports.
These are synthetic development trials, not human usability studies. Agents ran
in waves of three due to session concurrency limits; a fresh reviewer checked
the combined fixes and reports before handoff.

Baseline: `0803560` (compiler implementation at `4bbcfc3`). Upstream checkouts,
ports, retained MIT notices, individual reports, and replay scripts are isolated
under `/private/tmp/tua-port-audit/<trial>`. Verification and before/after LSP
responses are in `target/tua-port-audit`. Upstream projects and assets are not
vendored into the Tua product tree. Each report distinguishes porting mistakes,
documented limitations, confirmed defects, and proposed features.

## Phase 1: subsystem inventory and runtime evidence

| Trial / pinned upstream | Rewritten behavior | Verified scope |
| --- | --- | --- |
| [Sprit3r](https://github.com/evgiz/sprit3r/tree/ff693c07421e01f2b34b0ab03db43f606ee41dd8), `sprit3r` | Viewer class, camera controls, sprite stacking, sheet layout, voxel projection | Check/build; camera and voxel assertions; 12 render-command comparisons against upstream; real LOVE three-frame smoke. SUIT UI, file watching/drop loading, binary VOX import omitted. |
| [SNKRX](https://github.com/a327ex/SNKRX/tree/6b93a64d694d59472375467648868ae4521d6706), `snkrx` | Formation history, follower movement/speed, removal, leader promotion | Check/build; 20 paths, 6,000 history updates, 30,000 follower positions compared with upstream; ownership assertions; real LOVE smoke. Full combat, physics, Steam, menus and assets omitted. |
| [emoji-merge](https://github.com/a327ex/emoji-merge/tree/3b63aaa6b174f59cbcc3423ffafd558d60c876a5), `emoji-merge` | Weighted selection, drop safety, collision eligibility, score/chains, delayed merges, typed timer | Check/build; deterministic assertions for selected upstream rules; real LOVE eight-frame smoke. Collision events supplied explicitly; full physics, Anchor engine, art/audio and saves omitted. |
| [BYTEPATH](https://github.com/a327ex/BYTEPATH/tree/51ee3086ae3369a2c80e4e47d4b62d480af4fe89), `bytepath` | Stat accumulation/reset, director wave budgets, scheduler and wave display | Check/build; 20 Stat updates and four wave budgets compared with original upstream functions; real LOVE eight-frame smoke. Full player/combat, skill tree and resources omitted. |
| [Gravity Wars](https://github.com/whyboris/Gravity-Wars/tree/e66964fc33a37cc8d2a824ec4fad255d38c0d54f), `gravity-wars` | Typed projectile/gravity modules, shot/collision/grace/turn behavior, previous-aim display | Check/build; 13 upstream comparison scenarios and 400 trajectory steps; collision/turn assertions; real LOVE eight-frame smoke. Full UI/input variants and all weapon modes omitted. |

LOVE smoke runs exercise actual update/draw callbacks and terminate deliberately.
They establish neither pixel parity nor human interaction quality. Headless
comparisons use deterministic state and, where needed, graphics/dependency stubs.
Individual reports list original files/functions and excluded harness mistakes.

## Phase 2: full application ports

These are complete application migrations with native types at stable boundaries.
Existing Lua engines and vendor libraries remain explicit interop dependencies;
this is not a claim that every dependency was rewritten into native classes.
Source inventories account for the original application paths, including paths
not reached by the deterministic test harnesses.

| Port | Full application scope | Runtime evidence and limits |
| --- | --- | --- |
| Sprit3r | 8 typed modules, 470 lines; 11 Lua dependency/parser modules and all four resources retained | Actual LOVE menu/help/example, PNG/VOX loading, reload polling, resize, text/key/wheel and viewer controls; 12 render cases plus camera/voxel oracle. Widget results are injected; OS drag and human visual parity remain unverified. |
| SNKRX | All 10 application modules plus 2 typed contract/interop modules, 12,151 lines; 46 Lua engine modules, all 310 assets and 69 engine/distribution files retained | Original and port produce identical actual-LOVE menu → shop purchases → arena → damage → 600-tick logs. Identical test-only Steam stubs are required on macOS because upstream ships a Windows DLL; real Steam remains unverified. |
| emoji-merge | Entire main/conf application, 140 typed parameter/field occurrences; 49 Anchor Lua modules, 99 assets and metadata retained | Actual LOVE input drop, real Box2D collision/merge, score, delayed spawn, save and retry UI pass. Original helper names and deferred-window config restored after fixes. Dynamic Anchor objects retain explicit gradual boundaries. |
| BYTEPATH | All 90 application modules, 13,373 upstream lines, 88 modules with native annotation edits; 225 vendor/resource files retained | Actual menu start → 10-second Stage → passives/SkillTree passes; final samples reach HP 100/difficulty 1 and 892 nodes/1,986 links. Original reruns vary in HP and object counts, so these samples are not deterministic invariants. 81/90 chunks have identical stripped bytecode; nine migration deltas are documented. LOVE 0.10.2 needs identical test-only adapters on installed 11.5. Exact incidental object counts are not parity claims. |
| Gravity Wars | All 7 application modules/39 functions plus a shared typed state module, 1,267 lines; all resources/licenses retained | Actual LOVE startup and 109 deterministic state/render comparisons cover inputs, five weapons, trajectories/collisions, turns and reset. Documented goto restriction requires equivalent terminal return flow. |

Every port retains upstream licenses and resource provenance. None of these
bounded scenarios establishes exhaustive gameplay, pixel parity, controller
coverage or long-session stability. Small source migrations and invalid-input
precondition differences are disclosed in the individual reports. Test-only
harnesses use disposable runtime copies and separate save identities; production
builds retain original service integrations.

## Confirmed fixes

### Class-body semantic coordinates

Sprit3r's imported `Layer[]` loop lost field hover/completion only inside a class.
Emoji-merge's local and imported class parameters lost completion, field hover,
and method signatures, while ordinary functions worked. BYTEPATH's indexed task
and numeric loop values also lost types.

Semantic name lookup now maps source positions to parser positions when locating
parameters and loops, then maps back before expression inference. The Vec2
optimizer now passes source positions at that same query boundary; the existing
single-evaluation emission regression caught and verifies that integration.
Core tests cover local/imported parameters and imported-record/numeric loops;
an LSP regression checks an unsaved partial parameter member access.

Rebuilt-server replay restores Sprit3r's inferred and annotated loop members,
emoji-merge's parameter members/signatures, and BYTEPATH's indexed task fields,
callback signature, and numeric loop hover. This does not resolve every nested
class context, captured local, or rename case listed below.

### Typed array removal

SNKRX exposed `table.remove(Hero[])` returning `any`; a separate `number[]`
reproduction accepted assignment to `boolean`. Shared standard-library facts
now yield the element type plus `nil`, consumed by editor queries, inferred
function returns, and checking. Tests cover guarded record completion, invalid
assignments, dynamic/custom calls, and bounded nested inference.

Independent review caught assignment-style monkey patches (`table.remove = ...`
and `table["remove"] = ...`) being incorrectly treated as built-ins. The final
fix retains their existing semantics; four mutation-focused regression tests
cover dot/bracket assignments and conservative unknown keys. Recursive imported `Self` remains independent of
this general array result fix.

### Imported type navigation

Gravity Wars could hover `Bullet` in `bullet: Bullet`, and Type Definition worked,
but ordinary Go to Definition returned null. Definition now uses the existing
project type resolver for named/renamed import bindings and type-reference uses.
Tests cover namespace uses and renamed re-exports, checking the original target
file and identifier span. No second import resolver was introduced.

### Full-port correctness fixes

- **Lexical helper bindings:** checking and emission respect local/parameter
  bindings named `image`, `shader`, `input`, `vec2`, etc. Previously a valid
  engine wrapper was silently replaced by a different runtime constructor.
  Normal calls use AST scope identities. Generated default guards are parsed
  once and use that same lowering with inherited outer bindings, preserving
  callback parameter shadows. Independent review added nested-default/runtime,
  callback-scope, later-parameter binding, callee-comment and source-provenance
  regressions, including native class methods. Generated class-header diagnostics
  retain the available enclosing-class location; exact parameter provenance
  remains a navigation/source-mapping gap.
- **Lua module call contracts:** unannotated Lua functions retain every parameter
  and vararg even when the AST has no native type specifiers. SUIT wrappers no
  longer falsely reject their arguments.
- **Class normalization:** type/default collection reads the class-lowered text
  corresponding to its AST positions, preserving class defaults and defaults in
  ordinary functions following a class. Check/build/runtime regression included.
- **Deferred LOVE windows:** `love.conf` accepts `t.window = nil`; default field
  completion remains discoverable and invalid numeric assignments are rejected.
  Initial window state and later nullable writes still need flow analysis;
  `false` is not broadened to arbitrary booleans.

### Measured analysis improvements

SNKRX exposed repeated receiver inference during builtin target discovery.
Indexing now reuses one semantic model and rejects impossible function labels
using the shared builtin catalog. The first change reduced the same frozen-input
check from approximately 373 seconds to 218.59 seconds (same migration diagnostics).
A later sample attributed 1303/1481 samples to rescanning module literals for each
identifier; the shared scanner now runs once per index.

BYTEPATH traces showed ECS diagnostics resolving every method receiver, followed
by ECS inference eagerly evaluating arguments of unrelated methods. Both paths
now check the supported ECS operations before inference. Tests preserve world
aliases and add/get/remove/query/system behavior and assert unrelated calls do
not invoke the expensive query. Final full-project timings are recorded below;
these fixes do not imply all large-project latency is solved.

With candidate `59408fcea8a0b94bb2d1b3a8d470e76f0b92cef59c007367b60896c721ad5df7`,
SNKRX's same corrected-source build fell from 196.25 seconds to 37.703 seconds
(about 81% for the combined later changes); full check takes 32.404 seconds.
Both commands succeed. BYTEPATH's previously stopped >325-second check now
reaches migration diagnostics in 5.81 seconds; this is not a successful-check
comparison until those migration diagnostics are corrected. The completed port
then checks successfully in 6.18 seconds and builds in 8.94 seconds, retaining
325 upstream/style warnings rather than disabling checker categories.

Emoji-merge's final unsaved indexed probe returns in 3.38 seconds rather than
exceeding 60 seconds, but still returns no members. Completed indexed access
returns no members in 2.17 seconds. Exact cursor: zero-based line 771, character
59, immediately after `value_to_emoji_data[a.value].`. Typed parameter hovers
remain correct. The responsiveness improvement does not close expression-member
completion or establish an acceptable typing-latency budget.

## Pending defects and precision gaps

| Priority | Finding and evidence | Next step / verification |
| --- | --- | --- |
| Medium | HUMP Camera explicit require reports a false missing `smooth.damped` export although the vendor module returns it. | Resolve nested re-exports and honor explicit dynamic boundaries consistently; preserve the original vendor implementation. |
| High | BYTEPATH assigns HUMP Camera globally in main; Stage receives injected Tua Camera and crashes on `Camera.smooth`. | Explicit `local Camera: any = _G.Camera` preserves the initialized upstream value (direct require hits the nested-export defect); project-wide binding provenance must suppress conflicting builtin module injection. Constructor lexical fixes do not solve this. |
| High | An explicit local annotation contaminates unrelated same-named locals in other functions (SNKRX ten-line repro). | Replace name-only annotation contracts with declaration identities; port uses unique local names temporarily. |
| Medium | Indexed expression completion is empty for Gravity Wars bullet entries and emoji rule entries; emoji unsaved probe exceeded 60 seconds amid concurrent work. | Trace shared receiver recovery on valid full-project edits; repeat isolated latency measurements. |
| Medium | LOVE image constructor rejects a valid File argument while actual LOVE accepts it. | Correct generated File/FileData overloads and test checker/editor/runtime agreement. |
| Medium | Optional/default anonymous callback parameters fail parsing; named function workaround works. | Extend parser-backed anonymous metadata and useful recovery diagnostics. |
| Medium | Side-effect-required global function definitions are missing across files in Gravity Wars. | Define provenance for globals without inventing returned module exports. |
| Medium | Assert calls do not narrow nullable locals; LOVE initial enabled-window state is not tracked. | Add sound control-flow facts; retain nullable writable configuration contracts. |
| Medium | Editor/asset indexing still needs binding-aware helper conformance coverage. | Test the same lexical identities across constructor checks, emission, completion and metadata. |
| High | Recursive imported `Self` loses members in SNKRX follower/parent paths. | Preserve nominal identity through recursive expansion; retain cycle guards. Existing roadmap item, reinforced by this port. |
| Medium | Native class parameter rename is null in emoji-merge's `minimal.tua`, including declaration positions. | Preserve parameter-header identities/provenance and use shared scope resolution; test declaration/use rename, references, shadowing, and source edit ranges. |
| Medium | Captured narrowed `item` inside emoji-merge's timer callback still has empty member completion after the coordinate fix. | Isolate capture/narrowing and mutation behavior; compare ordinary and native-class callbacks and unsaved edits. |
| Medium | BYTEPATH's full Director `self.timer:after` still lacks completion/signature although ordinary and minimal-class controls work. | Minimize surrounding context and trace shared receiver resolution. Do not generalize this to all imported fields. |
| Medium | `table.insert` accepts strings into `Hero[]` and booleans into `number[]` (SNKRX independent repro). | Derive append/indexed insertion contracts from the array; cover mismatch ranges, nested diagnostics, overrides, and dynamic `any`. |
| Medium | `table.sort(number[], function(a,b) ...)` gives comparator parameters `any`, even outside classes (BYTEPATH). | Contextual argument-dependent callback types; verify number/record arrays, checking and member completion. |
| Medium, capability gap | Guarded `player.lastAngle` remains nullable in Gravity Wars; capturing it into a local and guarding that local works. | Define stable member-path narrowing with mutation/call invalidation. Current documented guarantee is for locals; do not claim a regression of that contract. |

Existing fluent-expression, event-result, overload-selection and large-project
performance work in [ROADMAP.md](../ROADMAP.md) remains open. The completed port scenarios
are not evidence that those issues are fixed.

## Migration guidance and design proposals

- **Constructor defaults:** Sprit3r expected an init-computed spacing to survive
  a field default. Tua explicitly specifies init-before-default ordering, so
  this was documented language friction, not a compiler bug. Use an uninitialized
  typed field and assign it in init. Consider a targeted overwrite diagnostic;
  reversing order requires a compatibility decision and migration plan.
- **Composition:** SNKRX/Anchor-style mixins need explicit services/adapters under
  native single inheritance. Provide migration recipes before adding protocols,
  mixins, or multiple inheritance. A timer declaration/adapter workflow is useful
  in two ports; this does not yet justify a new timer language domain.
- **Physics ownership and configuration:** investigate typed collision/contact
  payloads and checked data-driven optional property schemas against a larger
  port before promising built-in support.
- **Numeric units:** Gravity Wars suggests angle/force/dimension distinctions,
  but units or nominal numeric aliases remain a low-confidence design idea.
- **Legacy migration:** BYTEPATH loads every directory entry as Lua; use supported
  `emit_maps = false` for this loader. Keep old LOVE compatibility harnesses and
  native service substitutions explicit. Installed-editor interactions, visual
  parity and long-session latency remain separate acceptance work.

## Verification and handoff

Verification logs are under `target/tua-port-audit`:

- `RUST_MIN_STACK=16777216 RUST_TEST_THREADS=4 make verify` passes in
  `verify-release-ready.log`: workspace tests (including 1,577 core and 372 LSP
  tests), Clippy with warnings denied, extension tests/activation, metadata and
  whitespace checks. Existing ignored tests retain their repository status.
- Real-server `lsp-features-smoke`, feature example check/build (33 source files),
  LOVE-minimal build, docs build and screenshot metadata checks pass in
  `smokes-final.log`.
- The fresh final reviewer found a later-parameter binding error in generated
  defaults. The fix and runtime regression cover direct defaults, callback
  captures and native class methods. No additional concrete blocker was found
  in the ECS or indexing optimizations.
- Earlier full-gate failures caught string-sugar whitespace snapshots and
  recovery fixtures assuming a nonnullable window. These are corrected; nullable
  initial-state/member-path inference remains explicitly open. An initial
  package attempt timed out probing a mock server under concurrent load and
  was not treated as a passing package check.
- Five complete LOVE workflow runs and their individual limits are documented
  above and in the port reports. BYTEPATH's repeated upstream runs vary: no
  deterministic full-state or pixel-parity claim is made.

Installed-editor acceptance, real Steam integrations, exhaustive playthroughs,
and exact legacy-LOVE visual parity remain unverified. No editor UI was changed
in this audit. Screenshot metadata checks validate the existing October 25
0.4.1 gallery; they are not a fresh 0.5.1 visual review.

The final VSIX, archive integrity results, compiler/output comparisons and
checksums are recorded with the handoff artifacts under `target/tua-port-audit`.

Installable candidate: `target/tua-port-audit/tua-0.5.1-port-audit-macos-arm64.vsix`
(version 0.5.1, macOS arm64). ZIP integrity and bundled/release compiler identity
checks pass; the release binary also passes the feature LSP smoke.

VSIX SHA-256:
`aba25dfab0d7489e4bb8eb37b0e88f68cf7c75a559f93d9011c1e83a5208b1ed`.
Compiler SHA-256:
`dcc9926a74856c2aaeacab5e2c366f7177c7ed18eacfc50f2db436875792d951`.
Use VS Code's **Extensions: Install from VSIX...**. To replace an existing copy
of the same version, use `code --install-extension <path-to-vsix> --force`.

All five projects rebuild successfully with this exact packaged compiler.
Every emitted Lua file is byte-identical to its previously runtime-tested build;
`packaged-port-builds.json` records this comparison. Each project carries an
`AUDIT-CANDIDATE.json` with the final binary identity, separate from historical
performance measurements. Full source/build/resource/report archives and their
SHA-256 manifests are in `target/tua-port-audit/full-ports`.
