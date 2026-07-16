---
icon: lucide/map
---

# Roadmap

Tua tracks product direction as **Now, Next, Later**. This page describes
outcomes, not commit numbers. Completed implementation detail is preserved in
the [historical v1 ledger](archive/implementation-ledger-v1.md).

The [Capability Matrix](capabilities.md) is authoritative for what works today.

## Now

### Ship And Observe The Coordinated 0.1 Release

- exercise the checksummed platform server and VSIX pipeline from a real tag;
- keep CLI, server, extension, changelog, and migration notes synchronized;
- use representative completion and editor-contention profiles as release
  gates;
- fix correctness or responsiveness regressions before expanding language
  syntax.

### Deepen Everyday LOVE Workflows

- harden asset path navigation and diagnostics across realistic project roots;
- refine sprite-sheet preview and SignalBus graph workflows from user feedback;
- keep ECS opt-in until its diagnostics and completion remain predictable in
  larger projects;
- maintain one generated LOVE source for signatures, enums, callbacks,
  inheritance, and documentation.

## Next

### Improve Semantic Precision Without Losing Lua Flexibility

- broaden control-flow and metatable inference only where identities are
  compiler-proven;
- expand LuaDoc/LuaCATS interoperability for common existing Lua libraries;
- improve safe project-wide refactors and module changes;
- preserve `any` and ordinary Lua as explicit dynamic escape paths.

### Make Releases Easier To Trust

- add code signing when sustainable signing identities and key management are
  available;
- test upgrade and rollback behavior between adjacent extension/server
  versions;
- publish compatibility and migration notes for every minor release;
- add more architectures only when CI can build and test them natively.

## Later

### Revisit Storage And Extensibility With Evidence

- compact generated LOVE data only if measured compile time, binary size, or
  maintenance cost justifies replacing generated Rust source;
- revisit an external tooling bridge only after a concrete local consumer
  exists;
- reconsider generics when identity-preserving APIs cannot be modeled clearly
  with concrete types;
- keep executable third-party plugins out until ownership, ordering, security,
  and performance contracts can be made stable.

## Planning Rules

- Current behavior belongs in the specification and capability matrix.
- Product priorities belong here.
- Completed implementation detail belongs in dated archives or the changelog.
- A roadmap item moves only when implementation, tests, and user documentation
  agree.
