# M8 bounded conversation history

This slice completes the optional transcript entry in M8. Typed-context checkpoint
persistence, durable action reconciliation and the complete Save recipe remain
pending. It does not merge or publish the feature branch.

## Contract

`options.history = { capacity, maxBytes? }` enables scalar presentation records.
Defaults remain disabled. Count and total string-byte budgets bound retention;
oldest records are evicted and individually oversized records are skipped without
interrupting playback. Snapshot byte counts include every string field, not table
allocator overhead. No record retains a context, event/action payload or media tree.

Successful line entry records full displayed text. Accepted selections record the
selected translated reply before the next line. Failed presentation/transition,
invalid choice and suspension do not append. Refresh preserves original translations.
Browsing owns copies and never executes dialogue. Redaction removes by session-local
ID, clear retains the monotonic ID sequence, and dispose releases storage.
Cancel/restart retain history until the host clears it. Checkpoints exclude history;
restore starts without old transcript content or an automatically replayed line.

## Qualification

`make verify` passed after implementation: workspace tests (including 2,159 core
unit tests with one existing ignored test), LSP/CLI integrations, clippy,
model/feature examples, extension checks, JSON and whitespace verification.


- Focused emitted-Lua regressions pass for plain/action players and typed-context
  players, including identified events, rejected/hidden/suspended choices, resolver
  failure/reentrancy, original translation retention, redaction, ring eviction,
  oversized entries, 1,000 repeat visits, clear/restart/cancel/dispose and restore.
- Imported/forwarded history contracts retain field types, hover and completion;
  invalid entry kinds, redaction arguments and field assignments are rejected.
- The maintained typed feature tour formats/checks/builds and its history example
  executes in standalone Lua. An isolated typed-context example also checks,
  builds and executes with history enabled and typed returned byte counts.
- Native LÖVE 11.5 / LuaJIT 2.1.1700008891 passes both fixture families with JIT
  enabled and disabled. Existing audio/controller regressions pass, including
  100 rendered frames per JIT mode. No listening-quality assessment is claimed.
- Authored Lua/Tua and Rust formatting, generated model-contract drift, JSON,
  documentation build and whitespace gates pass.

Native compiler provenance: Tua 0.11.0, debug, aarch64-apple-darwin, revision
`335af4c2553157ca76cc430f0ec4921b77604d38` plus this slice's working changes. Executable SHA-256:
`d9149b3284a351076e4f1a43d3014d4d8852a790f9003fee66698097f72a5897`.
The subsequent model-contract edit only removes a trailing blank line.
Standalone coverage uses Lua 5.4.8; no separate Lua 5.1/LuaJIT executable or
installed editor UI run is claimed.

## Authoring observation

The current contract parser rejects an inline nested record with an optional field:
`type Options = { history?: { capacity: number, maxBytes?: number } }`.
A named `HistoryOptions` record is accepted and is used here; this does not change
the public options syntax. No parser or formatter fix is claimed in this slice.
