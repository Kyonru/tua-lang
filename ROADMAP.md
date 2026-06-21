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
