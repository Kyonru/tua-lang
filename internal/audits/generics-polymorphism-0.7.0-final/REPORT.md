# Tua 0.7.0 Generics and Protocols Final Audit

Date: 2026-11-10
Audited base: `06c1065b940622eee2f179b4c7752ce496010b8c`
Final implementation revision: `dc3413d1` plus the AUDIT-019 concrete generic-
parent specialization; installed-editor regression-test
revision: `f209b882`; latest protocol dependency-rename test revision:
`0dd22027`; final multi-workspace protocol-isolation test revision:
`24383678`; latest generic multi-workspace isolation test revision:
`a092ce8e`; generic-class source-map regression revision: `25320824`. The
final implementation revisions add imported inherited protocol-requirement
links to override diagnostics, safely diagnose malformed nested
protocol/class declaration text, and resolve generic member definitions
across modules. The November 10 boundary fixes are separate commits
`17b6b412`, `afd004ba`, and `7efaf29b`. This report records verification after
protocol-header, diagnostic, documentation, cycle, dependency-deletion,
dependency-rename, inherited-requirement, multi-workspace isolation, and
malformed-declaration fuzz coverage.

**Current status note (November 10):** The authoritative phase and scope
reconciliation is at the end of this report. It resolves the old cyclic
protocol finding, records current host-specific package verification, and adds
one high-scale LSP performance finding. Earlier issue counts and open labels
are historical snapshots.
Scope: `internal/generics-polymorphism-0.7.0.md`, protocol contracts, native generics, type inference, editor surfaces, Lua erasure, and the locally shareable VS Code extension.  
Working tree before audit: clean. No unrelated project changes were present.

## Executive summary

The documented 0.7.0 slice has strong compiler, project-analysis, LSP, runtime, and installed-editor coverage. The audit fixed the listed parser, inference, documentation, and diagnostic issues; this continuation also fixes inherited override links when a local base class implements an imported protocol, malformed declaration text that could panic during lowering, missing Go to Definition for imported generic class members, and nested class/protocol text being erased as a top-level declaration. The audit added regressions for these behaviors, generic multi-return checker/runtime coverage, and installed VS Code protocol implementation navigation, generic binder references, rename, and unsaved dependency-edit coverage.

At the prior checkpoint, the protocol/generic implementation was suitable for **manual beta testing on macOS arm64**. That packaged VSIX (40 files, 8.68 MB) passed the isolated VS Code real-server suite with its bundled compiler. The final focused audit and current artifact are recorded in the November 10 addendum below. General 0.7.0 release qualification remains open for Linux, Windows, Intel Mac, LuaJIT/LOVE, source-map/hot-reload, visual screenshot review, and long-session checks. A local class hierarchy can inherit an imported protocol's requirements; cross-module class inheritance itself remains outside the parser/lowering contract.

### Final focused audit update — November 10, 2026

Three additional defects were confirmed and fixed in this continuation: generic
alias expansion limits could hide the type of a known field; protocol
requirements accepted missing or empty parameter annotations; and comments in
generic function binders disappeared when those binders were erased to Lua.
The fixes preserve the existing recursive-alias hover behavior and `Self`
member inference. The exact regenerated 0.7.0 VSIX contains 40 files (8.71 MB),
has SHA-256
`8de57325a50b0adbbbe2045f1b83fe556dfdbc833a3b1d39c96a89a1db966f46`, and passed
the isolated installed-VSIX real-server test on VS Code 1.142.0/macOS arm64.
The workspace suites and editor tests pass; the combined `make verify` command
itself stopped when the sandbox denied the extension runtime-inspector's
loopback bind, and the extension suite passed 180/180 when rerun with local
loopback access. The benchmark smoke likewise passed with local process
sampling enabled.

AUDIT-028 records an optimization for repeated typed-domain resolution while
completing wide generic records. A later independent replay found that the
original timeout did not include a valid completion response and came from the
probe setup; it is not evidence of a confirmed 60-second editor regression.
The current sequential stdio probe returns 11 expected matches in about 54 ms.

The latest artifact is suitable for manual beta testing on this host. It is not
qualified for a general release: cross-platform/runtime qualification, visual
review, long-session memory behavior, and second-client testing remain open.

Repeated local profile measurements (five cold samples, twenty warm and contention samples) on the 1,000-line fixture show cold completion p95 251.10 ms, warm completion p95 6.24 ms, cold hover p95 266.65 ms, warm hover p95 20.03 ms, and completion p95 under contention 153.03 ms. The 100-module project reports cold completion p95 116.04 ms, warm completion 0.40 ms, and warm hover 1.66 ms. These have no before/after baseline; cold latency and large-project contention remain performance opportunities.

### Final audit continuation — November 10, 2026

One additional low-severity syntax/trivia defect was found and fixed: a line
comment after a supported generic protocol bound (for example,
`<T: Runnable -- note\n>`) was included in the bound name and rejected as an
unsupported constraint. Normalization also erased comment characters while
removing the bound. The parser now finds generic-list delimiters outside
comments and preserves comment trivia while erasing only the bound text. Parser
and emitted-Lua regressions cover a prefixed source file and comment contents
that include `,` and `>`.

The earlier 4,200-field completion timeout was rechecked with a sequential,
framed stdio LSP probe and a valid cursor position. The current server returned
hover `(field) f4199: number` and 11 completion matches in about 54 ms. The
earlier timeout did not yield a valid completion response and is treated as a
probe setup failure; it is not a current unresolved performance finding.

On the final source, `CARGO_INCREMENTAL=0 RUST_MIN_STACK=33554432
RUST_TEST_THREADS=1 make verify` passed: 1,891 core tests passed (1 ignored),
431 LSP tests, 92 CLI library tests (2 ignored), 137 CLI integration tests,
3 gameplay tests, and 180 extension tests. Clippy, version/config/schema
checks, language evaluation, JSON validation, and diff checks passed. This run
had local loopback access for the inspector test. `make extension-package`
rebuilt the macOS arm64 0.7.0 VSIX; npm audit found zero vulnerabilities. The
exact package passed the isolated installed VS Code real-server suite on VS
Code 1.142.0, and `unzip -tq` passed. SHA-256:
`be629e0c43098727b0091bd192737557e9f05ab7f52c751636736335055c4ee7`.

This updates local beta-test readiness only. Linux, Windows, Intel Mac,
LOVE/LuaJIT runtime, visual, long-session, and second-client qualification
remain open as recorded above.

## Final verification addendum — November 10, 2026

The earlier checksum and suite counts in this report are historical snapshots.
After that snapshot, the recursive protocol-signature defect was independently
reproduced and fixed, then same-file and cross-file identity, conformance, and
editor-surface regressions were added. Further tests cover deletion of one node
from a cyclic import graph through both `AnalysisHost` and LSP watched-file /
pull-diagnostics flow, editor recovery at every character prefix of a nested
record/union protocol fixture, and imported inherited-override diagnostic
ranges after Unicode-prefixed source lines. Further regressions confirm that
one inherited override diagnostic links to requirements in separate imported
modules, protocol obligations survive an intermediate subclass, and creating
then editing a missing type-only protocol dependency refreshes diagnostics.

At revision `0cbb06a0`, `CARGO_INCREMENTAL=0 RUST_MIN_STACK=33554432 make verify`
completed successfully on macOS arm64. It included 1,874 core tests passed (1
ignored), 425 LSP tests passed, 92 CLI library tests passed (2 ignored), 136
CLI integration tests passed, 3 gameplay runtime tests, 180 VS Code extension
tests, Clippy, schema/reference/version checks, and `git diff --check`. The
core test suite also passed independently with the recorded stack setting.
The VS Code inspector tests require loopback access; they passed in this
full-verification run with host loopback access. A transient CLI
temporary-workspace failure earlier in the audit also passed in isolation and
in the successful full run.

The exact `editors/vscode/tua-0.7.0.vsix` artifact was rebuilt after the
recursive-protocol editor regression was added, passes archive integrity
validation, and has SHA-256
`65be27f16e0139dceaeb50bd8260063d16c432b811256ca267cd33268daf2655`. It
passed the installed real-server replay in an isolated VS Code profile on this
macOS arm64 host, including the cyclic protocol hover/completion scenario. The
only changes since the prior product build are this test coverage and audit
notes. The artifact is ready for manual beta testing on this platform, not
general release qualification.

Still pending: LOVE/LuaJIT runtime and visual review; live-game source-map and
hot-reload qualification; Linux, Windows, and Intel Mac checks; repeated
release-host performance with a baseline and long-session retained-memory
measurement; and testing in a second LSP client. These are environment or
qualification gaps, not evidence of a known implementation defect. Cross-module
class inheritance, generic methods/protocols, compound bounds, and explicit
call-site generic arguments remain documented feature limits.

## Re-audit continuation — November 10, 2026

Two additional correctness defects were found by independently replaying
earlier audit reports against the current checkout and were fixed in separate
commits. A same-file and cross-file recursive-return shape had not been covered
by the earlier cyclic-signature regression; the rebuilt VSIX now checks that
shape as well. A separate docs commit explains why explicit call-site generic
syntax remains unsupported: `f < T > (x)` overlaps valid Lua comparison syntax.

The re-audit fixes are `e4e573ea` (recursive protocol return identity),
`d7c56d50` (discard a stale file-only generic-bound diagnostic when project
checking is authoritative), and `580307ea` (remove a redundant local import
from the re-export regression fixture). The installed-editor regression is
`cff5e939`. Documentation clarification is `754688f4`.

On the absolute-path CLI and actual LSP fixture, a class explicitly
implementing imported protocols `A` and `B` now returns the declared cyclic
protocol type through `A.next(): B` without a structural/`any` mismatch. A
generic function bounded by a re-exported protocol now accepts the imported
conforming `Actor`, while preserving TL1015 for a same-shaped `Lookalike` that
does not explicitly conform.

The full `CARGO_INCREMENTAL=0 RUST_MIN_STACK=33554432 RUST_TEST_THREADS=1 make
verify` suite passed at the re-audit revision: 1,882 core tests passed (1
ignored), 426 LSP tests, 92 CLI library tests (2 ignored), 137 CLI integration
tests, 3 gameplay tests, and 180 extension tests. Clippy, schema/version
checks, language-evaluation tests, JSON validation, and diff checks passed. An
initial parallel run hit the existing helper-process timeout test under load;
that test passed alone and the complete serial run passed.

The rebuilt VSIX was installed into an isolated VS Code 1.142.0/macOS arm64
profile and passed the real-server suite. It verifies recursive return hover
as `local target: B`, completion of `B.previous`, recursive argument hover and
completion, clean diagnostics, imported generic hovers/signatures/navigation,
and the protocol stub action. Package validation and the current SHA-256 are
recorded in the verification log below. LOVE/LuaJIT and other-platform/runtime
qualification remain open.

The `source_pipeline` nightly fuzz target completed 21,595 executions in 31
seconds without a crash (observed peak RSS about 600 MiB). The
`protocol_and_paths` target completed 711,266 executions in 31 seconds without
a crash (observed peak RSS about 533 MiB). Its corpus contained 134 seed files;
these bounded runs do not establish a general resource bound.

The core inherited-override regression now places an intermediate class
between the protocol-conforming base and the incompatible leaf override. The
focused `inherited_protocol_conformance_survives_overrides_and_reports_the_method_span`
test passes and still asserts the leaf method's primary span and both related
protocol requirements. This test-only extension does not change product code
or the packaged VSIX.

## Audit findings

### AUDIT-001 — Generic multi-return behavior lacked a persistent acceptance test

- **Category / severity:** Test coverage; Medium.
- **Status:** Fixed with regression tests in commit `10496682`.
- **Evidence:** The plan explicitly requires multiple generic return positions. A small CLI reproduction using `split<T, U>(left: T, right: U): (T, U)` checked successfully, but no test asserted both slot-specific inferred types and emitted-Lua runtime values together.
- **Change:** Added `generic_function_multiple_return_slots_infer_independently` in `crates/tua-core/src/check/tests.rs` and `generic_multiple_return_slots_preserve_lua_runtime_values` in `crates/tua-cli/tests/cli.rs`.
- **Verification:** Both focused tests pass. The CLI case builds the source, checks that generic syntax is erased and call arguments remain in place, and runs the emitted Lua with Lua 5.4.8.
- **Root fix direction:** None required; this was a regression-coverage gap.

### AUDIT-002 — Installed VS Code did not exercise protocol implementation navigation or rename

- **Category / severity:** Editor integration coverage; Medium.
- **Status:** Fixed with installed-editor regression coverage in commit `a71cf3e1`.
- **Evidence:** The core and in-process LSP tests already covered protocol navigation and rename, but the real-server VS Code suite stopped at protocol definition and generic selected definitions.
- **Change:** The real-server suite invokes VS Code's implementation provider from a protocol-typed call and renames a requirement from its declaration, asserting the implementation file is included.
- **Verification:** The development-extension real-server run and the exact packaged-VSIX run both pass on VS Code 1.142.0/macOS arm64. The test deliberately places the cursor inside the method identifier; a cursor at the end-exclusive identifier boundary correctly caused VS Code to reject rename during initial test development.
- **Root fix direction:** None required; the behavior works and is now exercised at the installed-editor boundary.

### AUDIT-003 — Growing recursive aliases expanded into verbose nested-type diagnostics

- **Category / severity:** Type display and diagnostics; Medium.
- **Status:** Fixed in `c25c5db0`.
- **Reproduction:**

  ```tua
  type Grow<T> = { next: Grow<T[]> }
  local function read(node: Grow<number>): number
    return node.next.next
  end
  ```

  Run `tua check` in a project containing this source. Before the fix, the mismatch rendered dozens of expanded records. After the fix, the actual error is `expected number, found { next: Grow<number[][][]> }` (less than 200 characters), retaining the incompatible value's shape without widening to `any`.
- **Root cause:** Both `GenericAliasExpansionContext.resolve` and `ProjectTypeDatabase::resolve_type_inner` stopped only when the full `(declaration, arguments)` pair recurred. Since `Grow<T[]>` changes arguments each time, expansion reached its depth bound before preserving a useful named edge.
- **Fix:** Stop expansion when the same alias declaration reappears on the current expansion path, even with changed arguments. Preserve that application and its substituted arguments. Other distinct alias chains remain protected by existing work/depth guards.
- **Verification:** `changing_argument_generic_alias_recursion_preserves_its_first_named_edge`, `changing_argument_generic_alias_recursion_preserves_its_first_project_edge`, `growing_recursive_generic_aliases_keep_a_symbolic_member_type`, and `growing_recursive_generic_alias_keeps_editor_types_finite_and_usable` pass. The last test checks imported cross-file completion and hover.
- **Regression value:** Checker reports remain accurate and compact; project and editor surfaces preserve the same changed generic arguments.

### AUDIT-004 — Cross-module inherited conformance remains unavailable

- **Category / severity:** Capability gap; Medium, known scope limit.
- **Status:** Open by design/architecture boundary.
- **Reproduction:** Create `src/base.tua`:

  ```tua
  export open class Base {
    read(): number { return 1 }
  }
  return { Base = Base }
  ```

  Then create `src/child.tua`:

  ```tua
  local BaseModule = import("./base")
  local Base = BaseModule.Base
  final class Child extends Base {
    override read(): number { return super.read() + 1 }
  }
  ```

  Run `tua check` from the project root. The importer reports
  ``unknown parent class `Base` `` at the `extends` clause. A separate
  `BaseModule.Base.new():read()` expression type-checks, confirming that this
  is the class-parent relation path rather than generic module export
  visibility. This is an unsupported capability, not an incorrect result for
  a documented feature.
- **Evidence:** G3 supports local generic inheritance and imported generic
  instance metadata, but cross-module class inheritance is not currently in
  the parser/lowering contract. Therefore inherited protocol conformance
  cannot be qualified through an imported base class.
- **Recommended improvement:** Design cross-module class extension as a separate language/module feature. It needs constructor/init behavior, nominal identity, invalidation, emitted `super` behavior, and editor navigation defined before implementation. Do not claim G3 fully supports cross-module inherited conformance.

### AUDIT-005 — Cold editor requests scale noticeably with a larger source file

- **Category / severity:** Performance follow-up; Medium, measured but not compared to baseline.
- **Status:** Open; no regression claim.
- **Evidence:** A five-cold/twenty-warm repeated profile is recorded in the executive summary. `single-1000` warm hover p95 remains about 20 ms; contention completion p95 is about 153 ms. There is no baseline comparison or long-session memory data.
- **Recommended improvement:** Capture repeated cold and warm p50/p95 on the release host with project sizes, toolchain, and memory details. Investigate cold snapshot/index startup and the warm hover path if a repeatable budget is exceeded.

### AUDIT-006 — Runtime and platform release qualification is incomplete

- **Category / severity:** Release gate; Medium.
- **Status:** Pending external/runtime environments.
- **Evidence:** This host is macOS arm64 with VS Code 1.142.0 and Lua 5.4.8. `lua` is available; LOVE and LuaJIT are not. No Linux, Windows, or Intel Mac host was available.
- **Recommended improvement:** Run the release matrix on Linux, Windows, Intel Mac, and Apple Silicon; run supported LuaJIT and LOVE runtime tests; exercise source maps and hot reload with the game runtime; perform a long-session memory check. The macOS VSIX result must not be presented as universal release qualification.

### AUDIT-007 — Protocol keywords were misclassified in ordinary Lua code

- **Category / severity:** Parser compatibility; Medium.
- **Status:** Fixed in `2433056f` (`fix: keep protocol keywords contextual`).
- **Reproduction:**

  ```tua
  local protocol = "existing value"
  local implements = protocol
  return { protocol = protocol, implements = implements }
  ```

  `parse_tua_source` previously failed on the `implements` binding. After the
  grammar classifier was narrowed, the protocol declaration scanner then
  mistook the `protocol` value use for a declaration and consumed the following
  return table as a protocol body.
- **Expected:** These remain legal Lua identifiers. The same names also work
  alongside Tua type syntax, and real `protocol Name { ... }` declarations
  still parse.
- **Root cause:** The parser selected its Tua grammar merely because it
  encountered the words, and the protocol scanner searched for `protocol`
  without requiring declaration position. A value use could therefore alter
  grammar selection or absorb later source as a declaration.
- **Fix:** Grammar selection now requires protocol declaration shape at a
  statement position; the class keyword remains the trigger for class headers.
  Protocol declaration scanning also requires statement position. Identifier
  behavior is covered in ordinary and typed source, including emitted Lua.
- **Verification:**
  `protocol_and_implements_remain_usable_as_lua_identifiers` and the complete
  protocol-focused core test selection pass. The regression also confirms a
  real protocol declaration still reaches its normal parser path.
- **Related plan requirements:** P0 contextual keyword compatibility and P1
  declaration identity/recovery.

### AUDIT-008 — Protocol arity failures lacked actionable detail

- **Category / severity:** Diagnostic usability; Low.
- **Status:** Fixed in `22393904` (`fix: explain protocol arity mismatches`).
- **Reproduction:** A method implementing `find(index?: number)` with
  `find(index: number)` correctly failed, but the diagnostic showed only two
  full signatures. Missing protocol varargs and a mismatched number of return
  values were similarly reported without naming the arity rule.
- **Expected:** State which side requires or permits the argument count,
  whether the protocol requires variadic arguments, or how the declared return
  counts differ.
- **Root cause:** The shared assignability result records
  `FunctionParameterCount`, `FunctionVararg`, and `FunctionReturnCount`, but
  the protocol diagnostic formatter only rendered failures with a parameter
  or return path.
- **Fix:** Format those structured failure kinds in the protocol conformance
  diagnostic with singular/plural wording.
- **Verification:**
  `protocol_compatibility_respects_optional_varargs_and_each_return_slot`
  covers valid optional/variadic/multi-return conformance and rejects an
  implementation-required optional argument, missing varargs, a wrong second
  return type, and wrong return count.

### AUDIT-009 — Editor recovery discarded semantics for an unfinished protocol

- **Category / severity:** Editor editing workflow; Medium.
- **Status:** Fixed in `12f2a2fa` (`fix: recover editor semantics in incomplete protocols`).
- **Reproduction:** Open a Tua file containing a local followed by an
  unclosed protocol declaration, then request hover on the local. The strict
  parser correctly reports the missing protocol brace, but degraded editor
  parsing previously had a class-body recovery path only and could not build
  current-buffer semantics.
- **Expected:** Preserve useful declarations and the partial protocol contract
  for editor queries while continuing to report the missing brace. Build/check
  must still reject the original source.
- **Root cause:** Current-buffer recovery synthesized missing class braces but
  did not apply equivalent recovery to protocol bodies. Adding it also exposed
  that project indexes preferred recovered syntax over a dependency's last-good
  contract, so an in-progress protocol edit could replace its last valid method
  set in dependent files.
- **Fix:** A shared editor-only declaration recovery closes unmatched class
  and protocol bodies for degraded analysis, completion, and signature help.
  Project indexes now prefer last-good parsed/type facts over degraded syntax
  for invalid dependencies. Normal parsing still sees the unmodified buffer
  and retains the targeted unclosed-protocol syntax diagnostic.
- **Verification:**
  `editor_recovery_closes_an_unfinished_protocol_without_losing_its_contract`
  verifies contract/AST preservation and strict-parser rejection;
  `unclosed_protocols_keep_current_buffer_semantics_available` verifies
  protocol member completion, current-buffer hover, and the remaining
  diagnostic.

### AUDIT-010 — Cross-file conformance diagnostics did not identify the contract location

- **Category / severity:** Diagnostic navigation and usability; Medium.
- **Status:** Fixed in `4f6f6842` (`fix: link protocol diagnostics to requirements`).
- **Reproduction:** Put a protocol in `contracts.tua`, import it from `actors.tua`, then declare an implementing class with a missing method or an incompatible method signature. The project checker correctly reported the implementation error in `actors.tua`, but there was no related location pointing to the protocol member in `contracts.tua`.
- **Expected:** Keep the actionable primary diagnostic at the class implementation and attach standard LSP `relatedInformation` for the required protocol method, including its source URI and UTF-16 range.
- **Root cause:** `Diagnostic` represented only its primary span, and project protocol indexes retained method types without their declaration provenance. Consequently the LSP serializer had no source declaration to report.
- **Fix:** Added a shared diagnostic related-information record and retained canonical method definitions in local and imported protocol indexes. Missing-method and incompatible-signature diagnostics now carry the requirement origin; the LSP serializer resolves it through the analysis snapshot and emits standard related information.
- **Verification:** Core `imported_protocol_conformance_is_checked_in_the_implementing_module` asserts requirement path/span for both missing and incompatible members. LSP `imported_protocol_conformance_diagnostics_are_published_in_implementing_file` asserts the published URI, range, and message. `related_diagnostic_ranges_use_utf16_columns` checks non-BMP source positioning. Full `tua-core` and `tua-lsp` suites pass.
- **Remaining edge:** The fast single-file diagnostic path does not carry cross-file related information; full project diagnostic refresh does. Broader local/inherited override related-range combinations remain covered only partially and are tracked in P2 follow-up.

### AUDIT-011 — Protocol headers rejected comments and editor recovery searched comments for braces

- **Category / severity:** Parser and editing workflow; Medium.
- **Status:** Fixed in `39d2dc4c` (`fix: parse commented protocol headers`).
- **Reproduction:**

  ```tua
  export -- public contract
  protocol -- behavior name
  Renderable -- contract body
  {
    render(): nil
  }
  ```

  The declaration was missed because the scanner skipped whitespace but not Lua comments between header tokens. It then produced unrelated parser diagnostics. With the same header left unfinished, editor recovery also searched the raw source for the first `{`; a brace in any header comment made recovery fail before reaching the protocol body.
- **Expected:** Comments act as trivia between declaration tokens. A complete declaration parses and erases normally; an incomplete one remains invalid to build/check but exposes partial semantics to editor queries, with comment text excluded from delimiter matching.
- **Root cause:** Header scanning treated comments inconsistently, and recovery rediscovered the body brace with an unlexed substring search.
- **Fix:** Consolidated protocol header parsing into `protocol_header_at`, which skips whitespace and line/long comments and retains the actual body-brace offset. The unclosed-declaration recovery path now uses that parsed offset.
- **Verification:** `protocol_headers_allow_comments_between_declaration_tokens` asserts exported identity and method parsing, Lua erasure, strict rejection of an unfinished declaration, and recovery when comments contain braces. The protocol-focused core selection passes (47 tests), full core passes (1,864 passed, 1 ignored), full LSP passes (409), and Clippy/docs build pass.

### AUDIT-012 — Language guide contradicted the implemented generic feature set

- **Category / severity:** Documentation accuracy; Low.
- **Status:** Fixed in `39d2dc4c` (`fix: parse commented protocol headers`).
- **Evidence:** The Classes section of `docs/language.md` said generic type parameters such as `<T>` were unsupported, although the same guide and the implementation support generic aliases, free functions, and classes.
- **Expected:** State supported generic forms and link to their limits, distinguishing deferred generic methods, generic protocols, and explicit call-site type arguments.
- **Root cause:** A paragraph predating generic implementation remained after the newer generic-language sections were added.
- **Fix:** Replaced the stale statement with the supported forms and a link to the typing guide; added a changelog entry.
- **Verification:** `make docs-build` passes, and the owning classes/language guides now agree with the milestone table and `docs/typing.md`.

### AUDIT-013 — Inherited protocol override errors omitted the contract relationship

- **Category / severity:** Inheritance diagnostics and navigation; Medium.
- **Status:** Fixed in `29238b54` (`fix: link inherited protocol override diagnostics`).
- **Reproduction:** Declare two local protocols with the same `update` requirement, have a base class implement both, then override `update` with an incompatible parameter in a subclass. The compiler rejected the override at the correct name but only said it was incompatible with the parent method; it did not name the protocol obligations or provide related requirement locations.
- **Expected:** Keep the primary highlight on the override name, identify all inherited protocol requirements affected, and attach a related LSP location for each declaration.
- **Root cause:** Override validation ran during class parsing without protocol declarations in its inputs. The later conformance pass does not re-check a child that inherits its base class's conformance.
- **Fix:** Parse protocols before class validation and provide those declarations to the shared override validator. It walks local ancestors, collects each distinct matching requirement, and augments the existing override diagnostic with its contract relationship and source location.
- **Verification:** `inherited_protocol_conformance_survives_overrides_and_reports_the_method_span` asserts the primary span and both requirement links for two inherited protocols. LSP `inherited_protocol_override_diagnostics_link_to_the_requirement` checks published primary and related ranges. Full core (1,864 passed, 1 ignored), full LSP (410 passed), Clippy, and docs build pass.
- **Remaining edge:** Broader imported/inherited combinations and override range matrices remained open at this finding's original revision. The imported-requirement case is covered by AUDIT-014.

### AUDIT-014 — Imported protocol obligations were missing from inherited override diagnostics

- **Category / severity:** Cross-file diagnostics and recovery; Medium.
- **Status:** Fixed in `b09ef990` (`fix: link imported inherited protocol obligations`); the implementation is included in the verified packaged VSIX.
- **Reproduction:** Put `Updatable` in `contracts.tua`; in `actors.tua`, define a local `Parent implements Updatable` with the matching `update(dt: string)`, then a local `Child extends Parent` whose `override update(dt: number)` changes the signature.
- **Expected:** Report the override at `Child.update`, name the inherited `Updatable.update` obligation, and link to the imported requirement declaration.
- **Actual before fix:** The override was rejected with only “signature incompatible with its parent method”; it had no related location for the imported protocol requirement.
- **Root cause:** Override validation records a class diagnostic while extracting the typed source model. Strict parsing returns the diagnostic without retaining the otherwise valid class ancestry, so project-aware diagnostics could not resolve imported protocol declarations.
- **Fix:** The analysis-only degraded parse now retains the typed model and leaves strict parse diagnostics untouched. The shared project-aware diagnostic pass walks ancestor protocol obligations through the same class helper used by local validation, resolves imported method definitions through `ProjectTypeDatabase`, and enriches the existing diagnostic. It does not add import resolution to the parser.
- **Verification:** Core analysis and real LSP regressions assert the imported file and exact method range. Full core (1,865 passed, 1 ignored), full LSP (413 passed), CLI (92 passed, 2 ignored), and Clippy pass.
- **Regression value:** Imported protocol contract information is preserved for inherited class diagnostics without changing the error's primary range or strict parser behavior.

### AUDIT-015 — Malformed nested protocol/class text could panic during lowering

- **Category / severity / confidence:** Parser recovery and code generation; Medium; confirmed by minimized libFuzzer crash inputs.
- **Status:** Fixed in `0572dd5e` (`fix: diagnose nested protocol declarations safely`); its compiler is included in the tested VSIX.
- **Reproductions:** `fuzz/seeds/source_pipeline/protocol_malformed_class.tua` puts a class declaration inside a protocol body, where it is an invalid protocol member. `fuzz/seeds/source_pipeline/protocol_nested_declaration.tua` puts protocol-shaped text inside a generic class body, where it is an invalid class member. Both files are UTF-8 source-pipeline fuzz seeds and can be replayed by the `source_pipeline` target.
- **Expected:** Both sources return a positioned syntax/class-member diagnostic. Invalid input must not abort compiler or editor requests.
- **Actual before fix:** The first input caused class parsing to find a class block inside the protocol body while protocol erasure also covered that source range. The lowering edit set overlapped and an internal `expect` panicked. A minimized variant of the second input showed the protocol scanner treating a declaration nested in a class body as top-level; class and protocol lowering again produced overlapping edits.
- **Root cause:** Class extraction scanned into protocol spans, and protocol discovery did not track brace nesting outside declarations. A declaration keyword inside an invalid nested body could therefore be interpreted as a second top-level declaration.
- **Fix:** Parse protocol declarations before class lowering and skip class candidates within each protocol's source span. Track lexical brace depth when locating top-level protocol declarations, while continuing to skip comments and string literals, so protocol-shaped text inside a class body is not surfaced as a top-level contract. The strict parser still reports the invalid body member.
- **Verification:** `class_like_text_inside_a_malformed_protocol_does_not_overlap_lowering_edits` and `protocol_declarations_nested_in_class_bodies_are_not_top_level_contracts` cover the minimized failures. The protocol-focused core selection passes (51 tests); the full core and LSP suites pass (1,868 core passed, 1 ignored; 414 LSP passed). Both minimized inputs are retained as fuzz seeds. A 30-second nightly source-pipeline fuzz run against the source-pipeline seed corpus completed without a crash (22,621 executions).
- **Regression value:** Malformed class/protocol combinations now yield diagnostics rather than panics or corrupted edit sets. The broader malformed-edit matrix remains open.

### AUDIT-016 — Imported generic class members had no project-level definition target

- **Category / severity / confidence:** Editor navigation and semantic parity; Medium; confirmed in both the project analysis test and installed VS Code.
- **Status:** Fixed in `dc3413d1` (`fix: navigate imported generic class members`); the installed VSIX regression passes.
- **Reproduction:** In `main.tua`, construct an imported `Box<number>` and place the cursor on `get` in `typed_box:get()`. A corresponding field reproduction places the cursor on `value` in `typed_box.value`.
- **Expected:** Go to Definition opens the generic class's method or field declaration in its defining module, matching the imported receiver type already shown by hover, completion, and signature help.
- **Actual before fix:** The type was correctly inferred as an imported generic class instance and member completion/hover worked, but project-level definition returned no location. An installed VS Code request timed out waiting for the class method definition.
- **Root cause:** `ProjectTypeDatabase::resolve_type` intentionally preserves `Type::GenericClassApplication` so nominal identity and invariant arguments remain intact. `class_member_definition_for_type` handled nominal `Type::Table` values only and fell through for generic class applications.
- **Fix:** Resolve a generic class member's declaration through the application's stable `GenericDeclarationId`, then use the existing class member source-target helper in the defining module. Generic substitution is not needed to select a declaration; imported identity is.
- **Verification:** The project-analysis regression asserts imported generic field and method targets resolve to `src/box.tua` and the exact member names. Installed VS Code coverage also verifies imported generic free-function navigation, generic class method navigation, and an unsaved generic record-alias edit that updates dependent hover type and restores when reverted. The exact real-server command and package hash are recorded below.
- **Regression value:** Navigation now follows the nominal generic class identity across modules without erasing or structurally guessing its type.

### AUDIT-017 — Nested protocol and generic class declarations were silently erased

- **Category / severity / confidence:** Parsing and source transformation correctness; High; confirmed with a CLI reproduction.
- **Status:** Fixed in the current worktree; a regression covers function, conditional, standalone `do`, loop, nested `do` within a loop, and `repeat` blocks.
- **Reproduction:** In `src/main.tua`, use:
  ```tua
  local function build()
    protocol Hidden {
      run(): nil
    }
  end
  ```
  Run `tua check` from the project root. Before the fix, it reported `checked 1 file(s)`; `tua build --emit-on-error` generated Lua with the protocol text blanked out. A generic class declaration in a Lua block followed the same extraction path.
- **Expected:** Protocols and native class declarations are top-level declarations. Nested declarations must remain in parser input and produce a syntax diagnostic; they must not disappear from the emitted program silently.
- **Actual before fix:** The declaration scanners used indentation and brace depth but did not track Lua statement blocks. An indented top-level declaration and a declaration inside `function`, `if`, or loop bodies were indistinguishable.
- **Root cause:** `parse_protocol_declarations` and `class_blocks` scanned declaration-like text independently of Lua block scope. Protocol lowering ran before the parser could reject nested syntax.
- **Fix:** Share a lightweight lexical Lua block tracker across both scanners. It skips comments, strings, and brace-delimited data, tracks functions, conditionals, loops, standalone `do`, and `repeat` blocks, and recognizes Tua declarations only when both brace and statement-block depth are zero.
- **Verification:** `protocol_and_generic_class_declarations_are_not_erased_inside_lua_blocks` asserts nested declarations remain in lowered parser input and are rejected, while top-level declarations after closed blocks still parse. The focused core test, parser test group, formatting check, and `cargo clippy -p tua-core --all-targets -- -D warnings` pass. The CLI reproduction now fails with positioned `TL0001` diagnostics instead of reporting a successful check. Broader malformed-edit and editor recovery coverage remains open.
- **Recommended regression coverage:** Keep the table-driven block cases and add them to the source-pipeline fuzz seeds if future scanner changes introduce declaration erasure at other statement boundaries.

### AUDIT-018 — Workspace-folder changes left open-document diagnostics stale

- **Category / severity / confidence:** LSP workspace invalidation; Medium; confirmed by a failing protocol-level regression before the fix.
- **Status:** Fixed in this audit continuation.
- **Reproduction:** Start an LSP session with two workspace folders, open a project file whose class violates an imported protocol, then send `workspace/didChangeWorkspaceFolders` removing that project's root. Before the fix, the open document received only its original fast and full diagnostics; no diagnostics publication followed the workspace reindex.
- **Expected:** Rebuild workspace analysis for the new folder set, refresh open documents against that analysis, and notify the client using push diagnostics or the negotiated pull-diagnostics refresh.
- **Actual before fix:** The handler reindexed workspace state and reset work queues but returned only the pull-diagnostics refresh path. Push-diagnostics clients saw no update; open documents also retained pre-change derived state until another event refreshed them.
- **Root cause:** `did_change_workspace_folders_notification` omitted `refresh_open_documents` and used the negotiated pull refresh helper unconditionally.
- **Fix:** Refresh open-document analysis after reindexing, then request a pull diagnostic refresh for pull clients or publish all open-document diagnostics for push clients.
- **Regression coverage:** `removing_a_workspace_root_republishes_diagnostics_for_open_documents` checks push clients receive a post-reindex publication. `pull_diagnostics_refresh_after_workspace_folder_change` checks pull clients receive a refresh request, no push publication, and current diagnostics when the document is next requested. Existing workspace-symbol folder-change coverage remains green.
- **Verification:** Full `make verify` passed with 417 LSP tests; after adding the pull-client case, the full LSP suite passed with 418 tests. The tests intentionally do not assume that removing the workspace folder removes the file's nearest `tua.toml` context, since the open URI may still resolve that project context.
- **Recommended follow-up:** Extend dependency-churn tests to add/replace roots and edit, rename, and delete multi-hop dependencies while requests are in flight.

### AUDIT-019 — Concrete generic parent specializations lost their type arguments

- **Category / severity / confidence:** Generic inheritance and type inference; High; confirmed by checker, editor-query, and emitted-Lua reproductions.
- **Status:** Fixed in the current worktree; installed VSIX regression passes.
- **Reproduction:** Declare `open class Base<T>` with a `value: T`, `init(value: T)`, and `get(): T`; then declare `final class NumericBox extends Base<number>` with `init(value: number) { super(value) }`. Before the fix, `tua check` reported `expected Base<T>, found NumericBox`, inherited field and method types remained `T`, and assigning the instance to `Base<number>` failed.
- **Expected:** A concrete application in `extends` specializes the subclass's inherited members and parent initializer. The subclass remains assignable to that exact applied parent and must not be assignable to another invariant application.
- **Actual before fix:** Applied parent arguments were represented in generic class definitions, but a non-generic subclass instance and its method receiver were represented as the legacy bare class alias. That discarded the generic parent edge when resolving `self`, constructor returns, and member types.
- **Root cause:** The checker used `GenericClassApplication` only for classes declaring their own binders. A class with no local binder but an applied generic ancestor fell back to a structural nominal alias, so generic parent substitutions were unavailable to shared member and assignability queries.
- **Fix:** Retain a zero-argument nominal generic-class application for non-generic classes with a generic ancestor. Resolve their class names, generated constructor results, and `self` receivers through the same class identity and ancestor substitution path. Formatting omits empty angle brackets. Existing non-generic classes without generic ancestors keep their legacy alias representation.
- **Regression coverage:** `nongeneric_subclass_can_specialize_a_generic_parent` covers a concrete `Base<number>` parent, a second subclass in the chain, typed and inferred construction, `super(...)`, inherited field and method typing, accepted `Base<number>` assignment, and rejected `Base<string>` assignment. `concrete_generic_parent_substitution_reaches_completion_and_hover` checks member completion and method hover. `concrete_generic_parent_substitution_preserves_emitted_lua_behavior` builds the chain and executes the output under the available Lua runtime.
- **Verification:** Focused core, analysis, and CLI runtime tests pass; full `make verify`, documentation build, LSP feature smoke, extension checks/package, and the installed test against the exact packaged VSIX all pass on macOS arm64. The real VS Code test confirms concrete parent field hover, inherited member completion, and specialized method signature return types through the VSIX's bundled server.
- **Regression value:** Concrete and transitive generic inheritance now has the same substituted receiver, constructor, member, and nominal assignability behavior as binder-to-binder inheritance.

### AUDIT-020 — Recursive protocol signatures needed same-file and hover regressions

- **Category / severity / confidence:** Protocol identity and editor consistency; High original impact; confirmed coverage gap.
- **Status:** The resolver fix was already present in `c84a5212` and `d04e3783`. This audit added same-file and exact-hover regressions; no additional product-code change was necessary.
- **Reproduction:** Define mutually recursive `A` and `B` contracts where `A.f` accepts `B` and `B.g` accepts `A`, then inspect a `B`-typed parameter and complete `value:g`. Repeat once in one source file and once across type-only imports. Matching implementations should conform; swapped protocol parameters should fail.
- **Expected:** The recursive back-edge retains the canonical protocol identity. Hover should keep the source-facing `B` type, completion should expose `B.g`, matching contracts should conform, and incompatible contracts should remain rejected.
- **Actual:** Current core/project analysis returns exact hover text `parameter value: B`, offers `g` for the typed receiver, accepts matching recursive implementations, and diagnoses both incompatible method parameters in same-file and cross-file fixtures.
- **Root cause and fix:** Earlier fixes preserve protocol identities when resolving recursive alias back-edges and register canonical imported identities in assignability. The remaining audit gap was that same-file and hover-consistency variants were not directly asserted. No checker, hover, or completion special case was added.
- **Regression coverage:** `same_file_recursive_protocol_signatures_keep_conformance_and_editor_types` and `cyclic_imported_protocol_signatures_preserve_conformance_identity` in `crates/tua-core/src/analysis/tests.rs` assert positive and negative conformance, exact hover, and completion. The VS Code real-server fixture now also verifies a mutually recursive two-file signature through editor hover, method completion, and diagnostics.
- **Verification:** Both focused core tests pass. `make extension-real-server-test` passes on VS Code 1.142.0/macOS arm64 with the recursive fixture: VS Code shows the nominal `B` parameter, completes `g`, shows `g: (value: A) -> nil` and its documentation, and publishes no errors for the valid call.
- **Recommended fix direction:** None remains for the confirmed defect. Keep the core and installed-editor cycle fixtures as regression anchors.

### Audit reconciliation — protocol method-stub action

The follow-up audit summary listed missing-method stub generation as an editor
capability gap. That item is already implemented in commit `971fd553` and its
line-ending fix `c71538f3`. Core analysis derives missing requirements from the
shared protocol model, accounts for imported contracts and inherited
implementations, and skips conflicting same-named obligations rather than
guessing a signature. The LSP action inserts explicit throwing TODO bodies.
LSP tests cover imported/inherited requirements, multiple methods, and
conflicting obligations; the installed VS Code suite applies the action and
confirms the missing-method diagnostic clears. It is not an open capability
gap in the current checkout.

### AUDIT-021 — Recursive protocol returns lost nominal identity

- **Category / severity / confidence:** Protocol type identity and inference; High; confirmed by CLI, analysis, LSP, and installed-editor reproductions.
- **Status:** Fixed in `e4e573ea`; same-file and imported-return coverage was added in `cff5e939`.
- **Reproduction:** Define mutually recursive `A` and `B` protocols where `A.next(): B` and `B.previous(): A`. Implement both on one class, assign the instance to `A`, then assign `a:next()` to `B`.
- **Expected:** The returned value retains nominal type `B`; assignment succeeds, hover says `local target: B`, and completion offers `B.previous`.
- **Actual before fix:** The local protocol environment expanded a recursive return through its method-table alias into a structural shape (or `any`). The checker could reject the valid assignment while hover showed a structural type and completion still exposed protocol members. The issue reproduced in same-file and cross-file arrangements.
- **Root cause:** `build_type_environment_with_domain_registry` represented local protocol aliases as method tables but did not register their nominal protocol identity on the table shape, unlike project protocol facts. Recursive alias expansion therefore had no canonical identity to preserve at the back-edge.
- **Fix:** Register local protocol method-table shapes with `protocol_nominal_identity`, allowing the shared resolver to preserve canonical protocol identity through recursive returns.
- **Regression coverage:** Core analysis tests cover same-file and imported cycles, positive and negative conformance, exact nominal hover, and member completion. The installed VS Code real-server fixture additionally checks the cyclic return hover (`local target: B`), completion of `B.previous`, cyclic parameter hover/completion/docs, and clean diagnostics against the bundled server.
- **Verification:** The CLI reproduction now checks without a false mismatch. Focused core/LSP suites, full serial `make verify`, development-server VS Code real-server suite, and exact packaged-VSIX real-server suite passed. See the November 10 verification entries below.
- **Regression value:** The canonical nominal identity survives a recursive method return consistently across checking, hover, and completion.

### AUDIT-022 — Imported generic protocol bounds retained a stale file-only error

- **Category / severity / confidence:** Cross-file generic constraint diagnostics; Medium; confirmed by CLI and LSP reproductions.
- **Status:** Fixed in `d7c56d50`; the re-export fixture cleanup is in `580307ea`.
- **Reproduction:** Put `Updatable` in a contracts module, directly re-export it from another module, declare `tick<T: Updatable>`, then call it with an imported class that explicitly implements `Updatable` and a structurally similar class that does not.
- **Expected:** The conforming class passes the imported bound; the lookalike remains rejected because protocol conformance is nominal.
- **Actual before fix:** Project-aware checking accepted the conforming class and correctly rejected the lookalike, but the earlier file-only pass emitted a stale TL1015 for the conforming class. The LSP diagnostic merge retained that stale error alongside authoritative project diagnostics.
- **Root cause:** The file-only pass lacked the canonical identity for an imported/re-exported protocol and conservatively emitted a generic-bound failure. When project checking was authoritative, the diagnostic reconciliation removed stale file-only protocol conformance diagnostics but did not also remove the generic-inference diagnostic category.
- **Fix:** During project diagnostic reconciliation, filter `TYPE_GENERIC_INFERENCE` from the stale file-only set when the project-aware check is authoritative. The project pass remains responsible for the final positive and negative result.
- **Regression coverage:** CLI and LSP tests assert that the imported conforming class receives no TL1015 while the structurally similar nonconforming class still receives TL1015. The fixture avoids a redundant local type import so it tests the bound diagnostic rather than the unrelated unused-import lint.
- **Verification:** The CLI reproduction now reports only the expected Lookalike bound error. Full serial `make verify` and the exact packaged VSIX real-server test pass.
- **Regression value:** Cross-file generic bound diagnostics now agree with authoritative project-aware conformance without weakening nominal rejection.

### AUDIT-023 — Formatter inserted spaces into generic function headers

- **Category / severity / confidence:** Generic syntax formatting; Medium; confirmed by the bundled feature-project format check.
- **Status:** Fixed in the current worktree.
- **Reproduction:** Run `target/release/tua fmt --check` from `examples/features` on the pre-fix checkout. `src/protocols.tua` contains `local function updateOne<T: Updatable>(...)`; the formatter proposed `local function updateOne < T: Updatable > (...)`, which is not valid Tua generic-function syntax. The same formatting defect affected compact generic function headers generally.
- **Expected:** Formatting preserves Tua's generic binder syntax and produces a stable, parseable, semantically valid result.
- **Actual before fix:** The spacing normalizer treated the binder's `<` and `>` as binary operators. This made the checked-in feature example fail `fmt --check` and changed source into syntax the parser does not accept.
- **Root cause:** `normalize_spacing` normalized each source line without recognizing generic function declaration headers; generic class headers had a separate path and existing tests did not cover function binders.
- **Fix:** Detect the binder span on named generic function declarations and normalize its parameters and optional protocol bound while keeping the delimiters adjacent to the function name and parameter list.
- **Regression coverage:** `formatter::tests::formats_generic_function_headers_without_changing_their_syntax` asserts the bounded header remains compact, formatting is idempotent, and the result passes semantic checking.
- **Verification:** The focused test, `cargo fmt --all -- --check`, bundled `tua fmt --check`, full serial `make verify`, and `make benchmark-smoke` pass. The exact rebuilt VSIX passes the installed VS Code real-server suite.
- **Recommended regression coverage:** Keep this test with generic function grammar changes; add multi-binder forms if the supported generic syntax grows.

### AUDIT-024 — Generic analysis lacked an explicit call-instantiation work budget

- **Category / severity / confidence:** Generic checker resource use; Medium; confirmed by code-path inspection and bounded stress regressions.
- **Status:** Addressed in the current worktree.
- **Reproduction:** Generate more than 4,096 distinct calls to a generic function, each with a distinct literal argument type, then run `tua check`. Repeated identical calls are the control case.
- **Expected:** Repeated specializations are reused, and generated call-heavy input has a documented finite checker budget with one actionable diagnostic when exceeded.
- **Actual before fix:** Each generic call independently specialized its declaration scheme; no per-source checker limit or repeated-call cache existed. Alias and constraint traversals had independent depth/work behavior, but that did not bound generic call specialization work.
- **Fix:** Add a per-file checker cache keyed by the generic function scheme and argument types. Count at most 4,096 distinct generic call candidates in one source check and emit a single TL1015 work-limit diagnostic when exceeded. Preserve an over-budget generic alias symbolic rather than returning a partial structural projection; explicitly stop constraint binder scans at the solver's recursion boundary.
- **Regression coverage:** Core tests exercise 4,097 distinct call types (one limit error), 4,097 identical calls (cache reuse), a 4,200-field alias expansion, a 140-link alias chain, and inference immediately below and at its 64-level recursion boundary.
- **Scope:** The 4,096 counter covers generic call candidates in the per-source checker. Callback-value specialization and semantic/LSP query instantiations are outside that counter; those paths retain their existing per-operation recursion limits. It is not a whole-process memory ceiling.
- **Verification:** All new tests pass in the full 1,882-test core suite; full workspace `make verify`, `make benchmark-smoke`, package checks, and installed VSIX checks pass.

### AUDIT-025 — Generic alias work limits erased known member types

- **Category / severity / confidence:** Type-checker soundness at an expansion boundary; Medium; confirmed by minimized CLI and checker reproductions.
- **Status:** Fixed in the current worktree.
- **Reproduction:** Define `type Wide<T> = { f1: T, ... }` with more than 4,096 fields, then assign `wide.f4199` from `Wide<number>` to a `string`. A second fixture uses a branching generic alias graph whose full expansion exceeds the budget and reads a field through a 12-member path.
- **Expected:** A known member retains its substituted type, and the invalid string assignment reports TL1003.
- **Actual before fix:** The structural-node work counter was exhausted while expanding the table. The resolver retained the symbolic root application, and member lookup degraded the known field to `any`, silently accepting the assignment.
- **Fix:** Count generic alias applications rather than every record field against the full-expansion work limit. Resolve requested member paths lazily from one substituted alias body with a finite 64-edge depth, and preserve a concrete table's generic application at the selected edge without recursively expanding sibling fields.
- **Source:** `crates/tua-core/src/types/environment.rs:444` and `:999` implement one-step alias substitution and the expansion budget; `crates/tua-core/src/semantic.rs:2651` implements bounded member lookup. Regressions are in `crates/tua-core/src/types/tests.rs`, `crates/tua-core/src/check/tests.rs`, and `crates/tua-core/src/analysis/tests.rs`.
- **Regression coverage:** `wide_generic_alias_members_keep_their_substituted_types`, `generic_alias_member_lookup_stays_typed_after_full_expansion_is_bounded`, `wide_generic_alias_member_keeps_its_type_past_the_work_boundary`, and `generic_alias_expansion_work_limit_preserves_a_symbolic_application` cover large records, incorrect assignments, branching work exhaustion, and fail-closed symbolic expansion. An independent real LSP probe confirmed receiver/member hover types for the 4,200-field alias.
- **Verification:** Independent CLI checks reject both invalid assignments with TL1003; all 20 generic-alias-focused core tests pass. A 4,200-field completion request exceeded the independent probe's 30-second timeout; it remains a stress follow-up and does not invalidate the checker/hover results.

### AUDIT-026 — Protocol requirements accepted untyped parameters

- **Category / severity / confidence:** Protocol contract validation; Medium; confirmed by CLI, parser, and LSP diagnostics.
- **Status:** Fixed in the current worktree.
- **Reproduction:** Declare `protocol P { decode(input): nil }` or `protocol P { decode(input:): nil }` and implement the same method without a parameter annotation.
- **Expected:** Reject the malformed protocol member at the parameter with one precise syntax diagnostic, and avoid a misleading downstream conformance error.
- **Actual before fix:** The method signature was accepted with an unknown/`any` parameter type, allowing a contract to be under-specified; some invalid signatures also produced confusing conformance cascades.
- **Fix:** Parse protocol parameters with balanced delimiter/comment-aware ranges; require explicit, non-empty annotations for named and variadic parameters; omit malformed requirements from the protocol so the parser diagnostic remains authoritative.
- **Source:** `crates/tua-core/src/classes.rs:604` parses/validates protocol parameter annotations. Core coverage is in `crates/tua-core/src/parser/tests.rs`; published diagnostic coverage is in `crates/tua-lsp/src/server/tests/diagnostics.rs`.
- **Regression coverage:** `protocol_requirements_reject_missing_and_empty_parameter_types` checks ordinary, empty, and variadic parameters plus exact source ranges and a valid multiple-return control. The LSP regression asserts the two TL0001 diagnostics and no TL1008 cascade.
- **Verification:** Independent CLI probes observed one TL0001 at `main.tua:2:5` for each malformed case and no TL1008; the valid typed variadic/two-return contract checks. Core and LSP focused tests pass.

### AUDIT-027 — Generic function binder comments disappeared during Lua erasure

- **Category / severity / confidence:** Source-preserving code generation; Low; confirmed with emitted-Lua and runtime reproduction.
- **Status:** Fixed in the current worktree.
- **Reproduction:** Put line comments between `id<`, `T`, and `>` in a generic function declaration, then run `tua build` and Lua on `print(id(42))`.
- **Expected:** Erase the generic binder while preserving both user comments exactly once in valid Lua.
- **Actual before fix:** `with_generics(None)` discarded comment trivia attached to the generic declaration even though the compiled program still printed `42`.
- **Fix:** Transfer source-ordered comment trivia and required line breaks from erased generic binders into the function parameter span; preserve trivia when no return annotation is present as well.
- **Source:** `crates/tua-core/src/emit/mod.rs:1671` and `:1700` preserve generic binder comment trivia; `crates/tua-core/src/emit/tests.rs` contains the Lua parse regression.
- **Regression coverage:** `generic_function_header_comments_survive_type_erasure` asserts both comments survive exactly once, the generic syntax is absent, and the emitted Lua parses. An independent CLI build/run probe printed `42` after the fix.

### AUDIT-028 — Large generic record completion repeated typed-domain resolution

- **Category / severity / confidence:** Editor completion performance; Low; redundant per-member domain work is confirmed by source/profile evidence, but the original end-to-end timeout is not confirmed.
- **Status:** Optimization implemented; no current completion defect reproduced.
- **Reproduction:** Run `cargo build -p tua-cli`, then `python3 internal/audits/generics-polymorphism-0.7.0-final/reproduce_large_generic_completion.py`. The script creates an imported `Wide<T>` alias with 4,200 fields and asks the actual stdio LSP for completion after `wide.f419`. The current server returns 11 matching labels; a corrected independent probe measured about 54 ms for the request on macOS arm64.
- **Expected:** Completion should remain responsive on wide generic records while retaining all matching members and typed-domain method documentation.
- **Observed behavior:** `complete_fields` called `CompletionDomainRegistry::typed_member` for each table member. Domain checks resolve and inspect the receiver type, so ordinary scalar fields repeated receiver classification. Profiling showed member collection in milliseconds and per-candidate domain enrichment dominating that path. A prior 60-second timeout was later traced to an invalid probe request/cursor and is excluded as symptom evidence.
- **Fix:** Call typed-domain documentation resolution only for function-valued completion items. Typed domain methods are represented as function-valued fields; ordinary values cannot have method documentation. This keeps existing domain-method documentation while avoiding expensive receiver classification for record values.
- **Source:** `crates/tua-core/src/completion/mod.rs` (`complete_fields` and `typed_member_documentation`).
- **Regression coverage:** `large_generic_record_completion_keeps_all_fields_available` expands and completes a 4,200-field `Wide<number>` alias, asserting all labels are retained. `skips_typed_domain_documentation_for_non_function_fields` verifies the early exit. Existing ECS, store, state-machine, audio, and input completion tests cover function-method documentation.
- **Verification:** `RUST_MIN_STACK=33554432 cargo test -p tua-core` passed (1,889 passed, 1 ignored); `cargo test -p tua-lsp` passed (427); `cargo test -p tua-core completion::tests::` passed (149). The dedicated 4,200-field regression passed in 0.30 seconds. The saved LSP script returns all expected matches; a corrected sequential probe measured about 54 ms. `make extension-package` passed with 180/180 extension tests and `npm audit` reporting 0 vulnerabilities. The exact package installed in an isolated profile and passed `test:vscode-real` on VS Code 1.142.0/macOS arm64. Earlier artifact SHA-256: `977162265a6b6be4a8b46cbaa7dfd90f9491dc756de1b6fbea9118bbd760a0a3`.

### AUDIT-029 — Conformance edits and inherited signature range matrix needed broader LSP coverage

- **Category / severity / confidence:** LSP regression-coverage gap; Medium; confirmed by comparing the P2/P3 acceptance matrix to the pre-existing tests. No new product defect was observed in the added scenarios.
- **Status:** Addressed with regressions in the current worktree.
- **Reproduction:** Run `cargo test -p tua-lsp protocol_signature_mismatch_matrix_preserves_lsp_ranges_and_requirement_links`, `cargo test -p tua-lsp inherited_override_signature_matrix_preserves_lsp_ranges_and_requirement_links`, and `cargo test -p tua-lsp unsaved_consumer_conformance_and_method_edits_refresh_protocol_diagnostics`.
- **Expected:** Protocol conformance diagnostics for parameter, return, required-argument-count, vararg, and return-count mismatches consistently underline the failing implementation and link to the declaration. Unsaved edits that remove conformance, remove a required method, or restore it refresh dependent diagnostics after the scheduled project-analysis pass.
- **Actual before coverage:** Core tests checked these compatibility shapes, but exact LSP ranges and inherited protocol related-information links were concentrated on parameter mismatches. Dependency tests changed contracts and files, but did not cover removing/re-adding conformance and methods in the consumer buffer.
- **Coverage added:** A table-driven LSP test covers all five conformance mismatch shapes and asserts TL1008, exact UTF-16 primary/related ranges, and actionable detail. A second matrix covers the same five incompatible signatures as inherited overrides and verifies each protocol requirement location. A pull-diagnostics edit sequence checks clean → no conformance → missing method → restored method states.
- **Source:** `crates/tua-lsp/src/server/tests/diagnostics.rs`.
- **Remaining boundary:** The matrix covers signature-shape failures, not every malformed partial class-header edit. Multi-module repeated rename/delete/create graph permutations and cancellation races remain outside these focused regressions.
- **Verification:** `cargo test -p tua-lsp` passed with 431 tests after the subsequent AUDIT-030 addition. The immediate pull after `didChange` intentionally observes fast diagnostics; the test advances the scheduled full-diagnostics queue before asserting project conformance results.

### AUDIT-030 — Unsaved protocol dependency churn needed a transitive re-export case

- **Category / severity / confidence:** Cross-file invalidation coverage; Medium; confirmed by acceptance-matrix review. No product defect was observed in the added case.
- **Status:** Addressed with a regression in the current worktree.
- **Reproduction:** Run `cargo test -p tua-lsp unsaved_protocol_dependency_churn_refreshes_through_multiple_reexports`.
- **Expected:** An unsaved contract edit should propagate through a two-hop `.d.tua` type re-export. Removing the intermediate export should discard stale conformance facts and report the missing exported type; restoring it should recover the latest contract and its mismatch.
- **Actual before coverage:** Direct contract edits, re-export identity, dependency deletion/rename, and unsaved contract changes were tested individually. No LSP test combined an unsaved type-only re-export chain edit with removal and restoration of its intermediate export.
- **Coverage added:** The pull-diagnostics regression verifies clean initial conformance, a changed requirement through both re-exports, removal of the intermediate symbol without stale TL1008, and restoration that brings the changed-contract TL1008 back.
- **Source:** `crates/tua-lsp/src/server/tests/diagnostics.rs`.
- **Remaining boundary:** Repeated on-disk rename/delete/create permutations across a larger dependency graph and cancellation during those changes remain untested. This regression validates unsaved VFS overlays and re-export identity, not OS watcher races.

### AUDIT-031 — Trailing comments in generic protocol bounds were rejected and erased

- **Category / severity / confidence:** Generic syntax and source-preserving erasure; Low; confirmed with parser and emission regressions.
- **Status:** Fixed in the current worktree.
- **Reproduction:** Place a line comment after a supported protocol bound, for example `local function identity<T: Runnable -- note, >\n>(value: T): T ... end`. A comment containing `,` or `>` also exercises delimiter scanning.
- **Expected:** Parse `Runnable` as the bound, accept the comment as Lua/Tua trivia, then erase the bound while retaining the comment once in parseable Lua.
- **Actual before fix:** Bound extraction included the comment in the protocol name and reported TL0001. Normalization also blanked protected comment characters while erasing the bound.
- **Fix:** Locate generic-list delimiters outside protected comments and strings; erase only unprotected bound characters. Parse the protocol name separately from trailing comment trivia.
- **Source:** `crates/tua-core/src/parser/mod.rs` (`parse_generic_protocol_bound` and its trailing-comment check) and `crates/tua-core/src/parser/normalize.rs` (`protected_syntax_characters` and generic-bound erasure).
- **Regression coverage:** `generic_protocol_bounds_allow_trailing_header_comments` verifies parsing and bound identity after a prefixed source line. `generic_protocol_bound_comments_survive_type_erasure` checks a delimiter-containing comment survives exactly once and the emitted output remains valid Lua 5.1.
- **Verification:** The two focused tests passed; `CARGO_INCREMENTAL=0 RUST_MIN_STACK=33554432 RUST_TEST_THREADS=1 make verify` passed on the final source, including 1,891 core tests (1 ignored), 431 LSP tests, 92 CLI library tests (2 ignored), 137 CLI integration tests, 3 gameplay tests, and 180 extension tests. Core clippy passed.
- **Verification:** The focused test passes; `cargo test -p tua-lsp` passes 431/431, and `cargo clippy -p tua-lsp --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check` pass.

During AUDIT-025 integration, a first direct-field shortcut changed recursive-alias hover and class `Self` hover behavior. The full core and LSP suites exposed those regressions; the final shortcut only preserves a raw field edge when it contains a generic alias application, and targeted tests for recursive imported `Self` and growing recursive generic aliases now pass.

## Coverage matrix

Status key: **Exercised** means a focused regression or real request/runtime test ran; **Inspected** means code/docs were reviewed but not independently executed for that specific edge; **Pending** means no qualifying environment or scenario was available.

| Plan area | Status | Evidence and limits |
| --- | --- | --- |
| P0 protocol syntax, nominality, colon receiver, mutable views | Exercised | Positive/negative assignments, colon calls, explicit dot/extracted receivers, class array mutation safety, nominal identity, inherited overrides, and contextual identifier compatibility are covered. |
| P1 parsing, identities, spans, formatting and erasure | Exercised, partial | Local/exported/multiline and comment-separated protocol headers, required non-empty protocol parameter annotations with ranged LSP diagnostics, bounded generic function formatting and semantic round-trip, comments containing braces during recovery, generic binder comment preservation during Lua erasure, contextual words, editor recovery of unclosed protocol bodies, malformed nested class/protocol cases, and declaration rejection inside Lua function/conditional/loop/do/repeat blocks pass. Current-buffer queries retain partial declarations while dependency indexes keep last-good contracts. Broader malformed-edit recovery remains incomplete. |
| P2 conformance, variance, multiple obligations, inheritance | Exercised, partial | Positive/negative local/imported conformance, malformed requirement diagnostics for missing/empty parameter annotations, inherited satisfaction/override, same-name obligations, optional parameters, typed varargs, multiple return slots/counts, variance, exact override/parameter/return locations, related locations for imported direct conformance, and multiple imported requirements inherited through local bases are tested. New LSP matrices cover parameter, return, required-argument-count, vararg, and return-count mismatch ranges for direct conformance and inherited overrides. Imported generic bounds accept explicitly conforming classes and reject structural lookalikes in CLI/LSP regressions. Partial malformed-header edits remain open. Cross-module class inheritance is unsupported by the current class contract. |
| P3 module identity, re-exports, cycles, edits | Exercised, partial | Named/namespace/re-export and cyclic protocol identity pass. Same-file and cross-file recursive signatures and method returns retain nominal identity: matching conformance passes, incompatible parameters are rejected, returned `B` remains `B`, and member completion agrees with hover. Editing one protocol inside a mutual import cycle refreshes implementing-class diagnostics while completion through the peer protocol remains available in the same analysis snapshot. Saved/unsaved refresh tests pass. New pull-diagnostics coverage removes and restores consumer conformance and required methods. A second test changes a contract through two declaration-file re-exports, removes the intermediate export without retaining stale conformance, then restores it and observes the changed mismatch again. Deleting a contract invalidates its last-good facts, reports the missing import, and removes stale conformance checks. Renaming a contract, changing the consumer import, and requesting diagnostics after the debounce reports the new mismatch. LSP tests prove same-named protocols and generic aliases with different contracts remain isolated across workspace roots in either root ordering. Workspace-folder changes reindex open-document analysis and republish push diagnostics; repeated filesystem churn and cancellation races are not exhausted. |
| P4 completion, hover, signatures, navigation, rename | Exercised on macOS, partial | Core/LSP suites and installed VSIX cover protocol completion, hover, signature, definition, implementation navigation, references/rename, and applying the missing-protocol-method quick fix; generic alias/class/function completion/hover/signatures, alias and imported function/class member definitions, generic binder definition/references/rename, and unsaved dependency edits that update and restore dependent generic hover types. Recursive same-file and cross-file protocol fixtures assert exact hover/completion agreement; the installed VSIX real-server test now verifies cyclic signatures through the actual editor adapter. Cross-platform qualification remains open. |
| P5 examples, generated Lua, runtime/source maps/hot reload | Exercised, partial | Feature/game examples build; gameplay polymorphism and generic multi-return emitted Lua execute on the available Lua runtime. A new end-to-end test builds a generic class and maps a runtime error from its method to the exact Tua source line. CLI and extension suites also cover source maps and hot reload. LOVE/LuaJIT runtime qualification and visual behavior remain pending. |
| G0 design contract and limits | Inspected | Decisions are present in plan and user docs; future generic methods, generic protocols, packs, explicit call-site args, compound bounds, and advanced LuaCATS are explicit limitations. |
| G1 aliases, identity, substitution, recursion, imports | Exercised, partial | Exact arity, unknown names, capture-safe substitution, imported/re-exported aliases, same-argument recursion, and growing-argument recursion are tested. The first repeated declaration stays symbolic with substituted arguments; other alias cycles retain work/depth guards. New regressions cover full-expansion budget exhaustion, typed member lookup across a 4,200-field alias and branching graph, and depth boundaries; exhaustive adversarial alias graph shapes remain untested. |
| G2 functions, inference, callbacks, LuaDoc | Exercised, partial | Independent calls, body checking, conflicts, nil/unions/empty table, callback contexts/ambiguity, bounds, varargs, LuaDoc generic forms, multi-return slot inference/runtime, repeated-call caching, and the 4,096 distinct-call budget are covered. Callback-only/return-only inference and explicit callsite args are deliberately unsupported; callback-value and LSP-query specializations are not charged to the per-source call budget. |
| G3 classes, constructors, substitution, bounds | Exercised, partial | Local/imported generic instances, constructor/member inference, concrete and transitive applied generic parents, `super`, invariant containers, protocol bounds, hover/completion/navigation covered. An imported protocol inherited through a local base now links from a child override. Cross-module class inheritance and generic methods/protocols remain unsupported. |
| Real editing and snapshots | Exercised, partial | Unsaved dependency edits, cyclic imports, incomplete consumer edits, warm snapshots and watched changes have core/LSP coverage. Long-session and broad dependency churn remain open. |
| UTF-16/editor client integration | Exercised | LSP UTF-16 and cross-file navigation/rename suites pass; exact VS Code test passes on macOS arm64. Other clients and platforms pending. |
| Performance and memory | Exercised, partial | Repeated five-cold/twenty-warm profile completed with p50/p95 for single-file and project fixtures; no before/after baseline, dependency-churn timing or long-session memory qualification. |
| Release artifact/package | Exercised, partial | `make extension-package` passes (180/180 extension tests, npm audit 0 vulnerabilities), VSIX archive validates, exact current VSIX installs and runs in isolated VS Code. No signing, publishing, or external beta distribution performed. |

## Verification log

All commands ran from repository root unless noted.

### November 10 focused audit commands

| Command | Outcome |
| --- | --- |
| `cargo fmt --all -- --check`; `git diff --check` | Passed after AUDIT-025 through AUDIT-027; formatting was applied with `cargo fmt --all`. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=33554432 cargo test -p tua-core generic_alias_` | Passed: 20 tests, including the 4,200-field member boundary, symbolic expansion stop, imports, recursion, and substitution. |
| Focused core tests for protocol parameter annotations and generic-header comments; focused LSP tests for protocol diagnostics, growing generic aliases, class `Self`, and recursive imported `Self` | Passed individually. The LSP diagnostic response contains exactly two TL0001 errors and no TL1008 cascade. The recursive-alias hover remains symbolic, and class `Self`/guarded-capture completion and hover remain available. |
| Independent `tua check` reproductions for the 4,200-field and branching generic aliases | Both rejected the invalid string assignment with TL1003 (`expected string, found number`). Independent stdio LSP hover reported `Wide<number>` and `(field) f4199: number`. Dot completion on the artificial 4,200-field shape did not answer within 30 seconds; not classified as a confirmed regression. |
| Generic-comment CLI reproduction (`tua build`, then Lua 5.4) | Passed: generated Lua retained the binder comments, and the program printed `42`. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=33554432 RUST_TEST_THREADS=1 make verify` | Rust workspace tests passed: core 1,887 passed/1 ignored; LSP 427 passed; CLI library 92 passed/2 ignored; CLI integration 137 passed; gameplay 3 passed; examples/syntax passed. Clippy, version/config checks, language evaluation, schema/JSON checks passed. The aggregate command exited at extension tests because the sandbox denied a loopback bind; the separately rerun `npm --prefix editors/vscode run check` passed 180/180 with local loopback access. |
| `make docs-build` | Passed; documentation build reported no issues. |
| `make benchmark-smoke` | Passed with local process-sampling access. Representative profile: single-1000 cold completion p95 249.47 ms, warm 6.85 ms, contention 154.64 ms; project-100 cold 120.59 ms, warm 0.40 ms, contention 30.42 ms. Memory remained under the configured 1,024 MiB ceiling. This smoke run uses one cold sample and is not a stable release-host baseline. |
| `make extension-package` | Passed: npm audit found 0 vulnerabilities; extension tests 180/180; generated the 0.7.0 VSIX with 40 files, 8.71 MB (9,129,987 bytes). |
| `env TUA_TEST_VSIX="$PWD/editors/vscode/tua-0.7.0.vsix" npm --prefix editors/vscode run test:vscode-real` | Passed against the exact packaged VSIX, installed into an isolated profile; bundled Tua server started and the real-server editor suite exited 0 on VS Code 1.142.0/macOS arm64. |
| `unzip -tq editors/vscode/tua-0.7.0.vsix`; `shasum -a 256 editors/vscode/tua-0.7.0.vsix` | Archive validation passed. SHA-256 `8de57325a50b0adbbbe2045f1b83fe556dfdbc833a3b1d39c96a89a1db966f46`. |

| Command | Outcome |
| --- | --- |
| `make extension-real-server-test` | Passed on VS Code 1.142.0/macOS arm64 after adding a cyclic cross-file protocol fixture. Parameter hover retained `B`, completion offered `g`, method hover rendered the `A` parameter and imported documentation, and the valid call produced no error diagnostics. Uses the release-built server from the current worktree. |
| `env CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 make verify` | Passed after AUDIT-018. Formatting, workspace tests (core 1,869 passed/1 ignored; LSP 417; CLI unit 92 passed/2 ignored, integration 134; gameplay 3; examples and syntax checks passed), Clippy, version/release/config/schema/LLM checks, language evaluation, extension tests (180/180), JSON validation, and diff checks all passed. This run used local host access for extension tests that need loopback. The larger Rust test stack is required by a known deeply nested core test. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-core` | Passed after AUDIT-020: 1,872 passed, 0 failed, 1 ignored. Includes same-file and cross-file cyclic protocol conformance, hover, and completion regressions. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-lsp` | Passed after AUDIT-020: 418 passed, 0 failed. |
| `make lsp-features-smoke` | Passed after AUDIT-018 against `examples/features` using the rebuilt debug server. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 cargo test -q -p tua-lsp` | Passed after adding push and pull workspace-folder diagnostic regressions: 418 passed, 0 failed. `cargo fmt --all -- --check` and `git diff --check` pass for the resulting test changes. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -q -p tua-lsp` | Passed after adding the editor-diagnostic regression: 416 passed, 0 failed. `did_open_publishes_actionable_module_scope_diagnostic_for_nested_protocol` asserts the published `TL0001`, exact start position, and message. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 cargo test -p tua-core protocol_and_generic_class_declarations_are_not_erased_inside_lua_blocks -- --nocapture` | Passed after AUDIT-017. Covers declarations nested in function, `if`, standalone `do`, `while`, `for` with nested `do`, and `repeat`; also verifies valid top-level declarations remain recognized after a block. |
| `cargo clippy -p tua-core --all-targets -- -D warnings`; CLI nested-protocol reproduction using `tua check` | Passed after AUDIT-017. Clippy is clean. The CLI no longer accepts/erases the nested protocol; it reports syntax diagnostics at the nested declaration. |
| `env CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 make ci-check` | Passed on the prior implementation revision. Re-run at final revision `b09ef990` stopped at `cargo audit` because the sandbox denied an exclusive lock under read-only `~/.cargo`; the remaining CI targets were run separately and passed. A copy of the existing advisory database was used for the final offline audit (see below). |
| `cargo audit --db /private/tmp/tua-audit-db-20261109 --no-fetch --stale` | Passed against the current lockfile (126 crate dependencies; 1,422 local advisories). No vulnerabilities; one allowed warning for unmaintained `paste 1.0.15` (RUSTSEC-2024-0436). The advisory database was copied from `~/.cargo/advisory-db` because its default parent is read-only in this environment. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 make fmt-check clippy test build-examples editor-regression-smoke version-drift-check release-tool-test config-schema-drift-check llms-drift-check llm-language-eval-test extension-check json-check docs-build benchmark-smoke diff-check` | All targets passed on final revision after rerunning `extension-check` with loopback permission. Includes workspace tests, example builds, editor regression smoke, docs, JSON/schema checks, representative LSP profiling, and inline completion evaluation. The initial sandboxed `extension-check` run failed because loopback bind was denied; the authorized retry passed 180/180. |
| `make extension-real-server-test` | Passed on installed local VS Code development extension after navigation/rename assertions were added. First runs exposed test cursor placement at the method token's end; changing it to an interior character made the provider test meaningful. |
| `make extension-package` | Passed after AUDIT-018. Package version synchronized at 0.7.0; extension check passed 180/180; npm audit reported 0 vulnerabilities; produced the tested 40-file, 8.64 MB VSIX with the updated bundled `tua` server. The sandboxed run could not bind loopback for runtime-inspector tests; the local-host retry passed. |
| `env TUA_TEST_VSIX="$PWD/editors/vscode/tua-0.7.0.vsix" npm --prefix editors/vscode run test:vscode-real` | Passed against the exact packaged VSIX on macOS arm64 / VS Code 1.142.0. Installs into an isolated profile and starts the bundled server; asserts protocol implementation navigation, generic binder definition/references/rename, imported generic hovers/completions/signatures and definitions, unsaved dependency edits, and concrete generic-parent hover/completion/signature substitution. `REAL_SERVER_EVIDENCE` reports zero diagnostics. The sandboxed Electron attempt terminated with SIGABRT; the local-host retry passed. |
| `make extension-package`, then `TUA_TEST_VSIX="$PWD/editors/vscode/tua-0.7.0.vsix" npm --prefix editors/vscode run test:vscode-real` | Passed for the rebuilt 0.7.0 artifact; extension checks passed 180/180 and npm audit reported zero vulnerabilities. The isolated VS Code run passed including recursive imported-protocol hover, completion, and clean diagnostics. `unzip -tq` passed; SHA-256 `65be27f16e0139dceaeb50bd8260063d16c432b811256ca267cd33268daf2655`. |
| `unzip -tq editors/vscode/tua-0.7.0.vsix && shasum -a 256 editors/vscode/tua-0.7.0.vsix` | Passed. The 0.7.0 VSIX archive is valid; SHA-256 is `8612fb2783b8c0eb197db74e34fe35bf704a4b97ea8ce7a5482898eaa012aea2`. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -q -p tua-core` | Passed after imported inherited-requirement diagnostics: 1,865 passed, 0 failed, 1 ignored. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-core imported_generic_class_alias_uses_runtime_constructor_and_shared_members -- --nocapture` | Passed. Project analysis resolves imported generic class field and method definitions to their exact declaration names in the provider module. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-core protocol_` | Passed: 47 protocol-focused core tests, including comment-separated headers and recovery when header comments contain braces, as well as contextual identifiers, explicit receivers, inherited overrides, and optional/variadic/multiple-return contracts. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-core --lib cyclic_imported_protocol_signatures_preserve_conformance_identity -- --nocapture` | Passed. The test mutates a protocol in a mutual-import cycle and asserts dependent conformance diagnostics and peer-protocol completion refresh from one snapshot. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-lsp` | Passed after the implementation change and deletion regression: 411 passed, 0 failed. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-lsp --lib deleting_a_protocol_dependency_refreshes_importing_class_diagnostics -- --nocapture` | Passed. After closing and deleting an imported protocol file and sending the LSP watcher event, dependent diagnostics report `TL4002` and no stale `TL1008` conformance diagnostic. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-lsp --lib renaming_a_protocol_dependency_refreshes_importing_class_diagnostics -- --nocapture` | Passed. The test deletes the old contract, creates and opens its renamed replacement, observes the unresolved old import, edits the consumer import, advances the scheduled diagnostic debounce, and observes the mismatch against the new imported requirement. A pull request inside the debounce window intentionally returns fast syntax diagnostics; waiting for scheduled full diagnostics is part of this verification. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -q -p tua-lsp` | Passed after imported inherited-requirement diagnostics: 413 passed, 0 failed; after protocol isolation coverage: 414 passed, 0 failed; after generic alias isolation coverage: 415 passed, 0 failed. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-lsp same_named_generic_aliases_stay_in_their_workspace -- --nocapture` | Passed. Two roots define same-named generic `Box<T>` aliases, one nullable and one non-null; hover and diagnostics remain project-local when workspace-folder order is reversed. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-lsp same_named_protocols_and_conformance_stay_project_local -- --nocapture` | Passed. Two workspace roots define `Updatable` with different parameter types; the incompatible implementation reports a linked error only against its own project's contract, while the other project remains clean. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-lsp --lib inherited_protocol_override_diagnostics_link_to_the_requirement -- --nocapture` | Passed. The real LSP diagnostics response contains the primary override range and a related location for its inherited protocol requirement. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -q -p tua-cli` | Passed after the override fix: 92 passed, 2 ignored. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -q -p tua-cli --test cli` | Passed after adding the generic-class traceback scenario: 134 passed, 0 failed. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-cli --test cli traceback_remaps_runtime_errors_inside_generic_class_methods -- --nocapture` | Passed on the available Lua runtime. A runtime failure in `Box<T>.fail` is mapped to `src/main.tua:9`, retaining its generated Lua location. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 cargo clippy -p tua-core -p tua-lsp --all-targets -- -D warnings` | Passed after the imported inherited-requirement fix. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-core inherited_override_diagnostics_link_to_imported_protocol_requirements -- --nocapture` | Passed. The core project diagnostic identifies both imported obligations inherited through a local base and links both declaration spans. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 cargo test -p tua-lsp inherited_override_diagnostics_link_to_imported_protocol_requirements -- --nocapture` | Passed. The LSP response preserves both related locations, their source URIs, and UTF-16 ranges. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 cargo test -p tua-core generic_function_multiple_return_slots_infer_independently` | Passed, 1 test. |
| `cargo test -p tua-cli --test cli generic_multiple_return_slots_preserve_lua_runtime_values` | Passed, 1 test, Lua 5.4.8. |
| `make docs-build` | Passed; standalone build reported no issues. |
| `make extension-screenshots-check` | Passed: 5 linked PNGs and manifest hashes/dimensions valid. Manifest is from 0.6.0; no protocol/generic UI layout changed, so gallery recapture was not needed for this language-only work. Visual review remains a release-prep task. |
| `unzip -tq editors/vscode/tua-0.7.0.vsix` | Passed: no archive errors in the exact beta-test package. |
| `shasum -a 256 editors/vscode/tua-0.7.0.vsix` | `6660c884990ec5a87e46463305a79b595104ef99d1298e6d36ed097b408bbba7` (latest package, version 0.7.0, includes AUDIT-018). |
| Final representative LSP smoke profile (`make benchmark-smoke`) | Passed on final revision. `single-1000`: cold completion p95 258.61 ms, warm 6.86 ms; contention completion p95 160.71 ms. `project-100`: cold 115.93 ms, warm 0.46 ms; contention p95 28.41 ms. Memory sampling completed and stayed within the configured 1,024 MiB limit. This smoke used one cold sample and is not a release-host baseline. |
| `cargo fmt --all -- --check`, `git diff --check` | Passed after implementation and test changes. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 cargo clippy -p tua-core -p tua-lsp --all-targets -- -D warnings` | Passed after the malformed declaration parser/lowering fix. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=33554432 RUST_TEST_THREADS=1 make verify` | Passed after AUDIT-023. Core 1,882 passed/1 ignored; LSP 426 passed; CLI library 92 passed/2 ignored; CLI integration 137 passed; gameplay 3 passed; extension 180/180. Clippy, schema/version/config checks, language evaluation, JSON validation, and diff checks passed. A default-stack attempt overflowed in the known stack-sensitive emitter test; the documented serial larger-stack run passed. |
| `make extension-package`, installed `test:vscode-real`, `unzip -tq`, and `shasum -a 256` | Passed after AUDIT-023. The exact 0.7.0 VSIX contains 40 files (8.68 MB), npm audit reports 0 vulnerabilities, and isolated VS Code 1.142.0/macOS arm64 real-server tests pass with the bundled server. Archive validation passes. SHA-256: `43568a26a7d0a9532fa4e6c81e48c33d9d84b601598f659b249ca0cbf92cbe98`. |
| `make extension-package` | Passed at the November 10 re-audit revision. Built Tua 0.7.0 VSIX (40 files, 8.65 MB); extension tests 180/180 and npm audit 0 vulnerabilities. |
| `env TUA_TEST_VSIX="$PWD/editors/vscode/tua-0.7.0.vsix" npm --prefix editors/vscode run test:vscode-real` | Passed against the exact rebuilt package on VS Code 1.142.0/macOS arm64 with the bundled server. The isolated-profile real-server suite covers cyclic protocol returns (`local target: B`, `B.previous` completion), recursive argument hover/completion/docs, imported generic behavior, and protocol stub action. `unzip -tq` passed. Current VSIX SHA-256: `d3fb8703ce8893cbb119bc6a9dc68f9085a928d7eeeeeccaa3c7407730806de0`. |
| Absolute-path imported-protocol generic-bound CLI reproduction and its CLI/LSP regression tests | Passed after `d7c56d50`: the explicitly conforming imported `Actor` has no TL1015; the nonconforming same-shape `Lookalike` retains TL1015. A redundant type import initially triggered the separate unused-import lint and was removed from the fixture in `580307ea`. |
| AUDIT-028 continuation: `RUST_MIN_STACK=33554432 cargo test -p tua-core`; `cargo test -p tua-lsp`; `make extension-package`; exact VSIX `test:vscode-real`; large generic completion reproduction | Passed. Core 1,889 passed/1 ignored; LSP 427 passed; extension 180/180; npm audit 0 vulnerabilities. The 4,200-field core completion test passed in 0.30 seconds; the stdio LSP fixture returned 11 matching candidates in 0.40 seconds. The exact 0.7.0 VSIX installed into an isolated profile and passed its real-server suite on VS Code 1.142.0/macOS arm64. Final artifact SHA-256: `977162265a6b6be4a8b46cbaa7dfd90f9491dc756de1b6fbea9118bbd760a0a3`. |
| AUDIT-029/030: `cargo test -p tua-lsp` | Passed: 431 tests. Includes direct protocol signature mismatch ranges, inherited override ranges with related protocol declarations, unsaved consumer conformance/method removal and restoration, and multi-hop re-export change/removal/restoration. |
| `cargo clippy -p tua-lsp --all-targets -- -D warnings`; `cargo fmt --all -- --check`; `git diff --check` | Passed after AUDIT-029/030. |

The VS Code/Electron tests require local host access for the editor and language server. A sandboxed first attempt terminated Electron with `SIGABRT`; rerunning with the approved local host access completed and passed. This was a test-environment restriction, not a Tua assertion failure.

## Prioritized backlog (reconciled November 10, 2026)

No confirmed correctness defect remains open in the scoped 0.7.0 feature set.
The following are explicit scope boundaries or qualification work, not hidden
implementation tasks:

| Priority | Category | State | Next evidence or decision |
| --- | --- | --- | --- |
| 1 | External runtime/platform qualification | Pending environment | Run on Windows, Linux, Intel Mac, supported LOVE/LuaJIT versions, and a second LSP client. The current host has Lua 5.4.8 but no LOVE or LuaJIT. |
| 2 | Long-session memory and live-editor performance | Pending qualification | Repeat an installed-editor real-game session with retained-memory profiling and multiple cold/warm samples. Current benchmark profile is recorded in this final audit addendum; it has no pre-change baseline. |
| 3 | Visual LOVE/editor review | Pending environment | Run the game and review rendered behavior and extension UI on a host with LOVE and VS Code. Protocol/generic runtime behavior has system-Lua tests; these do not replace LOVE rendering. |
| 4 | Cross-module class inheritance | Deliberately outside this contract | Only pursue after a separate design for superclass resolution, initialization/`super`, cycles, invalidation, navigation, and diagnostics. Generic class inheritance across modules remains supported for the documented imported-instance workflow. |
| 5 | Broader language abstractions | Deliberately unsupported | Generic methods/protocols, compound bounds, callback-only or return-only generic inference, and explicit call-site type arguments remain outside the documented surface. |
| 6 | Exhaustive malformed-input and scheduler interleavings | Non-exhaustive test boundary | The recovered protocol/class header shapes, UTF-8 boundaries, later declarations, stale dependency results, and create/delete/edit cases have regressions. The test suite does not enumerate every arbitrary edit sequence; no defect was reproduced in the covered cases. |

Recommended sequence for future work: complete host-dependent qualification, then
make a separate cross-module inheritance design decision. Do not describe the
current host-specific VSIX as cross-platform or LOVE-qualified.

## Reproduction fixture

The generic multi-return regression is now permanent in the test suite. For a standalone copy, create `src/main.tua`:

```tua
local function split<T, U>(left: T, right: U): (T, U)
  return left, right
end

local text, count = split("enemy", 3)
local _text_check: string = text
local _count_check: number = count
```

Use the repository's standard `tua.toml` (`source = "src"`, `out = "build"`) and run:

```sh
tua check
tua build
lua build/main.lua
```

For installed-editor qualification, run:

```sh
make extension-package
env TUA_TEST_VSIX="$PWD/editors/vscode/tua-0.7.0.vsix" npm --prefix editors/vscode run test:vscode-real
```

This test uses an isolated VS Code profile and does not install into the user's normal profile.

## Final audit reconciliation — November 10, 2026

This section supersedes earlier status snapshots in this report and the
November 9 phase table. It was checked against the current `main` tree at
`d5058b1d` and `make verify` was rerun with loopback access. The scoped
implementation is complete: this audit found no confirmed open compiler,
project-analysis, LSP, lowering, or VS Code defect in the documented 0.7.0
contract. This statement is limited to tested behavior; it is not a claim of
exhaustive proof over all possible programs or editor event interleavings.

| Area | Final status | Evidence and limits |
| --- | --- | --- |
| Protocols and polymorphism | Implemented for the documented method-only, explicit-conformance contract | Local and imported conformance, inherited local implementations, multiple obligations, variance, protocol calls, recursive signatures, structured diagnostics, source ranges, related locations, and runtime dispatch are covered. Cross-module class inheritance is deliberately outside the contract. |
| Generic aliases/functions/classes | Implemented for the documented slice | Substitution, imports/re-exports, recursion limits, direct and contextual inference, callbacks, multi-return slots, constructor/member/parent substitution, protocol bounds, and editor queries have core/CLI/LSP coverage. Generic methods/protocols, compound bounds, callback-only/return-only inference, and explicit call-site arguments are deliberately unsupported. |
| Malformed edits and dependency freshness | Exercised, not exhaustive | Protocol and generic-class header recovery, comment-aware boundaries, UTF-8 prefix handling, later declarations, last-good facts, create/delete/rename/edit, cyclic imports, workspace changes, and an in-flight stale diagnostic result are tested. Arbitrary edit streams and all cancellation interleavings are not exhaustively enumerated; no defect was reproduced in the covered cases. |
| Editor behavior | Verified on this host | LSP suite and 180 VS Code checks pass. The exact packaged VSIX is installed and tested with a real server on macOS arm64. Other OSes and LSP clients were not available for this run. |
| Runtime and emission | Verified with system Lua; LOVE remains unverified | Generated examples, protocol dispatch, generic results, Lua syntax, source maps, and generic-class traceback mapping pass. This host has Lua 5.4.8; `love` and `luajit` are unavailable, so LOVE/LuaJIT, visual behavior, and hot reload inside a running game were not exercised here. |
| Performance and memory | Representative current-host profile completed; release qualification remains open | The five-cold/twenty-warm/twenty-contention release profile is recorded below. It has no pre-change baseline. A long-session VS Code retained-memory run is still needed. |

`make verify` passed with local loopback access: 1,910 core tests passed (1
ignored), 446 LSP tests, 93 CLI unit tests passed (2 ignored), 137 CLI
integration tests, 3 gameplay tests, 1 helper-default test, 3 LOVE-example
build tests, 1 LSP smoke test, 1 Lua syntax test, and all 180 VS Code checks.
Workspace Clippy, version/schema checks, language reference and mutant
validation, JSON checks, and `git diff --check` passed. The three LOVE example
tests build successfully but skip launching LOVE when it is unavailable.

The restricted run initially failed only because the VS Code inspector checks
could not bind `127.0.0.1` (`listen EPERM`). The same full command was rerun with
local loopback access and passed. Representative profiling likewise required
local process sampling; the sandboxed attempt failed with `spawn EPERM` before
recording samples, and the loopback/process-enabled retry completed.

The profile ran against the optimized CLI on this macOS arm64 host with five
cold, twenty warm, and twenty contention samples. It completed `single-1000`,
`single-5000`, and `project-100`; their p95 values are recorded in PERF-001.
The attempt to continue through `project-1000` was interrupted after 270
seconds at a sampled 3.75 GiB RSS, before the profiler wrote a complete JSON
report. No comparable pre-change baseline is available, and this protocol
profile is not a substitute for installed-editor key-to-menu latency or
long-session memory qualification.

The following remain explicitly outside local completion and must not be
misrepresented as unfinished 0.7.0 implementation bugs: cross-module class
inheritance is a separate design decision; generic methods/protocols,
compound bounds, callback-only or return-only inference, and explicit call-site
type arguments are unsupported by the documented contract. Platform/runtime,
visual, second-client, and long-session checks require their respective
external environments. A final VSIX checksum and installed-package result are
recorded after packaging below.

### PERF-001 — Synthetic 1,000-module LSP workspace did not finish profiling

- **Category / severity:** Performance and memory scalability; High for the
  tested stress workload, not established for ordinary projects.
- **Confidence / status:** Confirmed observed symptom; root cause unresolved.
  This is the one new issue found by the final re-audit, outside language
  correctness.
- **Reproduction command:**

  ```sh
  TUA_REPRESENTATIVE_COLD_REPEAT=5 \
  TUA_REPRESENTATIVE_WARM_REPEAT=20 \
  TUA_REPRESENTATIVE_CONTENTION_REPEAT=20 \
  make profile-representative
  ```

  The profile visits the generated single-file, multi-module, and dependency
  graph fixtures in sequence. On `project-1000` it continued for 270 seconds
  without producing a completed profile result, so I stopped it. A process
  sample during that stage showed the active `target/release/tua lsp` at
  3,931,696 KiB RSS (about 3.75 GiB) and 298.6% CPU. The command emits its
  final JSON only after all selected fixtures finish; this interrupted run
  therefore has no complete profile artifact.
- **Completed fixture results (p95):**

  | Fixture | Completion cold / warm | Hover cold / warm | Contention completion | Diagnostics after edit |
  | --- | ---: | ---: | ---: | ---: |
  | `single-1000` | 267.54 / 8.59 ms | 274.47 / 23.21 ms | 162.14 ms; 0/640 canceled, 0 stale | 78.97 ms |
  | `single-5000` | 1,584.78 / 153.84 ms | 1,524.20 / 100.29 ms | 1,102.82 ms; 1/640 canceled, 0 stale | 470.56 ms |
  | `project-100` | 122.77 / 0.42 ms | 126.65 / 1.60 ms | 32.07 ms; 2/640 canceled, 0 stale | 17.72 ms |

  `project-100` has 100 files and 1,401 lines; `single-1000` and
  `single-5000` are one source file with exactly the named line count. The
  single-5000 values show that latency also rises substantially with one very
  large file. This suggests scale issues in the tested stress shapes, but does
  not establish a threshold or whether the cause is generic/protocol analysis.
- **Expected:** A large workspace should either warm and answer requests within
  a documented resource budget, or report bounded progress/cancellation rather
  than retain several GiB without a usable profile response.
- **Likely cause:** Not established. The profile opens the entry document while
  the server discovers and warms the project; separate parse, semantic,
  last-good, and snapshot ownership are candidate areas to measure, not a
  verified root cause.
- **Recommended next step:** Capture allocator/heap and per-phase workspace-warm
  data on this fixture, compare against the 100-module case, then reduce the
  measured largest retained structures. Add a stress regression with an agreed
  memory ceiling and a completion/freshness acceptance bound. Do not patch
  caching heuristically from RSS alone.
- **Related evidence:** The same profile completed `single-1000`,
  `single-5000`, and `project-100`; values and setup are summarized above.
  `project-1000` and the later dependency-graph fixtures were not completed.

  A narrowed follow-up used only `project-1000`, one cold/warm/contention
  round, and a four-request background burst. It completed workspace warming
  in 1,883.60 ms; one-sample warm completion was 9.04 ms, while cold
  completion/hover/definition/inlay requests were 2.53/2.64/6.55/2.95 seconds.
  Peak sampled RSS was 1,106 MiB. All four contention requests were canceled,
  so the script exited with `no valid-document contention completion was
  checked`; this does not qualify responsiveness under churn. A separate CLI
  `tua check` of the generated 1,000-file project completed in 6.33 seconds.
  This narrows the symptom to LSP cold/query/contention behavior in the tested
  fixture, but does not establish the cause.

The exact packaged `editors/vscode/tua-0.7.0.vsix` passed installation and the
isolated real-server VS Code test on VS Code 1.142.0/macOS arm64. The newest
package evidence is recorded below.

### Emitted Lua runtime audit — November 10, 2026

To check that accepted generic/protocol programs retain their intended runtime
behavior after lowering, two CLI integration fixtures build complete projects,
inspect generated modules for erased type-only syntax and preserved runtime
requires, then execute the generated entry point with the host Lua interpreter.
The tests are:

- `emitted_lua_preserves_imported_generic_protocol_runtime_behavior`: imports
  an exported protocol and generic class/instance alias across modules; checks
  the type-only module does not become a Lua `require`; verifies left-to-right
  call argument evaluation, generic multiple returns, a bounded generic
  function, imported generic construction and method chaining, callback
  argument compatibility, explicit nil return slots, typed varargs, and
  heterogeneous protocol-array virtual dispatch.
- `emitted_lua_preserves_generic_override_and_initialization_order`: verifies
  concrete generic-parent erasure, `super` initialization, field-default
  initialization order, inherited generic methods, and virtual override
  dispatch through a base-typed variable.

Both tests passed under Lua 5.4.8. Related runtime integration coverage also
passed for array invariance, generic multiple-return values, protocol dispatch,
concrete generic parents, generic class traceback mapping, and full gameplay
examples. The build path runs the emitter's Lua 5.1 syntax validation, and the
workspace's `emitted_example_lua_passes_syntax_validation` corpus passed too.
Reproduce the two focused runtime checks with:

```sh
cargo test -p tua-cli --test cli emitted_lua_preserves_imported_generic_protocol_runtime_behavior
cargo test -p tua-cli --test cli emitted_lua_preserves_generic_override_and_initialization_order
```

Full verification on this source state passed via `make verify`: 1,910 core
tests passed (1 ignored), 446 LSP tests, 93 CLI unit tests passed (2 ignored),
139 CLI integration tests, 3 gameplay tests, 1 helper-default test, 3 example
tests, 1 LSP smoke test, 1 Lua-syntax test, and 180 VS Code checks. Clippy,
version/schema checks, reference and mutant language evaluation, JSON checks,
and `git diff --check` also passed. The 3 LOVE example tests build, but LOVE
was not installed, so they did not launch a game. LuaJIT and LOVE execution
were unavailable; this is evidence for the emitted semantics on PUC Lua 5.4.8
and the repository's Lua 5.1 syntax parser, not universal runtime proof.

`make extension-package` then rebuilt the extension and bundled release-mode
server. It passed all 180 extension checks, found zero npm vulnerabilities, and
produced a 40-file VSIX. `unzip -tq` passed. The exact package was installed in
an isolated VS Code profile and passed `test:vscode-real` against VS Code
1.142.0 on macOS arm64, including cross-file generic/protocol hover, definition,
signature, edits, and navigation checks. SHA-256:
`3d0aacc98c9759d2c25c9af80ef6ff7571806ab7efd83adf82a4f966a9d6e218`.

The two CLI runtime regression tests are in `crates/tua-cli/tests/cli.rs`;
no product source was changed in this runtime audit. This verification
establishes confidence for the covered constructs,
not a claim that all possible generated Lua is 100% reliable.
