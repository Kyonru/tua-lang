# Roadmap implementation — November 2026

This pass implements six concrete roadmap improvements on top of the five-game
port audit. It is not a claim that every remaining roadmap item is complete.
Completed work has been removed from `ROADMAP.md`; current behavior is documented
in the language/tooling guides and changelog.

## Implemented

| Commit | Improvement | Evidence |
| --- | --- | --- |
| `c2c64dd` | Native and LuaDoc local contracts follow declaration identities, including shadowing, cross-file return inference, and trailing-edit completion. | Core checker, return inference, hover and completion regressions; full core/LSP suites. |
| `9579a3f` | Nested table functions survive direct and callable module re-exports. Explicit local `any` stays dynamic. | Direct/callable export tests, project hover/completion/checking test, saved HUMP reproduction checks. |
| `74c2bd9` | Project-assigned `Camera` and `tween` globals prevent built-in injection across files. | Lua/Tua ownership tests, local/import controls, unsaved edits and immutable older snapshots. |
| `7244dd3` | Empty calls select zero-argument overloads; entered arguments select an overload with room for them. | LOVE `Text:getWidth`, user overloads, comments, strings, nested expressions and all signature-help tests. |
| `ba8cec3` | Typed arrays constrain both forms of `table.insert`. | Valid/invalid values, index and argument count, record elements, dynamic values, shadowed roots and library replacements. |
| `65cb4ca` | Unresolved initializers retain dynamic lexical bindings and mask unrelated assignment contracts. | Full-port replay, nearest-binding/LuaDoc/branch controls, 1,588 passing core tests. |
| `d542485` | Document incremental singleton/mixin migration to explicit state, services, classes and adapters. | Two-module guide example checks/builds; existing runnable polymorphism example; docs build. |

The roadmap's obsolete instruction to ship 0.4 was also replaced with a
version-neutral next-release qualification priority; the current checkout is
0.5.1. This pass does not bump, tag or publish a release.

The full-port replay exposed a remaining unresolved-initializer case: a local
whose initializer could not be inferred did not enter the checker scope. The
follow-up records a dynamic binding and a declaration-owned assignment contract,
preventing fallback to an unrelated annotation while retaining typed outer
contracts inside narrowed branches. It also clears the receiver-count errors
seen in Sprit3r's larger returned native class instance. These findings were
fixed before accepting the final candidate; initial failing replay logs remain
as evidence.

## Boundaries

- Cross-file module ownership uses cached project scope declarations. It does
  not load modules, impose initialization order, or invent a precise global
  type. Explicit local imports retain their inferred contracts. Computed `_G`
  writes are outside this static ownership model.
- `table.insert` specializes typed arrays; arbitrary dictionary shapes remain
  dynamic. Signature-help contextual array specialization and `table.sort`
  comparator inference are not claimed by the insertion checker fix.
- The overload change corrects argument counting/selection. General fluent
  receiver recovery remains on the roadmap.
- The migration guide preserves init-before-default ordering. It does not
  introduce protocols, generic classes, a scheduler, or an ordering change.

## Remaining work

These are implementation/evidence tasks, not all requests for user input:

- Shared indexed/fluent receiver resolution, imported `Self`, cross-file event
  call results, captured class contexts, class parameter navigation and unclosed
  class recovery.
- Anonymous callback defaults, assertion and member-path narrowing, generated
  LOVE File/FileData overloads, binding-aware asset metadata, configuration flow,
  and contextual `table.sort` comparators.
- Shared documentation tag metadata, parsed-signature comment generation,
  parameter documentation maintenance and annotation navigation.
- Cold typing/require/format/hot-reload traces and installed-editor validation;
  graph/asset workflow refinements and representative object-pool benchmarks.

Protocol/generic design and changing class initialization order require a
compatibility design before implementation. Signing and additional native release
targets require credentials/infrastructure. The unspecified reference request
still needs an exact symbol/action/destination. These distinctions remain in the
roadmap rather than treating all unfinished engineering as blocked on the user.

## Verification and candidate

Evidence and command logs are in `target/roadmap-implementation/`. Full-project
replays use temporary copies of BYTEPATH and SNKRX with their scope/Camera
workarounds removed; the other three preserved ports are checked and rebuilt.
Initial debug-build replay times are workload evidence, not comparisons with
release-mode benchmarks. The final replay uses the current release compiler.
Native visual/gameplay parity and installed-editor qualification are separate from check/build success.

`RUST_MIN_STACK=16777216 RUST_TEST_THREADS=4 make verify` passed, including
1,588 core tests, 372 LSP tests, CLI/integration/gameplay tests, Clippy,
metadata checks and 155 extension tests. The suite retains its existing ignored
shader-host tests and manual corpus-latency probe. `make lsp-features-smoke extension-screenshots-check
docs-build` also passed. The screenshot check validates the existing 0.4.1
capture artifacts; this pass changed no extension panels and made no fresh
visual-capture claim. `make ci-check`, signing/publication and installed-editor
upgrade/rollback qualification were not run.

All five full ports passed `check` and `build` with the final release compiler:

| Project | Check | Build |
| --- | ---: | ---: |
| sprit3r | 0.24s | 0.59s |
| snkrx | 33.24s | 35.06s |
| emoji-merge | 11.48s | 22.82s |
| bytepath | 6.08s | 8.58s |
| gravity-wars | 0.12s | 0.33s |

These timings were collected during local verification, not in an isolated
benchmark. Existing project lint warnings remain visible. BYTEPATH has both
`local Camera: any = _G.Camera` workarounds removed; SNKRX restores the
`follow_pose` and `mouse_slot` workaround names to `p`. Sources/outputs are in
`/private/tmp/tua-roadmap-replay/`; the three other ports use their preserved
`/private/tmp/tua-port-audit/` workspaces. No fresh full-game visual parity run
is claimed.

Final compiler SHA-256: `9261657fe0acd9b9a95001f0230af303c2af1a18b3c01f2d56c138d49581c140`.

Installable local candidate (macOS Apple silicon):
`target/roadmap-implementation/tua-0.5.1-roadmap-macos-arm64.vsix`.
VSIX SHA-256: `372b26ac283fc9ad489b39e3c8446b0c19ee07cfb396cc54071be315b2d32082`. Archive integrity passed; the bundled server
hash matches the final replay compiler above. This is an unpublished 0.5.1
test candidate, not a newly tagged release.
