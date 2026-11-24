# M8 optional checkpoint persistence recipe

The `dialogue-checkpoint` example now composes existing `Save.contract` and
version-2 dialogue checkpoints. It adds no built-in or runtime API. The concrete
host envelope owns the `visits` local schema, its own version and a 1 MiB byte
limit. Save validates shape; the fresh player's restore validates identity,
compatibility, timers and availability against rebound facts. No defaults, writes,
action dispatch, transcript replay or implicit restart happen during load.

Storage timing is explicit. Missing checkpoints return false. Load/save issues
raise to the host; damaged slots block writes. Every save reopens the contract,
observing prior writes whose acknowledgement failed. This is a single-writer
recipe; it makes no concurrency, external-effect or physical power-loss guarantee.
Host schema migrations remain explicit. General action-enabled core checkpoints
are still pending; the separate durable-action recipe owns consequential effects.

## Qualification

Final checks pass: `make verify` (workspace tests, Clippy, model reference checks,
292 extension tests and metadata checks), documentation build, model-contract
drift check, Tua/Lua formatting gates and the native verification script.
The core suite reports 2160 passed and one existing ignored test.

The permanent CLI regression checks and builds typed source, then runs emitted
Lua for both the existing checkpoint round trips and the new persistence tests.
Coverage includes line/choice/suspended-wait round trips, rebound facts and locals,
unavailable choices, saved locale, invalid schema, changed version/content/graph/
position/timer, failed writes before and after persistence, failed read-back, torn
writes and two damaged slots. Rejected loads keep the player idle and perform no
writes. The in-memory backend uses a test-only checksum with real Save JSON/schema
validation; native filesystem runs use Save's real SHA-256 backend.

Native LÖVE 11.5 / LuaJIT 2.1.1700008891 qualification uses four separate processes
per JIT mode (enabled and disabled): write a line, restore/save a choice,
restore/save a suspended wait, then restore/finish it. Each mode uses a unique
save identity; successful completion removes its two save files. These are process
restarts, not simulated fsync or physical power loss. Standalone tests run under
Lua 5.4.8; no separate Lua 5.1 executable or installed editor UI check is claimed.

Compiler: Tua 0.11.0 debug, `aarch64-apple-darwin`, revision
`2370897c7378af66293bcfbfac08935b4c6a7e29` plus this slice's changes. SHA-256:
`e165b80efe69e97ee913af37376ae0040de8fd75b06c47e5d05804af6c631ca1`.

## Typed boundary and follow-up

Save enum inference currently exposes a string rather than a literal union.
The example explicitly checks and narrows the persisted state before constructing
the typed restore argument; it does not use `any` or weaken runtime validation.
A minimal follow-up is an imported `Save.contract` with a `Save.enum` field assigned
to the corresponding literal union: it should retain that union for checking and
editor assistance. This recipe does not claim that inference gap is fixed.
