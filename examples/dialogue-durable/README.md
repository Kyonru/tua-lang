# Durable dialogue action recipe

This typed host example combines `Dialogue.actions` and `Save.contract` without
changing their runtimes. One purchase costs five coins and adds one shell.
Gameplay counters and the action receipt share one authoritative save record.
Core dialogue checkpoints still reject action registries, including pending work.

## Try it

```sh
tua fmt --check
tua check
tua build
love build
```

The headless demo explicitly initializes its own profile, then makes one purchase
or recovers its outstanding purchase and exits. It uses LÖVE identity
`tua-dialogue-durable-demo`. The profile starts with ten coins; later invocations
keep that progress. After two successful purchases, a further attempt records a
stable declined result. This demo deliberately performs a purchase on launch;
a game should call `begin()` only in response to a deliberate player action.

Run the deterministic standalone regressions without touching that profile:

```sh
python3 verify.py
python3 verify.py --love /Applications/love.app/Contents/MacOS/love
```

Native verification uses unique test identities. It runs fault-injected fixtures
with real SHA-256, then separate LÖVE processes for prepare, commit, receipt replay
and acknowledgement verification. It removes its two test save files afterward.
Standalone fixtures use a test-only checksum. These are simulated crash windows
and process restarts, not physical power-loss tests.

## Ownership and protocol

- `profile.tua` defines a bounded 8-KiB Save record and opens fresh contracts.
- `journal.tua` validates operation identity and owns explicit persistence calls.
- `session.tua` attaches the authored action to an outstanding operation. Recovery
  prepares a fresh player but never calls its handler automatically.
- `scene.tdialogue` and `actions.tua` define the ordinary authored action contract.
- `types.d.tua` shares typed record/backend boundaries without runtime imports.

1. **Prepare.** Allocate a monotonically increasing operation number and save its
   intent before dispatch. `demo:1` identifies this occurrence, not the authored
   action definition. The record includes document/action/node IDs, a required host
   content version, payload and price. Another purchase cannot start while one
   intent remains outstanding.
2. **Commit.** Reopen the record. If it already contains a receipt, return its stored
   boolean result. Otherwise update coins/shells and write the result in the **same
   save candidate**. No reward is applied to another service or mutable game object.
   Read the committed profile to refresh the game's visible state.
3. **Reconcile.** After interruption, `recover()` loads and validates the record,
   checks the authored action and outcome targets, and attaches a fresh player at
   that action. The host explicitly calls `runAction()`. An existing receipt returns
   its result without charging again; prepared work may commit once. Even `false`
   is a durable result, so a declined purchase is not re-evaluated on replay.
4. **Acknowledge.** After the host consumes the outcome, clear the intent. Keep the
   operation counter: a stale key cannot execute after acknowledgement or after a
   later purchase begins. This recipe retains only one intent/receipt, not an
   unbounded ledger.

`journal.abandon(key)` can explicitly discard a verified **prepared** intent without
spending coins; it keeps the sequence number. It rejects committed receipts. Ordinary
player cancellation/disposal does not discard durable intent. Recovery therefore
requires an explicit host decision even if playback was cancelled before a crash.

## Failure behavior

Every operation uses a fresh Save contract. A failed write or read-back may still
have persisted the new generation; retry by reopening and reconciling the receipt,
not by blindly granting the reward again. Failed dispatch faults the current player;
use a fresh recovered session rather than trying to resume that faulted object.

All load issues stop reconciliation, including `damaged_slot_unverified` even when
an older snapshot is available. The recipe never calls `acceptRecovery()` or
`reset()` automatically. Accepting an older backup can lose a newer receipt and
must remain an explicit product recovery decision. Ordinary `recover()` never
creates defaults; only explicit `initialize()` can create a new profile.

Save validates the record shape. The journal separately checks the account,
integer counters, operation sequence/key, compatibility version, IDs, payload,
price and prepared/committed result consistency. Snapshots are owned copies.
Bump `merchant-v1` when host semantics change; this small recipe is deliberately
specific to one known action and is not a general action-graph checkpoint format.

## Limits and adaptation

This is a **single-writer local-state transaction recipe**. Its no-duplicate result
applies to counters and receipts stored together in that record. Save's verified
two-slot writes do not add `fsync`, multi-process locking, cloud conflict handling,
anti-cheat guarantees or rollback protection. See [Save recovery](../../docs/save-contracts.md#recovery-and-errors).

For remote purchases, achievements or inventory services, send the same operation
key and immutable payload to an authority that supports idempotent requests and
outcome lookup. A local receipt cannot atomically cover an external side effect.
Async handles, wall clocks and callbacks are never persisted here; arbitrary
non-resumable work remains rejected by core checkpoint APIs.

For several profiles, choose stable profile-specific storage/key namespaces. For
concurrent operations, use a transactional authority or durable ledger with an
explicit retention policy; do not simply remove this example's outstanding-intent
guard. Checkpoint save/resume, transcript storage and this host action protocol
remain distinct operations. A complete ordinary-dialogue Save recipe is still
separate M8 work.
