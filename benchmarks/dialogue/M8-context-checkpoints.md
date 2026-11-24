# M8 typed-context checkpoints

Version 2 adds owned Save-shape-validated locals and current-fact rebinding to
line/choice/wait restore. Plain version-1 export and graph identity remain unchanged.
Branch/choice conditions, targets, bindings and local defaults join v2 graph
identity. Full schema semantics remain the host compatibility key's responsibility.
Actions and typed-event registries are still excluded; no durable effect recovery
or automatic migration is claimed. The complete Save recipe remains pending.

Restore validates/bounds local data before schema copying or resolver callbacks.
It stages locals and presentation before mutation, rejects a rebound choice with
no enabled visible option, and never follows fallback during restore. History,
media, gameplay facts and external handlers are not serialized. History stays
empty on restore and restored presentation remains silent.

## Qualification

The final `make verify` run passed: workspace tests (2,160 core unit tests,
one existing ignored test), CLI/LSP integrations, clippy, model/feature examples,
extension checks, metadata and whitespace validation. An earlier run was explicitly
interrupted while refining the editor fixture; this result is from the fresh final run.


- Focused emitted-Lua tests pass for v1 compatibility, typed local ownership,
  false/signed numbers/arrays/optional values, rebound facts, line/choice/wait
  boundaries, suspension, corrupt/oversized input, changed conditions/bindings,
  compatible prose, resolver failure/mutation and event-registry exclusion.
- Version-2 data round-trips through Save's real JSON/schema pipeline with an
  in-memory test backend (test checksum, not production SHA-256).
- Imported and forwarded context definitions preserve concrete checkpoint-local
  types, hover/completion and invalid-write diagnostics. See the distinct player
  forwarding limitation below.
- The maintained authored example checks/builds and runs on standalone Lua 5.4.8.
  LÖVE 11.5 / LuaJIT 2.1.1700008891 passes the checkpoint fixture families and
  authored example with JIT enabled and disabled. No standalone Lua 5.1/LuaJIT
  executable or installed editor UI validation is claimed.
- A frozen checkpoint helper from `7d1f7890` matches new v1 graph output. V2 graph
  encoding with signed/fractional data matches between standalone Lua and LuaJIT.
- The audio example's controller/backend checks and 100 rendered frames per JIT
  mode pass, including silent restored presentations. No listening test is claimed.
- Authored Lua/Tua formatting, Rust formatting, docs build, model-contract drift,
  JSON metadata and whitespace checks pass.

Native compiler provenance: Tua 0.11.0, debug, `aarch64-apple-darwin`, revision
`7d1f7890ac66a94f1f6a7938b9827d6f417fb47d` plus this slice's working changes. Executable SHA-256:
`021f14e5048f033b896cf28ffc9dc410284bf7189349beb5443b448487095c08`.

## Constructed-player forwarding follow-up

On Tua 0.11.0 plus this slice, the permanent regression verifies imported/forwarded
context definitions, concrete checkpoint locals, hover/completion and rejected
invalid writes. Additional forwarding of an already-constructed player loses
inference in AnalysisHost. The example constructs players in the consuming module;
no `any` widening is added. An isolated CLI check with one direct player import
correctly rejected a wrong `setLocals` value, so this is not a universal import bug.

Editor-analysis reproduction matching the regression fixture:

```tua
-- context.tua
return Dialogue.context({
  facts = Save.shape(Save.record({ coins = Save.number(0) })),
  locals = Save.shape(Save.record({ visits = Save.number(0), flag = Save.boolean(false) })),
})
-- forward.tua
return require("context")
-- session.tua
local definition = require("forward")
local player = definition.new(require("scene"), { coins = 10 }, {
  checkpointVersion = "v1",
})
return player
-- players.tua
return require("session")
-- consumer.tua
local player = require("players")
local saved = player:checkpoint()
saved.locals.visits = "wrong"
player:restore("wrong")
```

The authored `scene.tdialogue` is:

```text
@dialogue 1
@document demo
@context "forward"
[start]
// @id demo.hello
: Hello (end)
```

Expected: type mismatches. Observed in AnalysisHost: invalid writes/arguments were
accepted. An inline-document variant also reproduced the failure. A follow-up should
compare CLI/editor module inference and cover direct/named returns and extra forwarding
for existing methods and checkpoints. No constructed-player forwarding fix is claimed.
