# Tua 0.7.0 Generics and Protocols Final Audit

Date: 2026-11-09  
Audited base: `06c1065b940622eee2f179b4c7752ce496010b8c`
Final implementation revision: `dc3413d1`; installed-editor regression-test
revision: `f209b882`; latest protocol dependency-rename test revision:
`0dd22027`; final multi-workspace protocol-isolation test revision:
`24383678`; latest generic multi-workspace isolation test revision:
`a092ce8e`; generic-class source-map regression revision: `25320824`. The
final implementation revisions add imported inherited protocol-requirement
links to override diagnostics, safely diagnose malformed nested
protocol/class declaration text, and resolve generic member definitions
across modules. This report records verification after
protocol-header, diagnostic, documentation, cycle, dependency-deletion,
dependency-rename, inherited-requirement, multi-workspace isolation, and
malformed-declaration fuzz coverage.
Scope: `internal/generics-polymorphism-0.7.0.md`, protocol contracts, native generics, type inference, editor surfaces, Lua erasure, and the locally shareable VS Code extension.  
Working tree before audit: clean. No unrelated project changes were present.

## Executive summary

The documented 0.7.0 slice has strong compiler, project-analysis, LSP, runtime, and installed-editor coverage. The audit fixed the listed parser, inference, documentation, and diagnostic issues; this continuation also fixes inherited override links when a local base class implements an imported protocol, malformed declaration text that could panic during lowering, missing Go to Definition for imported generic class members, and nested class/protocol text being erased as a top-level declaration. The audit added regressions for these behaviors, generic multi-return checker/runtime coverage, and installed VS Code protocol implementation navigation, generic binder references, rename, and unsaved dependency-edit coverage.

The protocol/generic implementation is suitable for **manual beta testing on macOS arm64**. The latest packaged VSIX (40 files, 8.64 MB), including incomplete-protocol recovery, protocol-requirement diagnostic links, commented protocol-header parsing, inherited-override diagnostic links, malformed nested-declaration recovery, and module-scope diagnostics for nested Tua declarations, was tested in an isolated VS Code profile and passed the real-server suite using its bundled compiler. The exact package passed archive validation and has SHA-256 `bc5050e55b8f58e2206428859bb3e1d840e91652ec6773d2b6d953ae5c181e4e`. It is not qualified for a general 0.7.0 release: Linux, Windows, Intel Mac, LuaJIT/LOVE, source-map/hot-reload, visual screenshot review, and long-session checks remain open. A local class hierarchy can inherit an imported protocol's requirements; cross-module class inheritance itself remains outside the parser/lowering contract.

Repeated local profile measurements (five cold samples, twenty warm and contention samples) on the 1,000-line fixture show cold completion p95 251.10 ms, warm completion p95 6.24 ms, cold hover p95 266.65 ms, warm hover p95 20.03 ms, and completion p95 under contention 153.03 ms. The 100-module project reports cold completion p95 116.04 ms, warm completion 0.40 ms, and warm hover 1.66 ms. These have no before/after baseline; cold latency and large-project contention remain performance opportunities.

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
- **Evidence:** G3 supports local generic inheritance and imported generic classes, but cross-module class inheritance is not currently in the parser/lowering contract. Therefore inherited protocol conformance cannot be qualified through an imported base class.
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

## Coverage matrix

Status key: **Exercised** means a focused regression or real request/runtime test ran; **Inspected** means code/docs were reviewed but not independently executed for that specific edge; **Pending** means no qualifying environment or scenario was available.

| Plan area | Status | Evidence and limits |
| --- | --- | --- |
| P0 protocol syntax, nominality, colon receiver, mutable views | Exercised | Positive/negative assignments, colon calls, explicit dot/extracted receivers, class array mutation safety, nominal identity, inherited overrides, and contextual identifier compatibility are covered. |
| P1 parsing, identities, spans, formatting and erasure | Exercised, partial | Local/exported/multiline and comment-separated protocol headers, comments containing braces during recovery, spans, formatting, diagnostics, Lua erasure, contextual words, editor recovery of unclosed protocol bodies, malformed nested class/protocol cases, and declaration rejection inside Lua function/conditional/loop/do/repeat blocks pass. Current-buffer queries retain partial declarations while dependency indexes keep last-good contracts. Broader malformed-edit recovery remains incomplete. |
| P2 conformance, variance, multiple obligations, inheritance | Exercised, partial | Positive/negative local/imported conformance, inherited satisfaction/override, same-name obligations, optional parameters, varargs, multiple return slots/counts, variance, exact override/parameter/return locations, related locations for imported direct conformance, and multiple imported requirements inherited through local bases are tested. Broader malformed-edit and uncommon override/range combinations remain open. Cross-module class inheritance is unsupported by the current class contract. |
| P3 module identity, re-exports, cycles, edits | Exercised, partial | Named/namespace/re-export and cyclic protocol identity pass. Editing one protocol inside a mutual import cycle refreshes implementing-class diagnostics while completion through the peer protocol remains available in the same analysis snapshot. Saved/unsaved refresh tests pass. Deleting a contract invalidates its last-good facts, reports the missing import, and removes stale conformance checks. Renaming a contract, changing the consumer import, and requesting diagnostics after the debounce reports the new mismatch. LSP tests prove same-named protocols and generic aliases with different contracts remain isolated across workspace roots in either root ordering. Workspace-folder changes reindex open-document analysis and republish push diagnostics; add/replace-root and broad dependency graph edit patterns are not exhausted. |
| P4 completion, hover, signatures, navigation, rename | Exercised on macOS, partial | Core/LSP suites and installed VSIX cover protocol completion, hover, signature, definition, implementation navigation, references/rename; generic alias/class/function completion/hover/signatures, alias and imported function/class member definitions, generic binder definition/references/rename, and unsaved dependency edits that update and restore dependent generic hover types. Cross-platform qualification remains open. |
| P5 examples, generated Lua, runtime/source maps/hot reload | Exercised, partial | Feature/game examples build; gameplay polymorphism and generic multi-return emitted Lua execute on the available Lua runtime. A new end-to-end test builds a generic class and maps a runtime error from its method to the exact Tua source line. CLI and extension suites also cover source maps and hot reload. LOVE/LuaJIT runtime qualification and visual behavior remain pending. |
| G0 design contract and limits | Inspected | Decisions are present in plan and user docs; future generic methods, generic protocols, packs, explicit call-site args, compound bounds, and advanced LuaCATS are explicit limitations. |
| G1 aliases, identity, substitution, recursion, imports | Exercised, partial | Exact arity, unknown names, capture-safe substitution, imported/re-exported aliases, same-argument recursion, and growing-argument recursion are tested. The first repeated declaration stays symbolic with substituted arguments; other alias cycles retain work/depth guards. Exhaustive adversarial alias graph limits remain untested. |
| G2 functions, inference, callbacks, LuaDoc | Exercised, partial | Independent calls, body checking, conflicts, nil/unions/empty table, callback contexts/ambiguity, bounds, varargs, LuaDoc generic forms, multi-return slot inference/runtime are covered. Callback-only/return-only inference and explicit callsite args are deliberately unsupported. |
| G3 classes, constructors, substitution, bounds | Exercised, partial | Local/imported generic instances, constructor/member inference, applied generic parents, `super`, invariant containers, protocol bounds, hover/completion/navigation covered. An imported protocol inherited through a local base now links from a child override. Cross-module class inheritance and generic methods/protocols remain unsupported. |
| Real editing and snapshots | Exercised, partial | Unsaved dependency edits, cyclic imports, incomplete consumer edits, warm snapshots and watched changes have core/LSP coverage. Long-session and broad dependency churn remain open. |
| UTF-16/editor client integration | Exercised | LSP UTF-16 and cross-file navigation/rename suites pass; exact VS Code test passes on macOS arm64. Other clients and platforms pending. |
| Performance and memory | Exercised, partial | Repeated five-cold/twenty-warm profile completed with p50/p95 for single-file and project fixtures; no before/after baseline, dependency-churn timing or long-session memory qualification. |
| Release artifact/package | Exercised, partial | `make extension-package` passes (180/180 extension tests, npm audit 0 vulnerabilities), VSIX archive validates, exact current VSIX installs and runs in isolated VS Code. No signing, publishing, or external beta distribution performed. |

## Verification log

All commands ran from repository root unless noted.

| Command | Outcome |
| --- | --- |
| `env CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 RUST_TEST_THREADS=1 make verify` | Passed after AUDIT-018. Formatting, workspace tests (core 1,869 passed/1 ignored; LSP 417; CLI unit 92 passed/2 ignored, integration 134; gameplay 3; examples and syntax checks passed), Clippy, version/release/config/schema/LLM checks, language evaluation, extension tests (180/180), JSON validation, and diff checks all passed. This run used local host access for extension tests that need loopback. The larger Rust test stack is required by a known deeply nested core test. |
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
| `env TUA_TEST_VSIX="$PWD/editors/vscode/tua-0.7.0.vsix" npm --prefix editors/vscode run test:vscode-real` | Passed against the exact packaged VSIX after AUDIT-018 on macOS arm64 / VS Code 1.142.0. Installs into an isolated profile and starts the bundled server; asserts protocol implementation navigation, generic binder definition/references/rename, imported generic hovers/completions/signatures and definitions, and unsaved generic-alias dependency edits. `REAL_SERVER_EVIDENCE` reports zero diagnostics. The sandboxed Electron attempt terminated with SIGABRT; the local-host retry passed. |
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
| Final representative LSP smoke profile (`make benchmark-smoke`) | Passed on final revision. `single-1000`: cold completion p95 255.24 ms, warm 6.65 ms; contention completion p95 157.80 ms. `project-100`: cold 115.93 ms, warm 0.40 ms; contention p95 29.80 ms. Memory sampling completed and stayed within the configured 1,024 MiB limit. This smoke used one cold sample and is not a release-host baseline. |
| `cargo fmt --all -- --check`, `git diff --check` | Passed after implementation and test changes. |
| `CARGO_INCREMENTAL=0 RUST_MIN_STACK=16777216 cargo clippy -p tua-core -p tua-lsp --all-targets -- -D warnings` | Passed after the malformed declaration parser/lowering fix. |

The VS Code/Electron tests require local host access for the editor and language server. A sandboxed first attempt terminated Electron with `SIGABRT`; rerunning with the approved local host access completed and passed. This was a test-environment restriction, not a Tua assertion failure.

## Prioritized backlog

| Priority | Group | Effort | Acceptance criteria |
| --- | --- | --- | --- |
| 1 | Cross-module inheritance and inherited protocol conformance design | L | A written contract covers runtime initialization/`super`, nominal identity, imports, cycles/invalidation, navigation, and diagnostics before parser or emitter work begins. |
| 2 | Cold and long-session performance qualification | M | Repeated release-host cold/warm p50/p95, dependency-edit timing, peak/retained memory, and a documented comparison against an agreed baseline. |
| 3 | Runtime/platform qualification | M/L | Linux, Windows, Intel Mac, Apple Silicon CI; supported LuaJIT/LOVE smoke; generated behavior and source maps checked; hot reload and long-session results recorded. |
| 4 | Finish broad protocol malformed-edit, override, and diagnostic-range matrix | M | Table-driven tests cover partial headers, nested comments/types, all inherited overrides, cancellation and exact primary/related ranges across local and imported declarations. |
| 5 | Generic installed-editor parity expansion | S/M | Completed in this continuation: installed VSIX now checks unsaved generic dependency type refresh/restoration and cross-file generic function and class-member navigation. |

Recommended sequence: design cross-module class extension separately; expand performance baselines and runtime/platform gates; complete protocol edge-case coverage. Keep the current scoped 0.7.0 beta contract explicit until those workstreams land.

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
