# M8 version-3 settled registry checkpoints

The host opts in with `checkpointEffects = "settled"`. Registry-backed players
then export v3 at line/choice/wait boundaries only, with `actionState = "settled"`.
Ready actions, executing handlers, pending handles and unconsumed resolutions
remain excluded. Default registry rejection and v1/v2 data/graphs are unchanged;
setting the policy on a plain/context-only player does not change its version.

Restore never dispatches actions or emits events. Typed-event clocks and fired
flags survive; later explicit updates may emit unfired notifications. Action IDs,
names, validated payloads, effects, targets and registry presence enter graph
identity. Context locals remain owned and validated; current facts/handlers are
rebound. Validators and handler semantics still require a host compatibility-key
change. There is no automatic version migration or arbitrary pending-work resume.

The marker means no action is attached or executing in this player, not durable
acknowledgement or proof that detached host work stopped. Recovering
an older position can revisit an effect after a crash: the existing durable-action
recipe or an external idempotency authority still owns that transaction. The
same warning applies to host effects performed for notifications. No closures,
handles or transaction receipts are added to core checkpoints.

## Qualification

Permanent emitted-runtime regressions cover context/action/event-only factories,
line/choice/suspended wait, typed-event timing and false payloads, changed action
identity/payload/effect/targets, invalid markers/status/locals, ready/pending/
unconsumed/synchronously executing actions, failure outcomes, default exclusion,
and restore without dispatch or transcript replay. Imported typing tests cover
valid/invalid policies and marker values plus hover and completion.

`examples/dialogue-actions/verify-checkpoints.py` checks/formats/builds the typed
example, executes emitted Lua and optionally runs all v1/v2/v3 fixture families
under native LÖVE. Separate native write/read processes persist a v3 checkpoint
and reward counter together, then verify that restore does not call the handler.
Each JIT mode uses a unique test save identity; its save files are removed on
successful completion.
These process restarts do not establish power-loss or external-effect atomicity.

The Lua fixture backends use test-only checksums; the native v3 disk round trip
uses Save's real SHA-256 backend. No renderer/audio API changes, installed-editor
UI qualification or performance improvement is claimed.

Native compiler provenance: Tua 0.11.0 debug, `aarch64-apple-darwin`, revision
`9e8558c867fca24609bca917ea74bce4b324889e` plus this slice's changes. SHA-256:
`0f8517662f45b7d322918ddea01b337b32e055b6cb3ae029bebbe16c09f1de21`.
Standalone execution uses Lua 5.4.8; native execution uses LÖVE 11.5 /
LuaJIT 2.1.1700008891 with actual JIT status printed. No separate standalone
Lua 5.1/LuaJIT executable is claimed.

Final `make verify` passed: workspace tests (including 2,165 core tests, with
one existing ignored test), Clippy, model examples, all 292 extension tests,
TextMate scopes, theme contrast, JSON validation and diff checks. Documentation
build and generated `llms.txt` synchronization passed separately. Focused emitted
runtime/editor tests and native checkpoint qualification passed with JIT enabled
and disabled. Authored runtime, contract, example and fixture formatting checks
also passed against the rebuilt compiler.
