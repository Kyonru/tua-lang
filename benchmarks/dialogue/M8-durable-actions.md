# M8 durable host-action reconciliation

This slice provides an explicit typed host recipe, using existing Dialogue and
Save APIs. It does not change runtime dispatch, register a new helper, or enable
core checkpoints for action/event registry documents. General checkpoint-envelope
Save recipes and action-enabled checkpoint representation remain separate work.

## Protocol and scope

An account-scoped monotonically increasing operation ID is persisted with immutable
intent before dispatch. A single local Save candidate contains both the gameplay
counter changes and a committed boolean receipt. Reconciliation opens a fresh
contract, validates the record and authored action, and returns a committed receipt
instead of applying the operation again. Acknowledgement removes the receipt but
keeps the sequence. Only prepared work can be explicitly abandoned.

The example enforces one outstanding operation and single-writer use. Recovery
never defaults, automatically accepts damaged data or dispatches work. The local
transaction claim excludes external side effects, concurrent writers, rollback,
cloud synchronization and power-loss durability. The latter limits match Save's
existing two-slot protocol. External authorities must honor an immutable operation
key and support outcome lookup; local receipts alone cannot provide that guarantee.

## Qualification

- The complete `make verify` gate passes: workspace tests, Clippy, model
  reference checks, all 292 extension tests, TextMate/theme checks, JSON
  validation and whitespace checks. Documentation builds and generated model
  contract drift checks also pass.

- The permanent CLI regression checks/builds the authored typed example, executes
  its emitted-Lua recovery fixtures and rejects invalid imported profile/receipt
  types. Typed source and explicit authored Lua formatter checks pass.
- Deterministic fault injection covers prepare, commit and acknowledgement before
  writes, after bytes persist and on failed read-back. Torn writes at each phase
  block recovery without writes/reset/implicit fallback. Stale keys, changed
  content/payloads/outcome targets, repeated receipt consumption, explicit prepared
  abandonment, cancellation and false receipts are covered.
- Standalone Lua 5.4.8 executes the real Save schema/envelope pipeline with a
  test-only checksum. Native LÖVE 11.5 / LuaJIT 2.1.1700008891 uses real SHA-256.
- With JIT enabled and disabled, separate native processes prepare an intent,
  commit its purchase, replay/acknowledge the receipt, then verify unchanged
  counters and a distinct second-operation ID. Each mode uses a unique test save
  identity and removes its save files. These are process restarts and injected
  failures, not physical power-loss or fsync qualification.
- No separate Lua 5.1/LuaJIT executable, installed editor UI or renderer/audio
  behavior change is claimed. The recipe itself is headless.

Native compiler provenance: Tua 0.11.0, debug, `aarch64-apple-darwin`, revision
`945cbbab3cbc5011371b3d3aac68ee5acb44db27` plus this slice's working changes. Executable SHA-256:
`686a2b0d864345a5ded71c07e6111dafa961f5bf775474931295f3a21fa5eda9`.
