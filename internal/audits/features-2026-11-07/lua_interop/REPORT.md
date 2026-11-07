# Lua interop and annotations audit

Bounded CLI/runtime/actual-stdio-LSP audit against the shared 0.6.0 baseline.
Two confirmed **medium** defects; no product changes. Both affect documented
mixed Lua/Tua workflows. No claims of comprehensive LuaLS compatibility.

## Reproduction and evidence

From repository root run `node internal/audits/features-2026-11-07/lua_interop/probe.js`.
The harness creates isolated projects in this directory, calls absolute
`target/release/tua check` and `build`, invokes `/opt/homebrew/bin/lua main.lua`
from successful build directories, and starts the built binary's real stdio LSP.
`results.json` records every CLI exit/output, LSP request/result and diagnostic
notification; `probe.log` retains stdout. Runtime is Lua 5.4.8. All source,
configuration and emitted files are retained. No cargo rebuild or broad suite.

| Surface | Outcome |
| --- | --- |
| LuaDoc class, fields, literal-union alias, params, returns | Exercised through Lua provider and Tua consumer; record completion and hover work; invalid parameter/field assignment rejected |
| Typed varargs and multiple returns | Valid mixed workflow checks/builds/runs: `15\t3\tthree\t6`; wrong vararg rejected TL1003 |
| Plain Lua preservation | Successful mixed build copies provider byte-for-byte (`luaCopiesEqual: true`) |
| Overloads | Local negative correctly rejected; imported contract lost (LUA-INTEROP-02) |
| Legacy metatable objects | Fields and method completion survive, valid colon call rejected (LUA-INTEROP-01) |
| LuaCATS emission | Alias, function params/return, local type and export class emitted; runtime prints `5` |
| Annotation authoring | Actual LSP tag completion, alias type completion, tag hover, documented signature work |
| Deprecation / nodiscard / metadata | With standard rules enabled, CLI and final LSP report TL4056/TL4068; `@diagnostic disable` does not suppress TL1003; `@generic T` alone is accepted metadata |
| Conflicting native/comment types | CLI and final LSP reject with TL1006 |
| Malformed annotations | Partial `@param` with rules=false did not diagnose; no finding assigned because standard-rule gating/partial authoring needs a separate isolated probe |
| Plain Lua CLI diagnostics | CLI reports checked 0 files for all-Lua fixture; LSP diagnoses its annotated mismatch. Recorded boundary, not counted as new defect |
| Not exercised | Generic inference (explicitly unsupported), every metadata tag, external declaration-library mapping, every legacy object builder, downstream LuaLS interpretation, installed GUI editor, LuaJIT runtime |

## LUA-INTEROP-01 — Documented metatable colon call rejected

**Medium; confirmed defect; new in this bounded audit search.**

Exact fixture: `metatable/src/player.lua` and `metatable/src/main.tua`.
The provider defines `Player.__index = Player`, `Player.new` returning
`setmetatable({name=name,x=0,active=true},Player)`, and
`function Player:update(dt) self.x=self.x+100*dt end`.
The consumer loads it, constructs `p`, then calls `p:update(0.02)`.

Run the absolute baseline `tua check` with cwd `lua_interop/metatable`.
Expected: acceptance, as expressly shown in `docs/lua-interop.md` under
“Use A Lua Module From Tua.” Actual: exit 1, `main.tua:3:1 TL1004:
argument count mismatch: expected 1, found 2`. The actual LSP publishes the
same error. Signature help still offers `function p.update(dt: any): any`.

Runtime oracle: run `tua build --emit-on-error` in that project (exit 1 but
emits), then `/opt/homebrew/bin/lua main.lua` from `build`; exit 0, stdout
`2.0`. Raw output is `metatable-runtime.json`. The harness rerun performs a
normal failing build; rerun those two explicit commands to refresh this oracle.

Source: `crates/tua-core/src/module_index/mod.rs:505-521` records method names
but derives callable shape from the body, whose explicit parameters omit Lua's
implicit receiver. The shape is propagated into constructor instances.
Existing test `crates/tua-core/src/module_index/tests.rs:179` verifies fields
and that update is a function, but does not invoke a consumer colon call.

Smallest maintainable improvement: preserve implicit receiver semantics when
constructing shared Lua method export signatures, ensuring checker and editor
consume the same contract. Add module-consumer core/LSP regressions for colon
calls, explicit receiver dot calls, and ordinary non-method functions; retain
runtime comparison. ROADMAP's typed Lua porting work is related but does not
explicitly enumerate this defect; searched prior audit reports found no match.

## LUA-INTEROP-02 — Lua exports discard overload call contracts

**Medium; confirmed defect; new in this bounded audit search.**

Exact fixtures: `overload-local/src/main.tua` and
`overload-import/src/{main.tua,legacy.lua}`. Both functions have these tags:

```lua
---@overload fun(value: string): string
---@overload fun(value: number): number
local function identity(value) return value end
```

Local `identity(true)` correctly fails TL1003. Exporting the same function in
`return {identity=identity}` and calling `Legacy.identity(true)` from Tua instead
passes `check` and `build`; runtime prints `true`. Expected: reject the boolean
against the two explicitly declared signatures, preserving the LuaDoc boundary
contract described in `docs/lua-interop.md` and `docs/annotations.md`.

Actual LSP corroborates contract loss: local call signature help lists both
string and number overloads; imported signature help lists only
`function Legacy.identity(value: any): any`, and the result hover is `any`.
All raw outputs are in `results.json` cases `overload-local` and
`overload-import`. The positive identity implementation intentionally accepts
all values at runtime; this finding is a lost static contract, not a claim of
runtime failure.

Source: `crates/tua-core/src/module_index/mod.rs:1579-1600`
(`typed_function_type`) creates a single FunctionType from primary parameters
and returns, omitting TypedFunction overloads. Existing core checks at
`crates/tua-core/src/check/tests.rs:1948` and signature tests at
`crates/tua-core/src/signature_help/tests.rs:124` exercise local overloads;
module-index tests contain no overload regression.

Smallest maintainable improvement: preserve the complete callable overload set
in the shared module export representation and route checking/signature help
through it. Test Lua and Tua module exports, good string/number calls, bad
boolean calls, result inference and actual LSP signature lists. ROADMAP calls
for expanded LuaDoc interoperability; known prior hover overload-selection
issues concern choosing among visible overloads, whereas this case loses the
entire set at the module boundary.

## Limits and follow-up

The negative aggregate fixture also reads an unknown member on an annotated
Enemy and does not diagnose it. It was not isolated against the intended open
LuaDoc-object policy, so it is evidence only, not a third confirmed defect.
Metadata-only generic/cast/as/diagnostic behavior and native generic syntax
limitations are not bugs. Configuration `rules=false` deliberately removes
ordinary lint noise in most probes; metadata warning tests explicitly enable
rules. LSP initial empty diagnostics followed by final project diagnostics are
retained, not confused with final acceptance. No unsupported syntax or external
service was required for either finding.
