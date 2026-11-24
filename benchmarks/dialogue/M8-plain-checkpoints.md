# M8 initial plain-dialogue checkpoints

Implemented on the feature branch. This is the first M8 slice, not completion of
M8. Typed context/locals, action reconciliation, transcripts and a full persistence
recipe remain pending. No merge or publication is included.

## Contract and scope

Opt in with a document ID and `checkpointVersion`. `checkpoint()` exports owned
version-1 JSON-shaped data at line, choice and wait boundaries. `restore()` accepts
a fresh idle player and validates identity, graph, position, timers and delivery
flags before committing state. A failing resolver leaves the target idle, and
mutation of caller-owned input during a resolver cannot change committed fields.

The exact canonical graph signature is length-prefixed rather than hashed. It
includes IDs, schema, targets, waits and event payload/timing but permits prose and
presentation edits. Numeric encoding follows Save's locale-independent 17-digit
format. Bounded work, depth, entry/string counts and array checks reject malformed
or oversized data. This is compatibility validation, not authenticity protection.

Restore retains elapsed timers, fired-event flags, saved locale and suspension.
Lines resume fully revealed. If the revealed clock had not started, it starts at
zero; otherwise it retains its saved value. A `restored` snapshot flag keeps the
optional audio adapter silent for that visit. New node entry clears the marker.
Host args/resolvers are rebound; live identity counters are fresh. No handles,
closures, wall clock, partial reveal, gameplay context or audio source is saved.

All context/action/event-registry documents, branches and conditional choices are
rejected, including action documents currently displaying a line after successful
work. There is no silent effect replay or automatic migration. Notifications can
still affect host state; crash-safe exactly-once effects require host atomic
persistence or durable idempotency, not the checkpoint's fired flags.

## Integration findings

A module-graph regression dropped inferred field entries when a factory result
was returned through a named local, because no per-field declaration span existed.
The fix retains the already inferred types and anchors navigation at the return
expression. Independent module tests cover ordinary and parenthesized returns;
the checkpoint test covers re-exports, hover/completion and invalid field use.
The shared JSON helper also received formatter-only cleanup alongside its internal
usage comment; that cleanup is kept separate from checkpoint behavior.

## Focused qualification

- The typed authored example builds and round-trips line, choice and suspended
  wait boundaries without disk or LÖVE.
- Runtime regressions cover corruption, identity/graph changes, compatible prose,
  fired-event ownership, locale changes, fresh-player enforcement, resolver failure
  and mutation, Unicode/bounds validation, and registry exclusions.
- Checkpoints round-trip through Save's actual schema/JSON encoder and decoder
  using an in-memory test backend. The test checksum is not production SHA-256.
- Standalone Lua 5.4.8 and LÖVE 11.5 / LuaJIT 2.1.1700008891 pass native checkpoint
  tests with JIT on and off. A canonical graph with a fractional wait matches
  between standalone Lua and native LuaJIT.
- The audio example passes emitted Lua and 100 rendered native frames in each JIT
  mode, including a real checkpoint restore that produces no chatter/recording.
- No standalone Lua 5.1/LuaJIT executable, installed editor UI or listening-quality
  assessment is claimed. LÖVE supplies LuaJIT compatibility coverage.

Compiler provenance for native qualification: Tua 0.11.0, debug,
`aarch64-apple-darwin`, revision `056666434d33ee4b816369f302b89004977ccee5` plus this slice's
working changes. Executable SHA-256:
`11646e9bc6b28e274bda4fecbe52d6cb5df218d3d481aacac59152b9ec8a5073`.

## Final verification

`make verify` passed, including workspace tests, LSP, CLI, clippy, model/feature
examples and extension checks. After the final runtime-only bounds and byte-order
refinements, `RUST_MIN_STACK=16777216 cargo test -p tua-core` passed (2,158 unit
tests, one ignored, plus the integration suites). The first direct invocation
omitted the Makefile's stack setting and overflowed; the configured rerun passed.

The rebuilt compiler checked and built both checkpoint/action examples. Final
native tests passed with JIT on and off, including Save JSON round trips and a
cross-runtime canonical graph with fractional waits and Unicode node IDs. The
rendered audio example passed 100 frames per mode and remained silent on restore.
Shared Tua/Lua formatting, Rust formatting, model-contract drift, documentation
build, JSON metadata and whitespace checks passed. These checks do not claim
installed-editor UI coverage or complete M8 action/context persistence.
