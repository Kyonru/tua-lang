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

Status: done.

Scope:

- Add this roadmap.
- Organize future implementation into independently reviewable commits.

Acceptance:

- The roadmap can be followed without making major implementation decisions.

## Phase 1: Rust Workspace And CLI Shell

### 3. `chore: scaffold rust workspace`

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

Scope:

- Integrate a Lua parser foundation.
- Parse ordinary Lua-compatible `.tua` files.
- Preserve source spans for diagnostics and later emission.

Acceptance:

- Parse tests cover locals, functions, tables, returns, and `require`.
- Syntax errors produce `TL000x` diagnostics with source locations.

### 8. `feat(parser): parse erased type syntax`

Status: done.

Scope:

- Parse v1 type aliases.
- Parse local annotations.
- Parse function parameter and return annotations.
- Parse table shape, optional field, and union type syntax.

Acceptance:

- Parser tests cover every type syntax form in `SPEC.md`.
- Type-only syntax is represented in the core AST or typed source model.

### 9. `feat(parser): parse compound assignment syntax`

Status: done.

Scope:

- Parse `+=`, `-=`, `*=`, `/=`, and `%=`.
- Restrict the accepted left-hand side shape to names and plain field chains.

Acceptance:

- Valid compound assignments parse.
- Calls, computed indexes, and other complex left-hand sides produce diagnostics.

## Phase 4: Erasure And Emission

### 10. `feat(emit): erase type-only syntax`

Status: done.

Scope:

- Emit Lua with type aliases removed.
- Emit locals and functions without type annotations.
- Preserve ordinary Lua statements and expressions.

Acceptance:

- Snapshot tests compare `.tua` input to emitted `.lua`.
- Type aliases emit no runtime code.

### 11. `feat(emit): lower compound assignment`

Status: done.

Scope:

- Lower supported compound assignments to Lua 5.1-compatible assignments.
- Keep field-chain lowering readable:
  `player.hp -= amount` becomes `player.hp = player.hp - amount`.

Acceptance:

- Snapshot tests cover all supported compound operators.
- Unsupported left-hand sides produce `TL200x` diagnostics.

### 12. `feat(build): mirror source tree to output tree`

Status: done.

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

Status: done.

Scope:

- Represent primitive types, `any`, aliases, unions, function types, table
  shapes, and optional fields.
- Add assignability rules for v1.

Acceptance:

- Unit tests cover primitive assignability, `any`, unions, optional fields, and
  table shape compatibility.

### 14. `feat(types): collect aliases and annotations`

Status: done.

Scope:

- Build per-file type environments.
- Resolve aliases within a file.
- Collect local and function annotations.

Acceptance:

- Unknown type names produce `TL100x` diagnostics.
- Duplicate type aliases produce a deterministic diagnostic.

### 15. `feat(check): check annotated locals and assignments`

Status: done.

Scope:

- Infer primitive literal types.
- Check annotated local initializers.
- Check assignments to known typed locals and known table fields.

Acceptance:

- Tests cover successful assignments and primitive/table-field mismatches.
- Unannotated dynamic code remains permissive.

### 16. `feat(check): check function boundaries`

Status: done.

Scope:

- Check annotated function arguments at call sites when the callee type is known.
- Check return statements against annotated return types.

Acceptance:

- Tests cover argument count, argument type, and return type diagnostics.
- Calls through `any` remain permissive.

## Phase 6: CLI Behavior

### 17. `feat(cli): implement check command`

Status: done.

Scope:

- Wire config loading, source discovery, parsing, and type checking into
  `tua check`.

Acceptance:

- `tua check` returns `0` for clean projects.
- `tua check` returns non-zero for parse or type errors.
- Diagnostics include file, line, column, severity, and message.

### 18. `feat(cli): implement build command`

Status: done.

Scope:

- Wire checking, lowering, erasure, and output writing into `tua build`.
- Block emission on errors by default.
- Implement `--emit-on-error` for parseable/lowerable files.

Acceptance:

- `tua build` writes mirrored Lua output for valid projects.
- Type errors block normal builds.
- `--emit-on-error` emits possible files and still exits non-zero.

### 19. `feat(cli): implement watch command`

Status: done.

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

Status: done.

Scope:

- Add `examples/love-minimal`.
- Include a tiny `.tua` module and `main.tua`.
- Build to Lua and run with LÖVE where available.

Acceptance:

- The emitted example runs under LÖVE 11.x.
- CI or local tests skip gracefully when LÖVE is unavailable.

### 21. `test(lua): validate emitted lua syntax`

Status: done.

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

Status: done.

Scope:

- Implement `tua lsp` over stdio.
- Publish diagnostics using the shared parser/checker.

Acceptance:

- LSP initialization succeeds.
- Opening or changing a `.tua` document returns matching CLI diagnostics.

### 23. `feat(lsp): add hover support`

Status: done.

Scope:

- Add hover for known type aliases, locals, function signatures, and table
  fields.

Acceptance:

- Hover tests cover at least one symbol from each supported category.
- Unknown symbols produce no hover instead of an error.

### 24. `feat(lsp): add basic completions`

Status: done.

Scope:

- Complete known type aliases in annotation positions.
- Complete known table fields after field access when the table shape is known.

Acceptance:

- Completion tests cover alias names and table fields.
- Completion remains quiet for `any` and unknown dynamic values.

## Phase 9: VS Code Extension

### 25. `feat(vscode): add minimal tua extension`

Status: done.

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

Status: done.

Scope:

- Launch `tua lsp` from the extension.
- Add a setting for an explicit server binary path.
- Surface diagnostics, hover, and completion from the LSP.

Acceptance:

- Manual smoke test in VS Code shows diagnostics for a type mismatch.
- Extension activation fails gracefully when `tua` is not found.

## Phase 10: V1 Polish And Release Readiness

### 27. `docs: add user-facing quickstart`

Status: done.

Scope:

- Add quickstart instructions to `README.md`.
- Show install/build/check/watch usage.
- Include the reference `Player` example from `SPEC.md`.

Acceptance:

- A new user can create a project, run `tua build`, and run the output in
  LÖVE by following the README.

### 28. `test: add v1 acceptance suite`

Status: done.

Scope:

- Add end-to-end tests that map directly to the `SPEC.md` acceptance criteria.
- Include fixtures for primitives, functions, unions, optional fields, table
  shapes, compound assignment, `require`, and emitted Lua output.

Acceptance:

- `cargo test --workspace` covers the full v1 contract.
- Fixture names clearly match spec features.

### 29. `chore: prepare v1.0.0-alpha release`

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

Status: done.

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

### 48.6 `feat(assets): add typed love asset loaders`

Status: done.

Scope:

- Add typed LOVE-focused asset loader helpers:
  - `image "assets/player.png"` returns `Image`.
  - `sound "assets/jump.wav"` returns `Source`.
- Lower helpers to LuaJIT/LÖVE-compatible calls during emission:
  - `image "path"` emits `love.graphics.newImage("path")`.
  - `sound "path"` emits `love.audio.newSource("path", "static")`.
- Include loader completions, hover, and signature help in the builtin catalog.
- Treat `Image` and `Source` as built-in nominal LOVE type names.

Acceptance:

- Annotated locals can use `Image` and `Source` without project-local aliases.
- Asset loader calls infer their returned LOVE resource type.
- Emitted Lua contains ordinary LOVE API calls and no Tua-only asset syntax.

### 49. `test(lsp): add luahelper parity fixtures`

Status: done.

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

Status: done.

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

The feature surface is already broad: diagnostics, hover, completion,
definition, references, rename, workspace symbols, code actions, call hierarchy,
LOVE helpers, LuaDoc parsing, and watched-file updates exist. Phase 12 should
therefore prioritize architecture, consistency, and performance over adding
another parallel feature path.

Status: done.

### 51. `docs(architecture): add rust-analyzer gap matrix`

Status: done.

Scope:

- Add `docs/architecture.md` with an architecture matrix that maps
  rust-analyzer crates to Tua equivalents:
  - `base-db`, `ide-db`, `vfs`, `vfs-notify`;
  - `project-model`;
  - `parser` and `syntax`;
  - `hir`, `hir-def`, `hir-expand`, `hir-ty`;
  - `ide`, `ide-assists`, `ide-completion`, `ide-diagnostics`;
  - `test-fixture`, `test-utils`, `profile`.
- Mark each row as existing, partial, missing, or intentionally out of scope.
- Link each planned missing piece to this phase's commit numbers.
- Include a short "current architecture" diagram for the existing
  `tua-core`, `tua-cli`, and `tua-lsp` crates.

Acceptance:

- The matrix describes current implementation accurately.
- Every planned architecture gap maps to a committable roadmap entry.
- Non-goals are explicit so the project does not drift into a Rust clone.

### 52. `test: add fixture DSL for IDE and project tests`

Status: done.

Scope:

- Add a compact fixture format for:
  - multiple files;
  - cursor/range markers;
  - expected diagnostics;
  - expected completions, hovers, locations, edits, and symbols.
- Support both core query tests and LSP protocol tests.
- Start by migrating a small representative set of repetitive tests.

Acceptance:

- New tests can define files and cursor markers inline.
- Multi-file fixtures can model `require` relationships without hand-written
  temp-project boilerplate.
- Existing tests can migrate gradually; no large mechanical rewrite is required.

### 53. `refactor(core): introduce analysis host and snapshot`

Status: done.

Scope:

- Add an internal `analysis` domain in `tua-core`.
- Define the public shape of `AnalysisHost` and immutable `AnalysisSnapshot`.
- Move shared request inputs into the host/snapshot boundary:
  - project config;
  - source roots;
  - file contents;
  - open document overlays;
  - builtin catalogs.
- Keep the existing public query functions working as wrappers.
- Avoid changing query behavior in this commit.

Acceptance:

- Existing core and LSP tests pass through the compatibility wrappers.
- No LSP handler directly owns project-wide semantic state.
- The snapshot API can answer at least parse, diagnostics, hover, completion,
  and definition queries for one file.

### 54. `feat(vfs): add virtual file system and open-file overlays`

Status: done.

Scope:

- Track workspace files in a small virtual file system.
- Store open editor documents as overlays over disk contents.
- Normalize paths, URIs, source roots, and module-relative paths in one place.
- Exclude build output and ignored paths.
- Keep file identity stable across edits and path normalization.

Acceptance:

- Open unsaved documents are used by diagnostics and hover.
- Closed documents fall back to disk content.
- Path and URI conversion has focused tests for spaces, nested paths, and
  cross-platform separators.

### 55. `feat(project-model): model roots, targets, and require paths`

Status: done.

Scope:

- Expand project modeling beyond config parsing.
- Represent:
  - workspace folders;
  - configured source roots;
  - output roots;
  - ignored paths;
  - file associations;
  - require roots and path separator;
  - target Lua version;
  - LÖVE version.
- Prepare for multiple workspace folders without merging their configs
  accidentally.

Acceptance:

- `tua.toml` loads into a project model used by CLI and LSP.
- Multiple source roots are testable even if only one is emitted by default.
- Require resolution uses project roots instead of ad hoc parent-directory
  probing.

### 56. `feat(db): add dependency-tracked query cache`

Status: done.

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
- Keep the cache deterministic and testable; avoid timing-based assertions.

Acceptance:

- Editing one file does not reparse unrelated files in common LSP requests.
- Changing an exported module invalidates known importers.
- Tests assert cache reuse and invalidation behavior without relying on timing.

### 57. `refactor(parser): separate syntax tree from semantic source model`

Status: done.

Scope:

- Split raw parser output from Tua's typed source model.
- Keep full source spans and token spans for IDE features.
- Preserve lossy/recovery parse paths used while typing.
- Make type-only syntax and Lua-compatible syntax explicit in the model.
- Keep LuaDoc parsing independent enough to work when Lua syntax is incomplete.

Acceptance:

- Parser tests distinguish raw parse, normalized source, and typed source
  extraction.
- Existing emit/type/check behavior is unchanged.
- Incomplete source can still provide best-effort completions and semantic
  tokens.

### 58. `feat(hir): lower files into semantic modules`

Status: done.

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
- Include origin spans for diagnostics, hover, navigation, and edits.

Acceptance:

- Definition, references, rename, document symbols, and semantic tokens can use
  the lowered module model for same-file symbols.
- The model has fixture tests independent of LSP JSON.
- Unknown dynamic expressions remain represented as dynamic/unknown instead of
  guessed.

### 59. `feat(module): build require graph and export index`

Status: done.

Scope:

- Build a workspace require graph from the project model and HIR modules.
- Index module exports by symbol ID, module path, and public name.
- Detect missing modules, duplicate candidate modules, and require cycles.
- Track whether exports came from `.tua`, `.lua`, LuaDoc annotations, or
  generated LOVE/builtin catalogs.

Acceptance:

- Cross-file definition and hover use the index instead of reading files during
  the request.
- Missing require diagnostics include the attempted module name and search
  roots.
- Require cycles do not panic and produce deterministic partial results.

### 60. `feat(scope): add lexical scope maps`

Status: done.

Scope:

- Build scope maps for locals, parameters, globals, module aliases, and table
  fields.
- Track shadowing and declaration/use ranges.
- Make scope lookup reusable by diagnostics, completion, rename, references,
  document highlight, and semantic tokens.
- Represent global allow-lists, Lua builtins, LOVE globals, and configured
  globals through the same lookup path.

Acceptance:

- Same-name locals in nested scopes resolve independently. Maybe warns about it (configurable).
- Rename and references use the same scope lookup.
- Undefined/unused diagnostics can be implemented without custom tree walks.

### 61. `feat(types): add project-aware type database`

Status: done.

Scope:

- Move type environments into the analysis database.
- Resolve aliases, table shapes, function signatures, module exports, and
  builtin stubs across files.
- Keep gradual typing semantics: `any` and unknown dynamic values stay
  permissive.
- Preserve existing multi-return behavior for Lua/LOVE calls.

Acceptance:

- A `.tua` file can use types exported by another indexed module.
- Required `.lua` files with annotations contribute types when available.
- Type queries are cached and invalidated through the database.

### 62. `feat(types): add flow-sensitive narrowing`

Status: done.

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

### 63. `feat(diagnostics): move diagnostics to incremental pipeline`

Status: done.

Scope:

- Run parser, type, lint, project, and emission diagnostics through the analysis
  database.
- Deduplicate diagnostics produced by overlapping checkers.
- Keep diagnostic config toggles in the project model.
- Ensure asset validation uses the project model and VFS consistently.

Acceptance:

- Diagnostics for an open file update after direct edits and after dependency
  edits.
- Error, warning, info, and hint severities round-trip through LSP.
- CLI and LSP use the same diagnostic pipeline.

### 64. `feat(ide-db): add shared symbol index`

Status: done.

Scope:

- Build a symbol index for aliases, functions, locals, parameters, fields,
  modules, and exports.
- Support fuzzy matching for workspace symbols and auto-import candidates.
- Store container names and module paths for display.
- Keep index entries tied to stable symbol IDs and source ranges.

Acceptance:

- Workspace symbols use the index instead of scanning files per request.
- Completion can ask the index for importable symbols.
- Index updates after watched file changes.

### 65. `feat(ide): route LSP features through analysis snapshot`

Status: done.

Scope:

- Update hover, completion, completion resolve, signature help, definition,
  document links, document highlight, document symbols, references, rename,
  inlay hints, semantic tokens, and workspace symbols to read from
  `AnalysisSnapshot`.
- Remove request-time disk reads from feature implementations.
- Keep simple public `*_tua_source` helpers for unit tests and CLI-like use.
- Treat unknown/dynamic results consistently across every feature surface.

Acceptance:

- Feature tests still pass.
- An LSP smoke test confirms all supported requests work after an edit and a
  watched-file change.
- Request handlers become mostly protocol conversion plus snapshot access.

### 66. `refactor(assists): move code actions onto analysis snapshot`

Status: done.

Scope:

- Separate code-action discovery from LSP protocol conversion.
- Represent assists as pure functions over a snapshot, source range, and
  diagnostics.
- Keep existing quick fixes:
  - create missing require target;
  - add missing local annotation;
  - remove unused local;
  - rename duplicate table key;
  - remove self-assignment.

Acceptance:

- Existing code action tests still pass.
- A diagnostic can produce the same workspace edit through the new framework.
- Assist tests use fixture syntax instead of hand-written JSON when possible.

### 67. `feat(ide): add selection and folding ranges`

Status: done.

Scope:

- Implement `textDocument/selectionRange` from syntax/HIR spans.
- Implement `textDocument/foldingRange` for functions, tables, comments, and
  multi-line type aliases.

Acceptance:

- Selection range grows from identifier to field chain to statement to block.
- Folding ranges are stable while typing incomplete code.
- LSP tests cover nested functions and table constructors.

### 68. `refactor(ide): route call hierarchy through require graph`

Status: done.

Scope:

- Move existing call hierarchy logic onto HIR, scope maps, and the require
  graph.
- Preserve known local, module-exported, and typed receiver method behavior.
- Ignore dynamic calls through `any`.

Acceptance:

- Existing call hierarchy tests still pass.
- Incoming and outgoing calls no longer perform request-time project scans.
- Unknown/dynamic call targets are omitted without diagnostics.

### 69. `chore(profile): add analysis tracing and benchmarks`

Status: done.

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

Status: done.

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

### 70.9 `lint: default linting based on lua so code can be formatted consistently`

Status: done.

Scope:

- Add a Lua-style formatting lint pass in `tua-core` that operates on parser
  tokens and token spans (not full code rewriting).
- Define a default baseline profile for `.tua` and `.lua` inputs, with conservative
  rules for:
  - indentation and spacing,
  - trailing whitespace,
  - statement separation around `then`/`do`/`end` blocks,
  - and simple bracket/comma/token spacing consistency.
- Add new formatter-related diagnostic codes and make them configurable under
  `[diagnostics]` with a shared opt-in toggle (for example
  `formatting_lints`).
- Keep diagnostics stable:
  - one message per rule per span,
  - no speculative suggestions for ambiguous syntax,
  - only whitespace-safe auto-fixes where possible.
- Add editor-facing messaging that explains how these lints help make mixed Lua/Tua
  code style consistent across contributors.

Acceptance:

- A focused fixture suite covers mixed indentation, whitespace, and token-spacing
  cases for supported Lua/Tua syntax.
- Project `tua.toml` defaults remain unchanged until users opt in to the formatting
  preset.
- CI and LSP smoke checks can run with the profile enabled without adding false
  positives in existing fixture projects.
- Diagnostics for formatting lints include ranges that can be pinned in editor
  UI and resolved through the current diagnostics path.

### 70.10 `feat(luadoc): expand LuaLS/LuaCATS annotation compatibility`

Status: done.

Scope:

- Expand beyond the initial LuaHelper/LuaLS annotation subset from commits 33
  and 34 using <https://luals.github.io/wiki/annotations/> as the primary
  compatibility reference.
- Preserve string literal types and literal unions instead of lowering them to
  plain `string`, so annotations like `"static" | "stream"` and
  `'friendly' | 'mad'` can power precise diagnostics and completions.
- Support optional shorthand such as `number?` as `number | nil`.
- Parse and lower additional LuaCATS type forms where they help common Lua and
  LOVE libraries:
  - arrays such as `Image[]`;
  - tuples such as `[number, number]`;
  - dictionaries such as `{ [string]: Source }`;
  - key-value tables such as `table<string, Image>`;
  - function types such as `fun(path: string): Image`.
- Parse useful LuaLS annotation tags not covered by the first subset:
  - `---@enum`
  - `---@meta`
  - `---@module`
  - `---@diagnostic`
  - `---@cast`
  - `---@as`
  - `---@deprecated`
  - `---@nodiscard`
- Keep unsupported or partially supported annotation forms non-fatal. They may
  preserve docs and spans without affecting types until a later commit.
- Add examples under `examples/features` with comments showing the supported
  LuaLS/LuaCATS forms.

Acceptance:

- LuaLS-style annotations from representative Lua and LOVE libraries produce
  useful hover, completion, signature help, and diagnostics.
- Literal unions complete valid string values and reject invalid values when the
  expected type is known.
- Optional shorthand, arrays, dictionaries, key-value tables, and function types
  round-trip through the typed annotation model with source spans.
- Meta/module annotations can describe definition-only Lua files and simulated
  require bindings without executing project code.
- Malformed or unsupported annotations remain controlled by the existing
  `doc_annotations` diagnostic setting.
- Examples and docs describe the supported subset honestly, including any
  LuaLS forms intentionally left shallow.

### 70.11 `feat(types): support practical metatable typing for method lookup and operators`

Status: done.

Scope:

- Add conservative static metatable support for common Lua patterns used in the
  language-guide/cheatsheet-style code.
- Infer method style receivers for `obj:method(...)` and `obj.method(obj, ...)`
  when the call target is a known function-with-`self` shape.
- Use `__index` and constructor-style `setmetatable` patterns to propagate
  object shapes:
  - `local M = {}; M.__index = M`
  - `function M:new(...) ... return setmetatable(instance, self) end`
- Support function-call equivalences on table fields so the following both type-check:
  - `lookupObject.y(lookupObject, 1)`
  - `lookupObject:y(2)`
- Add typed handling for selected built-in metamethods when resolvable:
  - `__index` function/table fallback with limited key-based value typing
  - `__newindex` assignment compatibility
  - `__add` binary operator result typing
  - `__tostring` result typing.
- Keep behavior conservative when runtime metatable construction is dynamic:
  - unknown `setmetatable` base/meta inputs,
  - computed `__index` chains,
  - dynamic `getmetatable` mutation.
- Add this as opt-in/gradual-analysis behavior so existing Lua/LuaDoc workflows
  continue to pass when only partial static information is available.

Acceptance:

- In a sample with `AccountShape`, `Account:new`, and `:deposit/:withdraw`, calls
  through `:` can infer receiver and update/check `balance` as `number`.
- For `lookupObject` method examples, both dot-call-with-explicit-self and
  colon-call forms type-check with identical inferred function signatures.
- For `vector` examples:
  - `vectorA + vectorB` infers the result type from `__add` as a vector-like
    table shape,
  - `tostring(vectorSum)` infers `string` from `__tostring`,
  - `vectorA.length` can use typed `__index` fallback when the key is recognized,
  - `vectorA.label = "..."` passes through `__newindex` compatibility checks.
- Fixtures cover constructor/layered-metatable and operator-metamethod cases with
  stable diagnostics and non-regressive behavior for existing features.

### 70.12 `feat(types): infer loop variable, table, and array types`

Status: done.

Scope:

- Add table-shape inference for constructors and updates:
  - infer list-like arrays from sequential numeric entries (e.g. `{ 1, 2, 3 }`);
  - infer record-like object fields from string keys (e.g. `{ name = "Ada", score = 10 }`);
  - infer mixed tables conservatively using unioned field/value shapes.
- Add indexing typing:
  - `table_expr[number_expr]` on inferred arrays yields element type;
  - string-key indexing on record-like tables yields the field value type;
  - unknown-key indexing on arrays falls back to element type with nil union safety
    where appropriate.
- Add loop-variable inference for `for` families:
  - numeric loops infer index variables from loop bounds/steps as `number`;
  - `ipairs` value variable infers array element type and index as `number`;
  - `pairs` key/value variables infer from known table key/value types where available.
- Track table/index operations through aliases and simple locals so `for _, v in ipairs(xs) do` and `for k,v in pairs(t) do` propagate types through scope.
- Keep inference conservative for dynamic iterator shapes, `next`, or unresolved custom
  iterators: those loop variables remain `any` unless annotated/imported.

Acceptance:

- `for i = 1, #scores do` infers `i: number` and supports `scores[i]` as typed
  element usage.
- `for _, value in ipairs(numbers)` infers `value` from `numbers` array element type.
- `for key, value in pairs(player)` infers `key`/`value` from known record fields and
  preserves fallback `any` behavior for dynamic/untyped maps.
- Indexing a literal/typed array and table in one pass emits stable element or field
  types for completions/hover/diagnostics.
- The new inference updates add no regressions to existing table completions, diagnostics,
  or require resolution tests.

### 70.13 `feat(types): inherit table shapes across metatable chains and typed callable objects`

Status: done.

Scope:

- Extend metatable typing to prototype-style inheritance where shape flows through
  metatable chains, not only direct `__index = self` style:
  - `setmetatable(Sub, { __index = Base })` for static inheritance of exported symbols;
  - chained inheritance `setmetatable(Leaf, { __index = Mid })` where each link is known.
- Propagate inherited methods and fields into call-site receiver typing so inherited
  method lookup and completion work when call sites use either `obj:method(...)`
  or `obj.method(obj, ...)`.
- Add typing for object-like callability:
  - `__call` on a table used as a constructor or factory should provide a typed call
    signature for `tableValue(...)`;
  - prefer known constructor return types through `setmetatable`-based class patterns.
- Add partial iterator/metamethod inference for typed iteration hooks where present:
  - typed `__ipairs` return tuple shape for array-like iteration;
  - typed `__pairs` key/value pair shape for map-like iteration.
- Keep behavior conservative on dynamic metatable composition and unknown function
  values supplied to `setmetatable`.

Acceptance:

- In a two-level inheritance sample, `Sub` members and methods become available from
  `Base` for both hover/completion and type-checking.
- `obj:method(...)` and `obj.method(obj, ...)` behave consistently when the target
  method comes from an inherited receiver shape.
- `ConstructorLike(...)` style calls resolve through `__call` and return the intended
  object type.
- `for k, v in pairs(typedMap)` and `for _, v in ipairs(typedArray)` use iterator
  types when `__pairs`/`__ipairs` metadata is statically known.
- Existing dynamic/mixed metatable cases still degrade to `any` or unknown shape when
  confidence is insufficient.

### 70.14 `feat(diagnostics): add inline lint/error suppression comments`

Status: done.

Scope:

- Add inline, line-targeted suppression comments for diagnostics, analogous to
  TypeScript `@ts-expect-error`.
- Define first-class syntax forms:
  - `-- tua-ignore-line [CODE ...]` suppresses matching diagnostics on this line.
  - `-- tua-ignore-next-line [CODE ...]` suppresses matching diagnostics on the
    immediately following line.
  - `-- tua-ignore-file [CODE ...]` suppresses matching diagnostics in the
    current file (useful for gradual migration, discouraged unless configured).
- Support explicit code matching and code-group matching; if no code is provided,
  suppresses all non-fatal diagnostics on the target scope.
- Add typo-safe behavior:
  - unknown diagnostic codes in suppression comments emit `TL...` warnings,
  - empty ignore lists are rejected to avoid silent no-op suppression.
- Integrate with shared diagnostics in CLI, LSP, and snapshot tests so behavior is
  consistent.
- Add config toggles under `[diagnostics]`:
  - `inline_ignores = false` (default),
  - `inline_ignore_scopes = ["tua-ignore-line", "tua-ignore-next-line"]` (extensible),
  - `inline_ignore_max_per_file = 0` (0 = unlimited).

Acceptance:

- The following comments are parsed and enforced consistently:
  - `-- tua-ignore-next-line TL4018`
  - `-- tua-ignore-line TL4018 TL4042`
  - `-- tua-ignore-file TL4018`
- `tua-ignore-next-line TL4018` on a line with no matching TL4018 emits a warning
  that suppression did not match.
- Inline suppression never changes emitted Lua and never changes checker type inference.
- Projects can gate suppression by config and default behavior remains unchanged when
  `inline_ignores` is disabled.

### 70.15 `feat(emit): export Lua code with optional LuaCATS/annotation mode`

Status: done.

Scope:

- Add an opt-in emission mode to preserve useful type information as Lua comments
  so consumers can keep type workflow on exported `.lua`.
- Add `[emit]`-style config for generated annotation style:
  - `annotation_mode = "off"` (default): current erased output.
  - `annotation_mode = "luacats"`: emit LuaCATS-style annotations from Tua types.
- Support preserved artifacts in emitted `.lua`:
  - `---@alias` for type aliases;
  - `---@type` for annotated locals;
  - `---@param` and `---@return` for function signatures;
  - `---@field` for typed table exports where field shapes are stable.
- Keep emitted Lua valid and runtime-equivalent to erased mode.
- Preserve existing behavior for `.lua` passthrough and non-typed input.

Acceptance:

- `local player: Player = ...` emits with `---@type Player` in `luacats` mode.
- `local function make_player(name: string): Player` emits `---@param` and `---@return`
  comments plus a plain Lua function body.
- `type Player = { ... }` emits `---@alias Player` when alias shape is stable.
- Exported `.lua` output remains parse-clean and does not rely on a runtime type
  checker to execute.
- Users can keep existing erased output with `annotation_mode = "off"` while
  enabling the richer mode per project.

### 70.16 `docs(lsp): inventory LuaHelper codes and map missing Tua/LSP support`

Status: done.

Scope:

- Collect LuaHelper diagnostic/code identifiers in use across the language server
  behavior relevant to this project:
  - diagnostics and warnings,
  - lint/check categories,
  - code-action/quick-fix families,
  - annotation/docs tags used by diagnostics or editor surfaces.
- Build a checked-in mapping artifact (for example `docs/luahelper-code-matrix.md`
  or a similar dedicated doc) with columns:
  - LuaHelper code/tag;
  - source of truth in LuaHelper;
  - equivalent behavior in Tua core checker/lint;
  - equivalent behavior in `tua-lsp` editor features;
  - support state: implemented, partial, missing, or intentionally out of scope;
  - exact commit reference for planned implementation.
- Include separate rows for each code/tag that currently:
  - exists in LuaHelper and is implemented in Tua core only,
  - exists in LuaHelper and is implemented in LSP only,
  - exists in LuaHelper but missing in both.
- Add short migration notes for missing entries:
  - likely owner (parser/types/lint/emit/lsp/config/fixtures),
  - prerequisite gap blockers, and
  - planned replacement if Tua intentionally diverges.
- Link missing rows to planned tasks in Phase 11 or later phases before closing the
  gap.

Acceptance:

- The matrix is comprehensive enough to answer:
  - "Is this LuaHelper code implemented in Tua core?"
  - "Does this Tua behavior surface in the LSP?"
  - "Which codes are missing today and why?"
- Every "missing" LuaHelper code has a dedicated follow-up roadmap ticket or a
  justified non-goal decision.
- The phase parity commit list references this matrix when claiming LuaHelper parity
  progress.

### 70.17 `perf(core): reuse cached semantic artifacts for definitions and references`

Status: done.

Scope:

- Remove repeated `lower_parsed_source` and scope construction in definition/reference hot paths by threading cached `HirModule` and `ScopeMap` from `analysis` snapshots into request handlers.
- Ensure `definition`, `prepare_rename`, and `rename` paths resolve local/value/type alias definitions through cached semantic data before falling back to reparsing/mining.
- Add a dedicated helper on `AnalysisSnapshot` that returns the parsed file plus cached `hir/scope` bundle for in-process queries.
- Keep parse-error recovery behavior unchanged for unsynchronized documents.

Acceptance:

- Definition and reference request handlers avoid rebuilding HIR and scope data for unchanged snapshots.
- Regression suite for `definition`, `references`, `prepareRename`, and `rename` remains behaviorally identical.
- New/updated metrics show fewer repeated `analysis.lower.hir` and `analysis.lower.scope` hits on repeated queries.

### 70.18 `perf(core): cache definition->scope resolution for local declarations`

Status: done, folded into 70.17.

Scope:

- Refactor `scope_references_to_definition` to use pre-built `ScopeMap` from file cache where available, instead of rebuilding per call.
- Ensure `scope_references_to_definition` can be called with an optional fast path that reads precomputed `ScopeMap` and only falls back to rebuild for legacy/unavailable cached inputs.
- Preserve declaration-kind-specific behavior for `Local`, `Function`, and `Parameter` symbols.
- Avoid changing semantics for table-field/global/module-export fallbacks.

Acceptance:

- Local/reference lookups for symbol IDs do not allocate a new scope map in the common cached path.
- Result ordering/deduplication remains stable for existing `references` and `references`-adjacent tests.
- No regressions in `scope_references_to_symbol` coverage.

### 70.20 `feat(lsp): add project-wide reverse reference index`

Status: done.

Scope:

- Extend snapshot indexing with a snapshot-owned lazy `definition_id -> Vec<Reference>` reverse edge index, so non-reference requests do not pay the indexing cost.
- Use stable identity keys derived from path + declaration span / symbol ID to support O(1)/sublinear `textDocument/references` collection.
- Keep handling for:
  - declaration inclusion toggle;
  - module/type alias/global alias fallback paths;
  - exported-name and shadowing edge cases.
- Integrate invalidation hooks so edited files and importer chains refresh only affected buckets.

Acceptance:

- `AnalysisHost::references` for local and global symbols no longer linearly scans every file in the project.
- Reference ordering and dedup remain deterministic and stable.
- No duplicate/missing references for existing cases in `tua-lsp` reference tests.

### 70.21 `feat(lsp): prefilter global references via symbol index`

Status: done.

Scope:

- Before token scanning fallback paths, query `SymbolIndex` for probable name matches and module-export candidates.
- Restrict cross-file scan candidates to files that could contain a declaration/export for the target symbol.
- Preserve current behavior for globals, aliases, exports, and unresolved references.
- Ensure `references` still returns declaration when `include_declaration = true`.

Acceptance:

- Global/reference lookups reduce the number of scanned files on realistic multi-module projects.
- Existing module/alias/global reference tests still pass unchanged.

### 70.22 `chore(perf): add profiling/bench coverage for lookup hot paths`

Status: done.

Scope:

- Add `TUA_PROFILE`-targeted spans around definition/reference query stages in `analysis` and LSP adapters.
- Add focused benchmark or micro-test harness for:
  - repeated `definition` at same position,
  - repeated `textDocument/references`,
  - rename prepare/readiness sequence.
- Record baseline and regression targets in project notes so future changes can be compared.

Acceptance:

- Developers can attribute query-time cost to cache-miss vs cache-hit components.
- Reference/lookup performance claims are repeatable and validated in CI-like local benchmarking runs.

### 70.22.5 `perf(ide): cache semantic facts for fast editor queries`

Status: done.

Scope:

- Add snapshot-owned semantic facts for cached project-aware type environments
  and inlay results.
- Route hover, completion, completion resolve, signature help, and inlay hints
  through cached semantic facts where possible.
- Cache repeated completion requests within a snapshot so completion resolve and
  repeated editor requests do not rebuild the same item list.
- Add profiling spans for hover, completion, completion resolve, signature help,
  inlay hints, receiver resolution, module-export completion, LOVE field
  completion, and semantic fact construction.
- Extend the LSP profiling fixture to measure `completionItem/resolve` and
  `textDocument/signatureHelp`.

Acceptance:

- Repeated completion and completion resolve share cached snapshot results.
- Hover, inlay hints, completion, and signature help agree on inferred LOVE
  object types through one project-aware semantic environment.
- Existing parse-recovery behavior for incomplete completion input remains
  permissive.

### 70.22.6 `perf(ide): reuse warm snapshots for repeated navigation`

Status: done.

Scope:

- Memoize the latest `AnalysisSnapshot` by host revision so repeated LSP
  requests against unchanged inputs reuse the same module graph, type database,
  symbol index, call hierarchy maps, and lazy request caches.
- Share snapshot-owned lazy caches across cloned snapshots for semantic facts,
  completion lists, definition lookup results, and reverse-reference indexes.
- Add an exact-position definition cache so repeated cmd+click/go-to-definition
  requests do not recompute the same result inside a warm snapshot.
- Extend the LSP profiling fixture with `definitionAgain`, an immediate repeat
  of the same `textDocument/definition` request.

Acceptance:

- A second `host.snapshot()` with no revision change does not walk the per-file
  cache or change cache hit/miss counters.
- Definition cache entries survive cloned warm snapshots until an input edit or
  config/root/catalog change bumps the host revision.
- `make profile-lsp` reports both cold `definition` and warm
  `definitionAgain` rows for navigation regression checks.

### 70.23 `feat(ide): compact inferred structural type presentation`

Scope:

- Improve the default presentation of inferred types in hover and inlay text:
  inferred primitives and named types remain fully visible, while inferred structural
  table/function/union shapes are compacted.
- For inferred anonymous shapes, prefer compact tokens such as `{ ... }` and ellipsis-
  style summaries for inlay hints instead of emitting the full literal shape inline.
- Keep explicit annotations and alias references unchanged: `MyType`, `string`, and
  `number` stay readable without expansion.
- Preserve a full-detail path through the existing hover/resolve channel (or equivalent
  existing client interaction point) that can open the complete inferred shape on
  demand.
- Reuse current long-type trimming policy when full detail is requested and the shape
  still exceeds display limits.

Acceptance:

- Primitive and named types (for example `number`, `string`, `Player` ) are shown
  in full by default.
- Inferred structural values default to a compact representation with `...` in
  popovers and inlay text.
- A user-initiated expand path shows the full inferred shape when available and
  readable.
- If that shape is too long, the current trimming behavior applies after expansion
  instead of silently dropping detail.
- Rendering changes only affect presentation and do not alter inference, checking,
  or emitted Lua.

### 70.24 `feat(lint): close LuaHelper assignment and operator diagnostic gaps`

Scope:

- Add focused lint/type checks for LuaHelper parity rows tracked in
  `docs/luahelper-code-matrix.md`:
  - assignment arity (`CheckErrorAssignParamNum`);
  - local declaration arity (`CheckErrorLocalParamNum`);
  - exact no-use assignment when a local is only assigned and never otherwise read;
  - broader binary operator type mismatch checks (`CheckErrorBinopType`);
  - optional distinction between unused local functions and local functions that
    are referenced but never called.
- Reuse the shared scope map and type resolver instead of adding one-off AST walks.
- Keep diagnostics configurable under `[diagnostics]`.

Acceptance:

- Fixture tests cover each mapped LuaHelper gap with stable `TL40xx` or `TL10xx`
  diagnostics.
- Existing dynamic or `any`-typed Lua remains permissive when static proof is not
  available.
- The LuaHelper code matrix is updated from missing/partial to the new support
  state for each landed check.

### 70.25 `feat(luadoc): model LuaHelper overload, vararg, and identifier annotations`

Scope:

- Extend LuaDoc/LuaHelper annotation compatibility for rows tracked in
  `docs/luahelper-code-matrix.md`:
  - `---@overload`;
  - `---@vararg`;
  - the `identifier` annotation keyword, if representative fixtures show a useful
    static meaning for Tua.
- Feed supported overload and vararg signatures into hover, completion resolve,
  signature help, and call diagnostics.
- Decide explicitly whether `identifier` stays unsupported, becomes an alias for
  `string`, or becomes a narrow literal/name-like type.

Acceptance:

- Parser and type-environment fixtures cover supported forms with source spans.
- Signature help can choose and display overloads where the call target is known.
- Unsupported annotation forms remain non-fatal and controlled by
  `doc_annotations`.
- The LuaHelper code matrix records any intentionally unsupported form as a
  non-goal with the reason.

## Phase 13: LOVE Domain Extensions

This phase explores Tua's identity as a LOVE-focused Lua language. The goal is
to add domain-aware workflows without turning Tua into a closed game engine.
Core Lua semantics should remain predictable, and extensions should be opt-in,
conservative, and tooling-aware.

### 71. `docs(plugins): design domain extension model`

Scope:

- Write a design document for Tua domain plugins.
- Define what plugins may extend:
  - new top-level declarations;
  - domain blocks;
  - validation passes;
  - lowering metadata;
  - completions, hover, diagnostics, and editor metadata.
- Define what plugins may not do:
  - rewrite arbitrary expressions;
  - change core Lua/Tua semantics;
  - execute project runtime code during analysis;
  - emit raw Lua strings as the primary lowering API.
- Compare possible implementation choices:
  - Rust-only built-in plugins;
  - Lua-authored plugins hosted by Rust through a restricted API;
  - static manifest plugins that provide metadata but no executable code.

Acceptance:

- The design states v1 plugin boundaries clearly.
- The document includes at least one sprite/animation example and one asset
  validation example.
- Security and determinism constraints are explicit.

### 72. `feat(plugins): add experimental plugin registry`

Scope:

- Add a plugin registry in `tua-core`.
- Register built-in domain plugins through Rust first.
- Expose plugin-provided:
  - declarations;
  - diagnostics;
  - completions;
  - hover snippets;
  - lowering hooks or structured lowering requests.
- Keep the registry disabled for third-party executable plugins until the API is
  proven.

Acceptance:

- Existing behavior is unchanged when no plugin contributes syntax.
- A test plugin can register one declaration and one diagnostic.
- Plugin diagnostics use stable Tua diagnostic codes or a reserved plugin code
  range.

### 73. `feat(luajit): add JIT-friendly typing and optimization-oriented constraints`

Scope:

- Add a JIT-oriented opt-in mode (`[jit]` or `[performance]` section in
  `tua.toml`) that encourages static-shape friendly Lua for LuaJIT.
- Add conservative diagnostics for variable value-shape drift:
  - assignments that change inferred concrete variable type (for example `number`
    to `string`) are warnings unless explicitly allowed by `any`;
  - table-field value-type changes are warnings unless field is `any`-typed or
    declared as a deliberate dynamic escape.
- Add conservative diagnostics for runtime table-shape mutation:
  - adding/removing object fields after construction (shape drift),
  - changing per-iteration field types in the same logical table,
  - introducing unknown metamethod/table-index behavior in hot loops.
- Add explicit escape hatches:
  - `any` permits deliberate dynamic behavior,
  - opt-in annotations disable specific JIT-oriented warnings at function or
    block scope,
  - optional `@luajit.pragma` or equivalent config markers to document exceptions.
- Add quick fixes where practical:
  - field type annotations,
  - table pre-seeding,
  - local variable re-declaration into a typed temporary.
- Keep current diagnostics enabled by default.

Acceptance:

- In JIT mode, the checker reports type-drift for variables where static type
  changes are clear and unsupported by an explicit `any`.
- Table values created with `local obj = { ... }` prefer fixed-shape usage and emit
  warnings on late key insertion patterns.
- Loop-intensive code paths gain actionable hints instead of speculative
  inference when shapes change.
- JIT diagnostics can be toggled off entirely in `tua.toml` and via LSP settings.

### 74. `feat(ecs): explore default ECS domain support with tiny-ecs integration`

Scope:

- Add an experimental ECS domain extension under `tua-core` that models
  component shapes, entities, systems, and entity IDs in a type-aware way.
- Define a default ECS profile that maps to a tiny-ecs-style backend:
  - typed component constructors,
  - strongly typed world/system APIs,
  - systems over typed component query lists.
- Keep ECS additions conservative and opt-in (`[ecs]` in config) so projects without
  ECS usage are unaffected.
- Integrate with the plugin registry path from item 72:
  - reusable ECS declarations,
  - ECS diagnostics,
  - completion + hover + signature help for ECS APIs,
  - code-action suggestions for common ECS mistakes.
- Include migration notes for existing tiny-ecs projects:
  - how to represent existing component schemas,
  - how to migrate dynamic component tables to typed schemas.

Acceptance:

- A typed ECS example can represent entities, components, and system queries with
  stable completions and hovers.
- Missing or mismatched component fields are diagnosed at edit time.
- Invalid system query/filter usage is surfaced as diagnostics with deterministic
  code locations.
- ECS checks remain optional and do not alter baseline Lua/LOVE analysis when
  disabled.
- At least one reference example is added for tiny-ecs interoperability or a
  compatible minimal adapter.

### 73. `feat(vectors): add LOVE-friendly Vec2 support`

Scope:

- Add built-in `Vec2` type support for common LOVE game code.
- Provide a `vec2(x, y)` constructor with inferred `Vec2` return type.
- Decide whether emitted Lua uses:
  - a tiny generated runtime module;
  - a configured external library such as `brinevector`;
  - plain table output for the first version.
- Add completion, hover, and signature help for supported vector fields and
  operations.

Acceptance:

- `local position: Vec2 = vec2(100, 200)` type-checks.
- `position.x` and `position.y` complete and hover as `number`.
- Emitted Lua is LuaJIT/LOVE-compatible and documented.

### 74. `feat(sprite): prototype sprite animation DSL`

Scope:

- Prototype a LOVE sprite/animation declaration, likely as a built-in plugin.
- Explore syntax for:
  - image path;
  - frame size;
  - animation names;
  - frame ranges;
  - FPS;
  - looping mode.
- Prefer structured lowering metadata over raw Lua string emission.
- Consider anim8-compatible output as the first target.

Example direction:

```tua
sprite Player
  image "assets/player.png"
  frame 16x16
  anim idle from 0..3 fps 8
  anim run from 4..9 fps 12
end
```

Acceptance:

- The prototype parses one sprite declaration form.
- Missing image assets and invalid frame ranges produce diagnostics.
- Emitted Lua can create an anim8-compatible grid/animation structure or a
  documented placeholder representation.
- The feature remains isolated enough to change syntax before stabilization.

### 75. `feat(assets): add richer LOVE asset metadata`

Scope:

- Extend asset validation beyond path and extension checks.
- Record asset metadata where practical:
  - kind;
  - relative path;
  - extension;
  - source span;
  - inferred LOVE type;
  - optional dimensions or duration when available without running game code.
- Surface metadata through hover, completion resolve, diagnostics, and future
  plugin hooks.
- Keep binary parsing optional and conservative.

Acceptance:

- Asset hovers show path, inferred LOVE type, and validation status.
- Completion can suggest project asset paths in asset-loader positions.
- Unsupported or missing assets produce diagnostics with actionable messages.
- Metadata is available to built-in plugins through a stable internal API.

### 76. `docs(tooling): explore optional Feather integration hooks`

Scope:

- Write an exploratory document for optional Feather/tooling integration.
- List possible integration points:
  - asset inspectors;
  - sprite/animation previews;
  - generated metadata export;
  - diagnostics surfaced in external tools;
  - plugin-provided inspector panels.
- Keep Feather integration optional and outside the core runtime/compiler path.
- Identify which hooks belong in Tua core and which belong in editor/tooling
  adapters.

Acceptance:

- The document proposes at least three concrete integration hooks.
- Each hook states required Tua metadata and whether it needs compiler, LSP, or
  editor support.
- No Feather dependency is added to the core crates in this commit.
