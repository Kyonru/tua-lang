# Tua Roadmap

This repository-level file tracks pending product and engineering priorities.
It is an internal planning aid, not documentation of current behavior or a
release commitment.

Current user-facing behavior belongs in `docs/specification.md` and the focused
language, tooling, and workflow pages under `docs/`. Completed work belongs in
`CHANGELOG.md` or Git history.

## Now

### Harden The Current 0.2 Release Line

- Exercise the checksummed platform server and VSIX pipeline from a real tag.
- Keep CLI, server, extension, changelog, and migration notes synchronized.
- Use representative completion and editor-contention profiles as release
  gates.
- Fix correctness or responsiveness regressions before expanding language
  syntax.

### Deepen Everyday LOVE Workflows

- Harden asset path navigation and diagnostics across realistic project roots.
- Refine sprite-sheet preview and SignalBus graph workflows from user feedback.
- Benchmark the object-pool runtime on representative LOVE populations.
- Maintain one generated LOVE source for signatures, enums, callbacks,
  inheritance, and documentation.

## Next

### Improve Semantic Precision Without Losing Lua Flexibility

- Broaden control-flow and metatable inference only where identities are
  compiler-proven.
- Expand LuaDoc/LuaCATS interoperability for common existing Lua libraries.
- Improve safe project-wide refactors and module changes.
- Preserve `any` and ordinary Lua as explicit dynamic escape paths.

### Make Releases Easier To Trust

- Add code signing when sustainable signing identities and key management are
  available.
- Test upgrade and rollback behavior between adjacent extension/server
  versions.
- Publish compatibility and migration notes for every minor release.
- Add more architectures only when CI can build and test them natively.

## Later

### Revisit Storage And Extensibility With Evidence

- Compact generated LOVE data only if measured compile time, binary size, or
  maintenance cost justifies replacing generated Rust source.
- Reconsider generics when identity-preserving APIs cannot be modeled clearly
  with concrete types.
- Keep executable third-party plugins out until ownership, ordering, security,
  and performance contracts can be made stable.

## Planning Rules

- Keep this file limited to pending outcomes, not implemented feature
  inventories.
- Do not use roadmap placement as evidence that a feature is available.
- Move implemented behavior into the relevant docs and remove the completed
  roadmap item.
- Add priorities only when ownership, user value, and a verification path are
  clear.
