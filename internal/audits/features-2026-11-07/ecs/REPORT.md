# ECS feature audit

Audited candidate 0.6.0 on 2026-11-07. Product code unchanged. **Three confirmed Medium defects**, one demonstrated typing capability gap, and one unconfirmed cross-module editor symptom. None are claimed fixed. The normal entity/system workflow is usable, but absence and false-valued data are not represented consistently.

## Reproduce and evidence

From repository root:

```sh
python3 internal/audits/features-2026-11-07/ecs/probe.py
python3 internal/audits/features-2026-11-07/ecs/lsp_probe.py
/Applications/LOVE.app/Contents/MacOS/love internal/audits/features-2026-11-07/ecs/runtime-love
```

`probe.py` creates the versioned fixtures, runs the absolute `target/release/tua check` and `build` in each project, then executes generated `build/main.lua` with Lua. `results.json` records every status/stdout/stderr; `console.txt` preserves a rendering. The LSP script performs real stdio requests, retaining `lsp-evidence.json` and `lsp-console.txt`. `love-results.txt` independently records emitted programs under **LuaJIT 2.1.1700008891 / Lua 5.1** through LOVE; ordinary Lua was **5.4.8**.

For any individual case:

```sh
TUA="$PWD/target/release/tua"
cd internal/audits/features-2026-11-07/ecs/missing_get
"$TUA" check
"$TUA" build
(cd build && lua main.lua)
```

## ECS-01 — get/remove erase the possibility of an absent component

- Category: confirmed defect; severity **Medium**; confidence high; CLI, emitted Lua, LOVE/LuaJIT and LSP verified.
- Contract: `docs/ecs.md:201,203` explicitly says `get` and `remove` return nil for absent values. Without an established component-presence guard, a statically typed component does not guarantee attachment to an arbitrary entity.
- Minimal source: `missing_get/src/main.tua` (six lines): declare Position `{x="number"}`, create an empty world/entity, assign `world:get(e,P)` to `position`, then read `position.x`. `missing_remove` changes only `get` to `remove`.
- Expected: nullable result in hover/checking; reject or require a guard before the field access. `add` should remain nonnullable.
- Actual: both check and build succeed. Emitted Lua fails at line 6 indexing nil. Actual hover on source line 4, character 9 displays `local position: { x: number }`. Signature help documents “or nil” but uses generic `any`, so the surfaces do not present one precise contract.
- Independent lead cross-check: `../cross-checks/ecs-absent-components.json` re-runs both successful checks and emitted nil-index crashes.
- Positive control: `happy` attaches Position first, obtains/uses the correct value, and passes on both Lua and LOVE. It also verifies system enable/disable, query counts, destroying entities during traversal, clearing the world during traversal, and non-reused entity IDs after clear.
- Root: `crates/tua-core/src/ecs.rs:181-187` handles `add`, `get`, and `remove` identically and returns only component data type. Existing test `component_inference_keeps_add_get_remove_return_types` at `ecs.rs:951-969` explicitly asserts the same nonnullable type for all three operations, so it encodes the defect rather than testing absence.
- Fix: shared domain return inference should return `T | nil` for get/remove and `T` for add; add any proven-presence refinement separately. Align signature help with that shared result and correct the prose at `docs/ecs.md:158` showing an unconditional record type.
- Regression: present/absent/removed/destroyed entities, get/remove vs add, guarded and unguarded field reads, local/imported components, checker + hover + emitted-runtime assertions.

## ECS-02 — World:get converts stored false into nil

- Category: confirmed runtime defect; severity **Medium**; confidence high; generated Lua and LOVE/LuaJIT verified.
- Contract: docs only prohibit nil component values (`docs/ecs.md:181,200`); `get` should return the attached value. Schematic/dynamic component data intentionally remains permissive.
- Repro: `false_value/src/main.tua` uses an explicitly dynamic schema, adds `false`, prints `has/get/count`, and asserts retrieval equals false.
- Expected: `has=true`, `get=false`, `count=1`.
- Actual in Lua and LOVE: `has=true`, `get=nil`, `count=1`; assertion fails. This is not just a wrong hint: flags can be attached and counted yet read as absent.
- Root: `vendor/runtime/tuaecs.lua:99-102` uses `store and store.values[entity] or nil`, losing false. `has` at 105-108 and `remove` at 111-125 correctly distinguish nil from false.
- Fix: use an explicit `if store then return store.values[entity] end`; preserve nil for no store.
- Regression: add/get/has/query/remove over false, true, zero, empty string, tables and nil rejection, on Lua 5.1/LuaJIT as well as host Lua. Existing ECS domain tests largely exercise record-shaped component values; the four standalone runtime-test suites do not include ECS.

## ECS-03 — Documented optional component schema is required by the checker

- Category: confirmed API contract defect; severity **Medium**; confidence high; CLI verified, runtime implementation inspected.
- Repro: `optional_schema/src/main.tua`: `local Flag = ecs.component("Flag")`.
- Expected: permitted per `docs/ecs.md:190` signature `ecs.component(name, schema?)`; useful for tags or dynamic data.
- Actual: check/build fail with `TL1004: argument count mismatch: expected 2, found 1`.
- Root: `crates/tua-core/src/ecs.rs:324-329` constructs two required parameters, and `typed_function` at 475-480 sets required count equal to all parameters. Runtime `vendor/runtime/tuaecs.lua:15-24` already defaults schema with `schema or {}`.
- Fix: mark schema optional through the shared typed-function metadata, preserving dynamic component value inference for omitted schema.
- Regression: one-argument call accepted; zero and three rejected; hover/signature optionality; two-argument static schema validation still rejects incorrect fields. Positive negative control `bad_schema` produces expected `TL5002` for `{x="bad"}`.

## ECS-GAP-01 — System callback data has no contextual checked schema

- Category: demonstrated capability gap, **not counted as a confirmed contract regression**; confidence high for observed behavior.
- `bad_callback` defines Position.x:number, then in `ecs.system(world,{P},function(entity,position,dt) ... end)` assigns `position.x` to both a number and a string. Check/build accept both; only unused-local warnings appear. Actual stdio hover of `position` in the callback reports `any`.
- `tua/ecsGraph` nevertheless resolves P to its component declaration and recognizes the system's read access. Thus the project already possesses useful contextual component identity that does not reach the callback contract.
- Source: `ecs.rs:333-338` models update as `Any`; inspection maintains distinct structural facts. This differs from STORES-01's rejection of shortened callbacks, though both belong to a shared callback-contract improvement epic.
- Improvement: provide the resolved entity/component/dt signature through shared contextual callback inference, preserving Lua's allowance for unused trailing callback arguments and unknown/dynamic component lists. Test both a correctly typed movement callback and an incompatible assignment in its body, plus queries and imported components.

## Follow-ups and missing options

- Unconfirmed: imported Position executes correctly (`imported` passes check/build/runtime), but the multi-root stdio probe returns `local position: any` for `world:get(e,domain.Position)`. Graph resolves the imported component. Independently reproduce in a single-root editor before classifying; reconcile with stores/module lane rather than adding a duplicate unconfirmed defect.
- Design option: absence/exclusion and any-of queries, optional query components, deterministic ordering adapters, explicit deferred command buffers. Current intersection-only/unordered behavior is documented, so their absence is not a bug.
- Design option: branded world-owned entity handles for accidental cross-world numeric-ID misuse; numeric IDs are currently an explicit runtime design.
- Performance remains unqualified for this pass. No large-population benchmark or GUI explorer interaction was performed.

## Coverage matrix

| Area | Status | Evidence |
|---|---|---|
| create/add/get/remove/destroy/clear, ID lifecycle | tested | happy + absent and false probes |
| System parameter order, dt, enabled | tested | happy emitted execution |
| Query count and entity destruction during iteration | tested | happy |
| Clear during iteration | tested, one matching entity | happy; multi-entity stress not claimed |
| Static payload schema errors | tested | bad_schema TL5002 |
| Missing schema option | tested | optional_schema TL1004 |
| Callback checking and hover | tested | bad_callback + real LSP |
| Imported components | tested CLI/runtime; editor symptom unconfirmed | imported + LSP |
| Local runtime override | tested | override, custom.lua retained, no bundled injection |
| ECS Explorer structural facts | tested protocol only | tua/ecsGraph response |
| Disabled config | inspected only | docs config; not executed |
| New matches during iteration | not tested; documented unspecified order | docs/ecs.md:176-181 |
| GUI explorer, long sessions, population performance | not exercised | explicit limits |

No exact pending ECS item matching these three defects was found in ROADMAP.md; the broad semantic precision and real-project qualification priorities apply. The known nominal-alias release failures were not recounted as ECS findings.

## Final reconciliation

The initial imported component-schema observation below was subsequently isolated and confirmed as MODULES-02; see [module audit](../modules/REPORT.md).
