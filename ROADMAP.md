# tua v1 Commit Roadmap

This roadmap breaks `SPEC.md` into small, committable implementation slices.
Each commit should leave the repository buildable, tested for the behavior it
adds, and free of unrelated refactors.

The roadmap assumes the v1 target from `SPEC.md`: `.tua` source files compile
to readable Lua 5.1-compatible output for LuaJIT/LÖVE, with erased static types
and no runtime support library.

## Commit Rules

- Keep each commit focused on one behavior or subsystem.
- Include tests with the commit that introduces behavior.
- Do not commit generated `build/` output except explicit test fixtures.
- Prefer readable compiler output over clever optimization.
- Preserve normal Lua `require` semantics throughout the project.
- Treat Luau as a reference, not a runtime dependency.

## Phase 0: Repository Foundation

### 1. `docs: add tua v1 specification`

Status: done.

Scope:

- Add `SPEC.md` as the v1 language and toolchain contract.

Acceptance:

- The spec defines language syntax, compiler behavior, CLI commands, editor
  tooling, non-goals, and `.tua` to `.lua` examples.

### 2. `docs: add committable implementation roadmap`

Status: current.

Scope:

- Add this roadmap.
- Organize future implementation into independently reviewable commits.

Acceptance:

- The roadmap can be followed without making major implementation decisions.

## Phase 1: Rust Workspace And CLI Shell

### 3. `chore: scaffold rust workspace`

Scope:

- Add a Cargo workspace.
- Create initial crates:
  - `crates/tua-core`
  - `crates/tua-cli`
  - `crates/tua-lsp`
- Add a root `README.md` with a short project description and links to
  `SPEC.md` and `ROADMAP.md`.

Acceptance:

- `cargo test --workspace` passes.
- `cargo run -p tua-cli -- --help` prints command help.

### 4. `feat(cli): add command structure`

Scope:

- Add `tua` CLI commands:
  - `init`
  - `check`
  - `build`
  - `watch`
  - `lsp`
- Add `build --emit-on-error`.
- Commands may return placeholder "not implemented" results except `--help`.

Acceptance:

- CLI argument parsing is tested.
- Unknown commands and invalid flags produce useful errors.

## Phase 2: Project Config And Diagnostics

### 5. `feat(config): load tua project configuration`

Scope:

- Define `tua.toml` config with defaults:
  - `source = "src"`
  - `out = "build"`
  - `target = "love11-luajit"`
  - `extension = "tua"`
  - `emit_maps = true`
- Implement config discovery from the current directory.
- Implement `tua init` to create default config and `src/`.

Acceptance:

- Config loading is tested for present, missing, and partial configs.
- `tua init` does not overwrite existing config unless a future explicit flag
  is added.

### 6. `feat(diagnostics): add shared diagnostic model`

Scope:

- Add a shared diagnostic type with file, line, column, severity, message, and
  optional code.
- Reserve code ranges from `SPEC.md`:
  - `TL0001`-`TL0999`: syntax
  - `TL1000`-`TL1999`: type errors
  - `TL2000`-`TL2999`: lowering/emission
  - `TL3000`-`TL3999`: project config
- Add CLI formatting like:
  `src/player.tua:12:15 error TL1002: type mismatch: expected number, found string`

Acceptance:

- Diagnostic formatting has snapshot tests.
- CLI commands can return diagnostics without panicking.

## Phase 3: Parsing And Source Model

### 7. `feat(parser): parse lua-compatible tua sources`

Scope:

- Integrate a Lua parser foundation.
- Parse ordinary Lua-compatible `.tua` files.
- Preserve source spans for diagnostics and later emission.

Acceptance:

- Parse tests cover locals, functions, tables, returns, and `require`.
- Syntax errors produce `TL000x` diagnostics with source locations.

### 8. `feat(parser): parse erased type syntax`

Scope:

- Parse v1 type aliases.
- Parse local annotations.
- Parse function parameter and return annotations.
- Parse table shape, optional field, and union type syntax.

Acceptance:

- Parser tests cover every type syntax form in `SPEC.md`.
- Type-only syntax is represented in the core AST or typed source model.

### 9. `feat(parser): parse compound assignment syntax`

Scope:

- Parse `+=`, `-=`, `*=`, `/=`, and `%=`.
- Restrict the accepted left-hand side shape to names and plain field chains.

Acceptance:

- Valid compound assignments parse.
- Calls, computed indexes, and other complex left-hand sides produce diagnostics.

## Phase 4: Erasure And Emission

### 10. `feat(emit): erase type-only syntax`

Scope:

- Emit Lua with type aliases removed.
- Emit locals and functions without type annotations.
- Preserve ordinary Lua statements and expressions.

Acceptance:

- Snapshot tests compare `.tua` input to emitted `.lua`.
- Type aliases emit no runtime code.

### 11. `feat(emit): lower compound assignment`

Scope:

- Lower supported compound assignments to Lua 5.1-compatible assignments.
- Keep field-chain lowering readable:
  `player.hp -= amount` becomes `player.hp = player.hp - amount`.

Acceptance:

- Snapshot tests cover all supported compound operators.
- Unsupported left-hand sides produce `TL200x` diagnostics.

### 12. `feat(build): mirror source tree to output tree`

Scope:

- Discover `src/**/*.tua`.
- Emit matching `build/**/*.lua`.
- Preserve relative paths.
- Preserve `require(...)` calls unchanged.

Acceptance:

- Integration tests compile nested source files.
- Output paths match source paths with only the extension changed.

## Phase 5: Type System Core

### 13. `feat(types): add v1 type representation`

Scope:

- Represent primitive types, `any`, aliases, unions, function types, table
  shapes, and optional fields.
- Add assignability rules for v1.

Acceptance:

- Unit tests cover primitive assignability, `any`, unions, optional fields, and
  table shape compatibility.

### 14. `feat(types): collect aliases and annotations`

Scope:

- Build per-file type environments.
- Resolve aliases within a file.
- Collect local and function annotations.

Acceptance:

- Unknown type names produce `TL100x` diagnostics.
- Duplicate type aliases produce a deterministic diagnostic.

### 15. `feat(check): check annotated locals and assignments`

Scope:

- Infer primitive literal types.
- Check annotated local initializers.
- Check assignments to known typed locals and known table fields.

Acceptance:

- Tests cover successful assignments and primitive/table-field mismatches.
- Unannotated dynamic code remains permissive.

### 16. `feat(check): check function boundaries`

Scope:

- Check annotated function arguments at call sites when the callee type is known.
- Check return statements against annotated return types.

Acceptance:

- Tests cover argument count, argument type, and return type diagnostics.
- Calls through `any` remain permissive.

## Phase 6: CLI Behavior

### 17. `feat(cli): implement check command`

Scope:

- Wire config loading, source discovery, parsing, and type checking into
  `tua check`.

Acceptance:

- `tua check` returns `0` for clean projects.
- `tua check` returns non-zero for parse or type errors.
- Diagnostics include file, line, column, severity, and message.

### 18. `feat(cli): implement build command`

Scope:

- Wire checking, lowering, erasure, and output writing into `tua build`.
- Block emission on errors by default.
- Implement `--emit-on-error` for parseable/lowerable files.

Acceptance:

- `tua build` writes mirrored Lua output for valid projects.
- Type errors block normal builds.
- `--emit-on-error` emits possible files and still exits non-zero.

### 19. `feat(cli): implement watch command`

Scope:

- Watch configured source directory.
- Run an initial build.
- Debounce source changes.
- Re-check and re-emit changed files.

Acceptance:

- Watch behavior has integration coverage around rebuild triggering where
  practical.
- Manual run shows stable rebuild output without duplicate noisy events.

## Phase 7: LÖVE Example And Runtime Validation

### 20. `test(love): add minimal love example`

Scope:

- Add `examples/love-minimal`.
- Include a tiny `.tua` module and `main.tua`.
- Build to Lua and run with LÖVE where available.

Acceptance:

- The emitted example runs under LÖVE 11.x.
- CI or local tests skip gracefully when LÖVE is unavailable.

### 21. `test(lua): validate emitted lua syntax`

Scope:

- Add syntax validation for emitted Lua.
- Prefer LuaJIT when available.
- Allow fallback to installed Lua for syntax-only checks when compatible.

Acceptance:

- Emitted fixtures pass syntax validation.
- Missing LuaJIT is reported as skipped, not failed, unless the environment
  explicitly requires it.

## Phase 8: Language Server

### 22. `feat(lsp): serve diagnostics`

Scope:

- Implement `tua lsp` over stdio.
- Publish diagnostics using the shared parser/checker.

Acceptance:

- LSP initialization succeeds.
- Opening or changing a `.tua` document returns matching CLI diagnostics.

### 23. `feat(lsp): add hover support`

Scope:

- Add hover for known type aliases, locals, function signatures, and table
  fields.

Acceptance:

- Hover tests cover at least one symbol from each supported category.
- Unknown symbols produce no hover instead of an error.

### 24. `feat(lsp): add basic completions`

Scope:

- Complete known type aliases in annotation positions.
- Complete known table fields after field access when the table shape is known.

Acceptance:

- Completion tests cover alias names and table fields.
- Completion remains quiet for `any` and unknown dynamic values.

## Phase 9: VS Code Extension

### 25. `feat(vscode): add minimal tua extension`

Scope:

- Add `editors/vscode`.
- Register language id `tua`.
- Associate `*.tua`.
- Add basic grammar coverage for Lua plus tua type syntax.

Acceptance:

- Opening `.tua` files uses the tua language mode.
- Syntax highlighting recognizes keywords, comments, strings, type aliases, and
  annotations.

### 26. `feat(vscode): connect extension to tua lsp`

Scope:

- Launch `tua lsp` from the extension.
- Add a setting for an explicit server binary path.
- Surface diagnostics, hover, and completion from the LSP.

Acceptance:

- Manual smoke test in VS Code shows diagnostics for a type mismatch.
- Extension activation fails gracefully when `tua` is not found.

## Phase 10: V1 Polish And Release Readiness

### 27. `docs: add user-facing quickstart`

Scope:

- Add quickstart instructions to `README.md`.
- Show install/build/check/watch usage.
- Include the reference `Player` example from `SPEC.md`.

Acceptance:

- A new user can create a project, run `tua build`, and run the output in
  LÖVE by following the README.

### 28. `test: add v1 acceptance suite`

Scope:

- Add end-to-end tests that map directly to the `SPEC.md` acceptance criteria.
- Include fixtures for primitives, functions, unions, optional fields, table
  shapes, compound assignment, `require`, and emitted Lua output.

Acceptance:

- `cargo test --workspace` covers the full v1 contract.
- Fixture names clearly match spec features.

### 29. `chore: prepare v1.0.0-alpha release`

Scope:

- Audit diagnostics for consistency.
- Audit CLI help text.
- Ensure examples and docs match actual behavior.
- Add a changelog entry.

Acceptance:

- All tests pass.
- `SPEC.md`, `ROADMAP.md`, and `README.md` agree on supported v1 behavior.
- The project can be tagged as `v1.0.0-alpha.1`.

## Deferred After V1

These items are intentionally outside the v1 commit sequence:

- runtime type checks;
- type-only imports and exports;
- generics;
- metatable typing;
- vararg typing;
- package management;
- optimization passes;
- alternate Lua output targets;
- full Luau compatibility.

## Phase 11: LuaHelper And Rust-Analyzer Parity

This phase turns the current LSP into a deeper project-aware editor experience.
The sequence is scoped so each commit can land independently while keeping the
server buildable and tested.

### 30. `docs(lsp): add luahelper parity matrix`

Scope:

- Add a checked-in feature matrix comparing Tua, LuaHelper, and useful
  rust-analyzer patterns.
- Split features into:
  - already supported;
  - protocol surface supported but shallow;
  - missing and planned;
  - intentionally out of scope.
- Use the matrix to drive the remaining commits in this phase.

Acceptance:

- The matrix reflects current Tua behavior, not aspirational behavior.
- Each planned gap points to a roadmap commit number.

### 31. `feat(project): add shared analysis snapshot`

Scope:

- Introduce a project analysis snapshot shared by diagnostics, hover,
  completion, definition, references, rename, signature help, and workspace
  symbols.
- Cache parsed sources, type environments, module exports, require edges, and
  file diagnostics.
- Keep request handlers deterministic by reading from a snapshot instead of
  mutating global state during a request.

Acceptance:

- LSP requests no longer need to independently re-read the same required module
  in the common path.
- Updating one open document invalidates only affected snapshot entries.
- Existing LSP and core tests still pass with the snapshot path enabled.

### 32. `feat(lsp): reindex project on watched file changes`

Scope:

- Wire `workspace/didChangeWatchedFiles` into the shared analysis snapshot.
- Handle created, changed, and deleted `.tua`, `.lua`, and `tua.toml` files.
- Republish diagnostics for open files affected by module or config changes.

Acceptance:

- Creating a required module updates completions, hover, definition, and
  diagnostics without restarting the server.
- Deleting a required module clears stale exports and reports affected missing
  require diagnostics.
- Changing `tua.toml` updates diagnostic settings for files under that root.

### 33. `feat(parser): parse luahelper doc annotations`

Scope:

- Parse leading Lua comments for LuaHelper/LuaLS-style annotations:
  - `---@class`
  - `---@field`
  - `---@alias`
  - `---@type`
  - `---@param`
  - `---@return`
  - `---@generic`
  - `---@version`
- Preserve spans and documentation text.
- Keep annotation parsing independent from ordinary Lua parsing failures where
  possible.

Acceptance:

- Annotation fixtures parse into a typed annotation model.
- Malformed annotations produce diagnostics only when annotation diagnostics are
  enabled.
- Existing `.tua` type syntax continues to parse unchanged.

### 34. `feat(types): lower luahelper annotations into type environment`

Scope:

- Convert parsed LuaHelper annotations into Tua `TypeEnvironment` entries.
- Support annotated Lua files as providers of types, functions, globals, and
  module exports.
- Merge comment docs with symbols so hover, completion resolve, and signature
  help can reuse them.

Acceptance:

- A `.lua` file with `---@param` and `---@return` provides a typed function to
  a `.tua` caller.
- A returned module table with annotated function fields exposes typed exports.
- Type conflicts between `.tua` syntax and comment annotations produce stable,
  deterministic diagnostics.

### 35. `feat(lua): ingest standard-library annotation stubs`

Scope:

- Add a built-in Lua standard-library stub source based on LuaHelper/LuaLS-style
  annotations.
- Cover global functions, `_G`, `_ENV`, `math`, `string`, `table`, `io`, `os`,
  `coroutine`, `debug`, `package`, and Lua 5.1/LuaJIT compatibility variants.
- Replace ad hoc builtin signature strings where the typed stub can be the
  source of truth.

Acceptance:

- Hover, completion, and signature help for Lua builtins are generated from the
  stub model.
- Version-gated symbols such as `setfenv`, `getfenv`, `unpack`, and `warn`
  respect the configured target.
- Builtin docs appear in hover and completion resolve.

### 36. `feat(lint): add undefined and unused symbol diagnostics`

Scope:

- Add configurable diagnostics for:
  - undefined variables;
  - unused locals;
  - unused parameters;
  - use before local definition where Tua can prove it.
- Make diagnostics aware of globals from Lua, LÖVE, configured stubs, and
  required modules.

Acceptance:

- Diagnostics can be enabled independently in `tua.toml`.
- Shadowing and nested scopes have regression tests.
- Dynamic code through `any`, `_G`, or unknown module exports remains
  permissive where static proof is not possible.

### 37. `feat(lint): add luahelper-style correctness checks`

Scope:

- Add configurable lints for:
  - duplicate function parameters;
  - wrong function argument count;
  - bad require files;
  - impossible boolean expressions;
  - duplicate `if` conditions;
  - duplicate binary operands;
  - float equality;
  - const assignment;
  - function parameter type mismatch;
  - function return type mismatch.
- Reuse the existing type checker where possible instead of duplicating type
  logic in lint visitors.

Acceptance:

- Each lint has focused fixtures and a config toggle.
- Diagnostics use stable codes and messages.
- Lints do not produce duplicates for errors already reported by the checker.

### 38. `feat(completion): add annotation and doc-snippet completions`

Scope:

- Complete annotation tags after `---@`.
- Complete type names inside annotation type positions.
- Generate function documentation snippets from function signatures.
- Support LuaHelper-like trigger characters where useful without making normal
  identifier completion noisy.

Acceptance:

- `---@` offers annotation tags.
- `---` above a function can generate a snippet with params and returns.
- Annotation completions work in `.lua` and `.tua` files.

### 39. `feat(completion): make resolve truly lazy`

Scope:

- Stop embedding full documentation payloads in initial completion items.
- Store enough item data to resolve details and documentation from the analysis
  snapshot.
- Include module path, symbol kind, type label, and docs in resolved items.

Acceptance:

- Initial completion payloads stay small for large projects.
- `completionItem/resolve` can enrich items after edits without stale cache
  crashes.
- Builtins, module exports, locals, functions, aliases, and annotation snippets
  all resolve consistently.

### 40. `feat(hover): attach docs and declaration context`

Scope:

- Include doc comments and annotation documentation in hover output.
- Show declaration kind, type, module path, and source location when useful.
- Keep compact code-fence output for symbols without docs.

Acceptance:

- Hovering an annotated function shows signature plus documentation.
- Hovering a module export shows the exported symbol docs, not just the
  receiving field name.
- Hover remains empty for unknown symbols.

### 41. `feat(signature): add parameter documentation`

Scope:

- Carry parameter docs from annotations and comments into signature help.
- Keep active parameter selection working through nested calls, comments, and
  strings.
- Support required-module functions, builtin stubs, and local functions through
  the same formatting path.

Acceptance:

- Signature help includes per-parameter documentation when present.
- Active parameter tests cover nested calls and trailing commas.
- Lua builtin and user-defined signatures use the same LSP shape.

### 42. `feat(references): add project-wide symbol references`

Scope:

- Use the shared analysis snapshot to find references across workspace files.
- Include exported module symbols, required-module fields, aliases, locals
  where their scope crosses files through exports, and globals.
- Respect LSP `includeDeclaration`.

Acceptance:

- References from a module export include callers in other files.
- References update after watched file changes.
- Large workspaces are bounded by indexed files and do not scan arbitrary
  directories on every request.

### 43. `feat(rename): add safe cross-file rename`

Scope:

- Extend rename from single-file edits to workspace edits when the target has a
  project-wide identity.
- Support module export renames and type alias renames across files.
- Reject ambiguous globals, dynamic fields, and unknown `any` paths.

Acceptance:

- Renaming an exported function updates its local declaration, returned module
  field, and known callers.
- Rename returns `null` for unsafe dynamic cases.
- Workspace edits are deterministic and sorted by path and range.

### 44. `feat(code-action): add quick fixes`

Scope:

- Advertise `codeActionProvider`.
- Add quick fixes for diagnostics where the edit is mechanical:
  - create missing require target;
  - add missing local annotation from inferred type;
  - remove unused local;
  - rename duplicate table key;
  - replace self-assignment with no-op removal.
- Keep code actions optional and conservative.

Acceptance:

- Each quick fix has an LSP test asserting the exact workspace edit.
- Code actions are absent when the edit would be unsafe or ambiguous.

### 45. `feat(imports): add auto-import completion`

Scope:

- Index exported module symbols by name.
- Offer completion items for unresolved identifiers that can be imported from a
  known module.
- Resolve the item into a text edit that inserts or updates a `local X =
require("module")` binding.

Acceptance:

- Unresolved exported functions appear in completion with module detail.
- Accepting the item inserts a require binding once.
- Existing require aliases are reused instead of duplicated.

### 46. `feat(config): handle workspace configuration changes`

Scope:

- Handle `workspace/didChangeConfiguration`.
- Accept editor-provided diagnostic toggles, file associations, ignored paths,
  require path separator, and preview settings.
- Reconcile editor settings with `tua.toml` using documented precedence.

Acceptance:

- Changing a diagnostic toggle in the client republishes diagnostics.
- File associations allow configured non-`.lua` files to be analyzed as Lua
  interop sources.
- Ignored paths are excluded from project indexing and diagnostics.

### 47. `feat(semantic): add richer token classes`

Scope:

- Expand semantic tokens for globals, module names, methods, fields, aliases,
  annotation tags, readonly values, deprecated symbols, and unresolved
  references.
- Keep the current compact token legend stable where possible.

Acceptance:

- Semantic token tests cover annotations, module exports, globals, parameters,
  locals, and unresolved names.
- Tokens remain sorted and delta-encodable.

### 48. `feat(lsp): add call hierarchy`

Scope:

- Advertise and implement call hierarchy for known functions.
- Use the project snapshot to find callers and callees.
- Support local functions, module-exported functions, and methods where the
  receiver type is known.

Acceptance:

- Preparing call hierarchy on a function returns one item.
- Incoming calls include same-file and cross-file callers.
- Dynamic calls through `any` are ignored rather than guessed.

### 48.5 `feat(love): add built-in love api catalog`

Scope:

- Generate and check in a LOVE 11.x API catalog from
  <https://github.com/love2d-community/love-api>.
- Use the generated catalog by default for LOVE completions, hover, and
  signature help.
- Keep the existing curated Lua builtin catalog unchanged.
- Preserve the older curated LOVE table only as fallback data for entries that
  are not covered by the generated catalog.

Acceptance:

- `love.graphics.pr` completes generated LOVE API fields such as `present`,
  `print`, and `printf`.
- APIs outside the old curated subset, such as `love.physics.newWorld`, are
  available through completion.
- LOVE hover and signature help show generated argument and return labels.
- No project configuration is required to enable LOVE autocomplete.

### 49. `test(lsp): add luahelper parity fixtures`

Scope:

- Add a fixture suite derived from representative LuaHelper behaviors:
  - annotated Lua modules;
  - builtin stubs;
  - project-wide references;
  - watched-file reindexing;
  - rich completion resolve;
  - signature docs;
  - lint toggles.
- Keep fixtures small and readable.

Acceptance:

- The parity suite runs under `cargo test --workspace`.
- Each fixture states the LuaHelper behavior it is matching.
- The suite can be expanded without changing helper APIs.

### 50. `docs: update roadmap after parity milestone`

Scope:

- Mark completed parity commits.
- Move any intentionally skipped LuaHelper behavior to a documented non-goal
  list.
- Update `README.md` and `SPEC.md` if the supported language surface changed.

Acceptance:

- Docs match implementation.
- The parity matrix has no stale "planned" entries for landed work.

## Phase 12: Rust-Analyzer Architecture Parity

This phase uses rust-analyzer's crate architecture as a reference point for
making Tua faster, more maintainable, and more project-aware. It is not a goal
to copy rust-analyzer directly; the goal is to introduce the same kinds of
boundaries in a Lua/LÖVE-sized form.

### 51. `docs(architecture): add rust-analyzer gap matrix`

Scope:

- Add an architecture matrix that maps rust-analyzer crates to Tua equivalents:
  - `base-db`, `ide-db`, `vfs`, `vfs-notify`;
  - `project-model`;
  - `parser` and `syntax`;
  - `hir`, `hir-def`, `hir-expand`, `hir-ty`;
  - `ide`, `ide-assists`, `ide-completion`, `ide-diagnostics`;
  - `test-fixture`, `test-utils`, `profile`.
- Mark each row as existing, partial, missing, or intentionally out of scope.
- Link each planned missing piece to this phase's commit numbers.

Acceptance:

- The matrix describes current implementation accurately.
- Every planned architecture gap maps to a committable roadmap entry.
- Non-goals are explicit so the project does not drift into a Rust clone.

### 52. `refactor(core): introduce analysis database module`

Scope:

- Add an internal `analysis` domain in `tua-core`.
- Define stable IDs for files, modules, symbols, and source roots.
- Move shared request inputs into an `AnalysisHost` and immutable
  `AnalysisSnapshot` API.
- Keep the existing public query functions working as wrappers.

Acceptance:

- Existing core and LSP tests pass through the compatibility wrappers.
- No LSP handler directly owns project-wide semantic state.
- The snapshot API can answer parse, diagnostics, hover, completion, and
  definition queries for one file.

### 53. `feat(vfs): add virtual file system and open-file overlays`

Scope:

- Track workspace files in a small virtual file system.
- Store open editor documents as overlays over disk contents.
- Normalize paths, URIs, source roots, and module-relative paths in one place.
- Exclude build output and ignored paths.

Acceptance:

- Open unsaved documents are used by diagnostics and hover.
- Closed documents fall back to disk content.
- Path and URI conversion has focused tests for spaces, nested paths, and
  cross-platform separators.

### 54. `feat(project-model): model source roots and require paths`

Scope:

- Expand project modeling beyond current config loading.
- Represent configured source roots, output roots, include/exclude globs,
  require roots, target Lua version, and LÖVE version.
- Prepare for multiple workspace folders without merging their configs
  accidentally.

Acceptance:

- `tua.toml` loads into a project model used by CLI and LSP.
- Multiple source roots are testable even if only one is emitted by default.
- Require resolution uses project roots instead of ad hoc parent-directory
  probing.

### 55. `feat(db): add dependency-tracked query cache`

Scope:

- Cache expensive queries:
  - parsed files;
  - type environments;
  - diagnostics;
  - module exports;
  - require edges;
  - symbol indexes.
- Track dependencies between queries at file granularity first.
- Invalidate affected results on document, file, or config changes.

Acceptance:

- Editing one file does not reparse unrelated files in common LSP requests.
- Changing an exported module invalidates known importers.
- Tests assert cache reuse and invalidation behavior without relying on timing.

### 56. `refactor(parser): separate parse tree from semantic source model`

Scope:

- Split raw parser output from Tua's typed source model.
- Keep full source spans and token spans for IDE features.
- Preserve lossy/recovery parse paths used while typing.
- Make type-only syntax and Lua-compatible syntax explicit in the model.

Acceptance:

- Parser tests distinguish raw parse, normalized source, and typed source
  extraction.
- Existing emit/type/check behavior is unchanged.
- Incomplete source can still provide best-effort completions and semantic
  tokens.

### 57. `feat(hir): lower files into a semantic module model`

Scope:

- Add a small HIR-like model for Tua modules:
  - declarations;
  - locals and parameters;
  - functions;
  - type aliases;
  - table-shape fields;
  - returned module exports;
  - require bindings.
- Use stable symbol IDs instead of repeatedly searching syntax nodes.

Acceptance:

- Definition, references, rename, document symbols, and semantic tokens can use
  the lowered module model for same-file symbols.
- The model has fixture tests independent of LSP JSON.
- Unknown dynamic expressions remain represented as dynamic/unknown instead of
  guessed.

### 58. `feat(module): build require graph and export index`

Scope:

- Build a workspace require graph from the project model and HIR modules.
- Index module exports by symbol ID, module path, and public name.
- Detect missing modules, duplicate candidate modules, and require cycles.

Acceptance:

- Cross-file definition and hover use the index instead of reading files during
  the request.
- Missing require diagnostics include the attempted module name and search
  roots.
- Require cycles do not panic and produce deterministic partial results.

### 59. `feat(scope): add lexical scope maps`

Scope:

- Build scope maps for locals, parameters, globals, module aliases, and table
  fields.
- Track shadowing and declaration/use ranges.
- Make scope lookup reusable by diagnostics, completion, rename, references,
  document highlight, and semantic tokens.

Acceptance:

- Same-name locals in nested scopes resolve independently.
- Rename and references use the same scope lookup.
- Undefined/unused diagnostics can be implemented without custom tree walks.

### 60. `feat(types): add project-aware type database`

Scope:

- Move type environments into the analysis database.
- Resolve aliases, table shapes, function signatures, module exports, and
  builtin stubs across files.
- Keep gradual typing semantics: `any` and unknown dynamic values stay
  permissive.

Acceptance:

- A `.tua` file can use types exported by another indexed module.
- Required `.lua` files with annotations contribute types when available.
- Type queries are cached and invalidated through the database.

### 61. `feat(types): add flow-sensitive narrowing`

Scope:

- Narrow union and optional types inside simple guards:
  - `if value ~= nil then`;
  - `if value == nil then`;
  - `if type(value) == "string" then`;
  - equality checks against primitive literal types where supported.
- Keep narrowing local to a block and conservative around mutation.

Acceptance:

- `number | nil` narrows to `number` after a non-nil guard.
- Return checking and hover use narrowed types inside guarded blocks.
- Mutating the narrowed variable clears the narrowed fact.

### 62. `feat(diagnostics): move diagnostics to incremental pipeline`

Scope:

- Run parser, type, lint, project, and emission diagnostics through the analysis
  database.
- Deduplicate diagnostics produced by overlapping checkers.
- Keep diagnostic config toggles in the project model.

Acceptance:

- Diagnostics for an open file update after direct edits and after dependency
  edits.
- Error, warning, info, and hint severities round-trip through LSP.
- CLI and LSP use the same diagnostic pipeline.

### 63. `feat(ide-db): add shared symbol index`

Scope:

- Build a symbol index for aliases, functions, locals, parameters, fields,
  modules, and exports.
- Support fuzzy matching for workspace symbols and auto-import candidates.
- Store container names and module paths for display.

Acceptance:

- Workspace symbols use the index instead of scanning files per request.
- Completion can ask the index for importable symbols.
- Index updates after watched file changes.

### 64. `feat(ide): route all LSP features through analysis snapshot`

Scope:

- Update hover, completion, completion resolve, signature help, definition,
  document links, document highlight, document symbols, references, rename,
  inlay hints, semantic tokens, and workspace symbols to read from
  `AnalysisSnapshot`.
- Remove request-time disk reads from feature implementations.
- Keep simple public `*_tua_source` helpers for unit tests and CLI-like use.

Acceptance:

- Feature tests still pass.
- An LSP smoke test confirms all supported requests work after an edit and a
  watched-file change.
- Request handlers become mostly protocol conversion plus snapshot access.

### 65. `feat(assists): add code action framework`

Scope:

- Add an `assists` or `code_action` domain modeled after rust-analyzer's
  assist separation.
- Represent assists as pure functions over a snapshot and source range.
- Start with no-op plumbing plus one safe quick fix.

Acceptance:

- LSP advertises `codeActionProvider`.
- A diagnostic can produce a workspace edit through the framework.
- Assist tests use fixture syntax instead of hand-written JSON when possible.

### 66. `feat(ide): add selection and folding ranges`

Scope:

- Implement `textDocument/selectionRange` from syntax/HIR spans.
- Implement `textDocument/foldingRange` for functions, tables, comments, and
  multi-line type aliases.

Acceptance:

- Selection range grows from identifier to field chain to statement to block.
- Folding ranges are stable while typing incomplete code.
- LSP tests cover nested functions and table constructors.

### 67. `feat(ide): add call hierarchy`

Scope:

- Implement call hierarchy on known local and exported functions.
- Use the require graph and symbol index for cross-file callers.
- Ignore dynamic calls through `any`.

Acceptance:

- Incoming calls include same-file and cross-file callers.
- Outgoing calls include known callees from function bodies.
- Unknown/dynamic call targets are omitted without diagnostics.

### 68. `test: add fixture DSL for IDE tests`

Scope:

- Add a compact fixture format for source text, cursor positions, expected
  ranges, expected diagnostics, and expected edits.
- Replace repetitive test helpers where it improves readability.
- Support multi-file fixtures for project-aware features.

Acceptance:

- New tests can define files and cursor markers inline.
- The DSL supports both core query tests and LSP protocol tests.
- Existing tests can migrate gradually.

### 69. `chore(profile): add analysis tracing and benchmarks`

Scope:

- Add lightweight timing/tracing hooks around parsing, lowering, type checking,
  indexing, and LSP requests.
- Add benchmark fixtures for small, medium, and large projects.
- Keep profiling disabled by default.

Acceptance:

- Developers can run a documented command to profile LSP request latency.
- Benchmarks report cache hit/miss behavior.
- Profiling output does not pollute normal CLI or LSP output.

### 70. `docs: publish architecture guide`

Scope:

- Document the final architecture:
  - CLI;
  - LSP;
  - analysis host/snapshot;
  - VFS;
  - project model;
  - HIR-like module model;
  - type database;
  - IDE feature layers.
- Explain how to add a new query or LSP feature.

Acceptance:

- New contributors can identify where parser, type, diagnostics, and IDE code
  belongs.
- The guide links back to `SPEC.md` and the rust-analyzer gap matrix.
- Any outdated roadmap entries from this phase are marked complete or deferred.
